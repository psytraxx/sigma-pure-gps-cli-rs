mod commands;

use anyhow::{Context, Result, bail};
use serialport::SerialPort;
use std::time::Duration;
use tracing::{debug, warn};

const CHUNK_SIZE: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const BAUD_RATE: u32 = 115_200;

pub fn open_port(port_name: &str) -> Result<Box<dyn SerialPort>> {
    serialport::new(port_name, BAUD_RATE)
        .timeout(READ_TIMEOUT)
        .open()
        .with_context(|| format!("Failed to open {port_name}"))
}

/// Returns the raw 76-byte unit info response.
pub fn load_unit_info(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    send(port, commands::CMD_LOAD_UNIT_INFO)?;
    recv(port, 76)
}

/// Reads and discards the full EEPROM. The original app always does this before writing AGPS
/// data — the device will not respond to CMD_SEND_AGPS without the prior EEPROM read.
pub fn load_eeprom(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    send(port, commands::CMD_GET_COMPLETE_EEPROM)?;
    let raw = recv(port, 1030)?;
    // Strip 5-byte header and trailing checksum; payload is 1024 bytes
    Ok(raw[5..5 + 1024].to_vec())
}

pub fn print_unit_info(raw: &[u8]) {
    if raw.len() < 76 {
        println!("Unit info response too short ({} bytes)", raw.len());
        return;
    }
    // Strip 5-byte header and trailing checksum; serial = payload[0..6], firmware = payload[64..70]
    let serial_bytes = &raw[5..11];
    let firmware_bytes = &raw[69..75];

    // Serial number: 6-byte little-endian integer
    let serial: u64 = serial_bytes
        .iter()
        .enumerate()
        .fold(0u64, |acc, (i, &b)| acc + (b as u64) * (1u64 << (i * 8)));

    // Firmware version is stored as a BCD byte: the hex representation is read as decimal.
    // e.g. byte 0x32 -> hex string "32" -> parsed as decimal 32 -> displayed as "3.2"
    let fw_raw = format!("{:02X}", firmware_bytes[1]);
    let fw_val: u32 = fw_raw.parse().unwrap_or_else(|_| {
        warn!(
            "Unexpected firmware byte value {:#04x} (hex \"{fw_raw}\" is not valid BCD); \
             displaying as 0.0",
            firmware_bytes[1]
        );
        0
    });
    let firmware = format!("{}.{}", fw_val / 10, fw_val % 10);

    println!("Serial number:    {serial}");
    println!("Firmware version: {firmware}");
}

/// Returns the 32-byte settings block from EEPROM offset 272.
/// Covers timezone, language, units, contrast, altitude references.
/// (See Gps10Decoder.decodeSettings in the ActionScript source)
pub fn get_settings(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    const SETTINGS_OFFSET: usize = 272;
    const SETTINGS_SIZE: usize = 32;
    let eeprom = load_eeprom(port)?;
    eeprom
        .get(SETTINGS_OFFSET..SETTINGS_OFFSET + SETTINGS_SIZE)
        .map(|s| s.to_vec())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "EEPROM too short for settings block ({} bytes)",
                eeprom.len()
            )
        })
}

/// Returns the 20-byte totals block from EEPROM offset 304.
/// Covers cumulative distance, time, calories, elevation gain, and reset date.
/// (See Gps10Decoder.decodeTotals in the ActionScript source)
pub fn get_totals(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    const TOTALS_OFFSET: usize = 304;
    const TOTALS_SIZE: usize = 20;
    let eeprom = load_eeprom(port)?;
    eeprom
        .get(TOTALS_OFFSET..TOTALS_OFFSET + TOTALS_SIZE)
        .map(|s| s.to_vec())
        .ok_or_else(|| {
            anyhow::anyhow!("EEPROM too short for totals block ({} bytes)", eeprom.len())
        })
}

/// Returns the 172-byte sleep screen block from EEPROM offset 96.
/// Contains a 118-byte 16×59 pixel bitmap, clock position, name position, and CRC.
/// (See Gps10Decoder.encodeSleepScreen / POSITION_SLEEPSCREEN in the ActionScript source)
pub fn get_sleep_screen(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    const SLEEPSCREEN_OFFSET: usize = 96;
    const SLEEPSCREEN_SIZE: usize = 172;
    let eeprom = load_eeprom(port)?;
    eeprom
        .get(SLEEPSCREEN_OFFSET..SLEEPSCREEN_OFFSET + SLEEPSCREEN_SIZE)
        .map(|s| s.to_vec())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "EEPROM too short for sleep screen block ({} bytes)",
                eeprom.len()
            )
        })
}

/// Reads 15 bytes from flash at AGPS_DATA_START (0x1000 = 4096).
/// Command sends len-1=14; response is 15+6 bytes. Date is at payload offsets [10..12].
pub fn get_agps_flash_header(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    let cmd = commands::build_flash_read_cmd(0x1000, 14);
    send(port, &cmd)?;
    let raw = recv(port, 5 + 15 + 1)?;
    verify_checksum_seed0(&raw)?;
    Ok(raw[5..5 + 15].to_vec())
}

pub fn upload_agps(port: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    // Step 1: notify start (fire-and-forget) + CMD_SEND_AGPS
    send(port, commands::CMD_TRANSFER_STARTED)?;
    send(port, commands::CMD_SEND_AGPS)?;
    let reply = recv(port, 8)?;
    if reply.len() < 7 {
        bail!("CMD_SEND_AGPS response too short: {} bytes", reply.len());
    }
    debug!("CMD_SEND_AGPS reply: {:02X?}", reply);

    // Step 2: open stream
    send(port, commands::CMD_SEND_START)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_START reply: {:02X?}", reply);
    std::thread::sleep(Duration::from_millis(50));

    // Step 3: send data in 64-byte chunks, 50 ms apart
    for chunk in data.chunks(CHUNK_SIZE) {
        send(port, chunk)?;
        std::thread::sleep(Duration::from_millis(50));
    }

    // Close stream
    send(port, commands::CMD_SEND_END)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_END reply: {:02X?}", reply);
    std::thread::sleep(Duration::from_millis(50));

    // Step 4: confirm success
    send(port, commands::CMD_TRANSFER_SUCCESS)?;
    Ok(())
}

/// Writes a modified 1024-byte EEPROM image back to the device.
///
/// Sequence mirrors AGPS upload: CMD_TRANSFER_STARTED → CMD_SEND_EEPROM (recv 8) →
/// CMD_SEND_START (recv 9) → 1024 bytes in 64-byte chunks → CMD_SEND_END (recv 9) →
/// CMD_TRANSFER_SUCCESS.
pub fn write_eeprom(port: &mut Box<dyn SerialPort>, eeprom: &[u8; 1024]) -> Result<()> {
    send(port, commands::CMD_TRANSFER_STARTED)?;
    send(port, commands::CMD_SEND_EEPROM)?;
    let reply = recv(port, 8)?;
    debug!("CMD_SEND_EEPROM reply: {:02X?}", reply);

    send(port, commands::CMD_SEND_START)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_START reply: {:02X?}", reply);
    std::thread::sleep(Duration::from_millis(50));

    for chunk in eeprom.chunks(CHUNK_SIZE) {
        send(port, chunk)?;
        std::thread::sleep(Duration::from_millis(50));
    }

    send(port, commands::CMD_SEND_END)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_END reply: {:02X?}", reply);
    std::thread::sleep(Duration::from_millis(50));

    send(port, commands::CMD_TRANSFER_SUCCESS)?;
    Ok(())
}

/// Writes home altitude 1 and/or 2 into the settings block (EEPROM offset 272) and uploads
/// the full EEPROM.
///
/// Encoding from encodeSettings in Gps10Decoder.as: raw = altitude_m * 10 + 10000 (16-bit LE).
/// Update flag UPDATE_FLAG_SETTINGS=16 → flags [16, 2, 1, 20] at EEPROM offset 80.
pub fn set_home_altitude(
    port: &mut Box<dyn SerialPort>,
    alt1_m: Option<i32>,
    alt2_m: Option<i32>,
) -> Result<()> {
    let eeprom_vec = load_eeprom(port)?;
    let mut eeprom: [u8; 1024] = eeprom_vec
        .try_into()
        .map_err(|_| anyhow::anyhow!("EEPROM read returned unexpected length"))?;

    let settings = &mut eeprom[272..272 + 32];

    if let Some(m) = alt1_m {
        let raw = (m * 10 + 10000) as u16;
        settings[7] = (raw & 0xFF) as u8;
        settings[8] = (raw >> 8) as u8;
    }
    if let Some(m) = alt2_m {
        let raw = (m * 10 + 10000) as u16;
        settings[9] = (raw & 0xFF) as u8;
        settings[10] = (raw >> 8) as u8;
    }

    // Recalculate settings block checksum (seed=1, covers bytes 0..30).
    let crc = settings[..31]
        .iter()
        .fold(1u8, |acc, &b| acc.wrapping_add(b));
    eeprom[272 + 31] = crc;

    // UPDATE_FLAG_SETTINGS=16: generateUpdateFlagData(16) → [16, 2, 1, 20].
    eeprom[80..84].copy_from_slice(&[0x10, 0x02, 0x01, 0x14]);

    write_eeprom(port, &eeprom)
}

/// Erases all activity log data on the device by writing the TRIP_DATA_RESET update flag.
///
/// Reads the current EEPROM, patches offset 80 with update flags [0, 6, 1, 8]
/// (UPDATE_FLAG_TRIP_DATA_RESET=4, per generateUpdateFlagData in Gps10Handler.as), then
/// writes the full 1024-byte image back.
pub fn delete_tracks_memory(port: &mut Box<dyn SerialPort>) -> Result<()> {
    let eeprom_vec = load_eeprom(port)?;
    let mut eeprom: [u8; 1024] = eeprom_vec
        .try_into()
        .map_err(|_| anyhow::anyhow!("EEPROM read returned unexpected length"))?;

    // UPDATE_FLAGS_DEFAULT_DATA = [0, 2, 0, 3]; applying flag 4 (TRIP_DATA_RESET):
    //   byte[1] |= 4 → 6, byte[1] |= 2 → 6 (already set), bit-count of byte[0]=0 plus 1 for
    //   flag 4 → byte[2]=1, CRC = (0+6+1+seed_1) & 0xFF = 8.
    let update_flags: [u8; 4] = [0x00, 0x06, 0x01, 0x08];
    eeprom[80..84].copy_from_slice(&update_flags);

    write_eeprom(port, &eeprom)
}

pub struct LogHeaderMeta {
    pub count: u8,
}

/// Returns the number of stored log headers.
pub fn get_log_header_count(port: &mut Box<dyn SerialPort>) -> Result<LogHeaderMeta> {
    send(port, commands::CMD_GET_LOG_HEADER_COUNT)?;
    let reply = recv(port, 8)?;
    debug!("log header count reply: {:02X?}", reply);
    let count = reply[5];
    Ok(LogHeaderMeta { count })
}

/// Returns the raw bytes for all log headers (n × 65 bytes, stripped of the 5-byte response
/// header and trailing checksum byte).
pub fn get_log_headers(port: &mut Box<dyn SerialPort>, count: u8) -> Result<Vec<u8>> {
    let n = count as u32;
    let len = n * 65;
    let start = 0x1F_DFFFu32 - len + 1;
    // AS3 sends len-1 as the command length but reads len+6 bytes back
    let cmd = commands::build_flash_read_cmd(start, len - 1);
    send(port, &cmd)?;
    let total = (len + 6) as usize;
    let raw = recv(port, total)?;
    verify_checksum_seed0(&raw)?;
    Ok(raw[5..5 + len as usize].to_vec())
}

/// Returns raw log data bytes for a single track (stripped of 5-byte header + 2 trailing bytes).
pub fn get_log_data(
    port: &mut Box<dyn SerialPort>,
    start_addr: u32,
    stop_addr: u32,
) -> Result<Vec<u8>> {
    let len = stop_addr - start_addr + 1;
    let cmd = commands::build_flash_read_cmd(start_addr, len);
    send(port, &cmd)?;
    // Response: 5-byte header + len bytes + checksum + extra byte
    let total = (len + 7) as usize;
    let raw = recv(port, total)?;
    Ok(raw[5..5 + len as usize].to_vec())
}

fn send(port: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    port.write_all(data).context("Serial write failed")
}

fn recv(port: &mut Box<dyn SerialPort>, n: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    port.read_exact(&mut buf).context("Serial read failed")?;
    Ok(buf)
}

fn verify_checksum_seed0(data: &[u8]) -> Result<()> {
    if data.is_empty() {
        anyhow::bail!("Empty response");
    }
    let expected = data[data.len() - 1];
    let computed = data[..data.len() - 1]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_add(b));
    if computed != expected {
        anyhow::bail!("Response checksum mismatch: computed {computed:#04x}, got {expected:#04x}");
    }
    Ok(())
}
