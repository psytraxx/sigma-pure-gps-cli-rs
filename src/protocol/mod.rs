use anyhow::{Context, Result, bail};
use serialport::SerialPort;
use std::time::Duration;
use tracing::debug;

// Command bytes reverse-engineered from Gps10Handler.as (decimal literals converted to hex).

/// Tells the device a transfer is starting (no response expected).
const CMD_TRANSFER_STARTED: &[u8] = &[0x57, 0x08, 0x00, 0x00, 0x00, 0x00, 0x01, 0x60];

/// Prepares the device to receive AGPS flash data; device replies with 8 bytes.
const CMD_SEND_AGPS: &[u8] = &[
    0x52, 0x0C, 0x00, 0x00, 0x00, 0xF8, 0x7F, 0x00, 0x00, 0x10, 0x00, 0xE5,
];

/// Opens the data stream; device replies with 9 bytes.
const CMD_SEND_START: &[u8] = &[0x53, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x5B];

/// Closes the data stream; device replies with 9 bytes.
const CMD_SEND_END: &[u8] = &[0xAB, 0x08, 0x00, 0x00, 0x00, 0x01, 0x02, 0xB6];

/// Confirms a successful transfer (no response expected).
const CMD_TRANSFER_SUCCESS: &[u8] = &[0x57, 0x08, 0x00, 0x00, 0x00, 0x02, 0x01, 0x62];

/// Reads the full 1024-byte EEPROM; device replies with 1030 bytes (5 header + 1024 + checksum).
const CMD_GET_COMPLETE_EEPROM: &[u8] = &[
    0x56, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x03, 0x00, 0x64,
];

/// Requests device identification; device replies with 76 bytes.
const CMD_LOAD_UNIT_INFO: &[u8] = &[
    0x56, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x45, 0x00, 0x00, 0xA7,
];

/// Requests the number of stored log headers; device replies with 8 bytes (`reply[5]` = count).
const CMD_GET_LOG_HEADER_COUNT: &[u8] = &[
    0x54, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60,
];

/// Writes the full 1024-byte EEPROM back to the device; device replies with 8 bytes.
const CMD_SEND_EEPROM: &[u8] = &[
    0x52, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x62,
];

/// Builds a flash-read command for the given start address and length.
fn build_flash_read_cmd(start: u32, len: u32) -> Vec<u8> {
    let mut cmd = vec![
        0x56,
        0x0C,
        0x00,
        0x00,
        0x00,
        (start & 0xFF) as u8,
        (start >> 8 & 0xFF) as u8,
        (start >> 16 & 0xFF) as u8,
        (len & 0xFF) as u8,
        (len >> 8 & 0xFF) as u8,
        (len >> 16 & 0xFF) as u8,
    ];
    let checksum: u8 = cmd.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    cmd.push(checksum);
    cmd
}

const CHUNK_SIZE: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const BAUD_RATE: u32 = 115_200;
const CHUNK_DELAY: Duration = Duration::from_millis(50);

pub fn open_port(port_name: &str) -> Result<Box<dyn SerialPort>> {
    serialport::new(port_name, BAUD_RATE)
        .timeout(READ_TIMEOUT)
        .open()
        .with_context(|| format!("Failed to open {port_name}"))
}

/// Returns the raw 76-byte unit info response.
pub fn load_unit_info(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    send(port, CMD_LOAD_UNIT_INFO)?;
    recv(port, 76)
}

/// Reads and discards the full EEPROM. The original app always does this before writing AGPS
/// data — the device will not respond to CMD_SEND_AGPS without the prior EEPROM read.
pub fn load_eeprom(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    send(port, CMD_GET_COMPLETE_EEPROM)?;
    let raw = recv(port, 1030)?;
    // Strip 5-byte header and trailing checksum; payload is 1024 bytes
    Ok(raw[5..5 + 1024].to_vec())
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

/// Writes a 172-byte encoded sleep screen payload to EEPROM offset 96 and triggers
/// UPDATE_FLAG_SLEEPSCREEN (flag=8).
///
/// Flag bytes derived from generateUpdateFlagData(8) with default state [0,2,0,3]:
///   byte[0] |= 8 → 8; byte[1] |= 2 → 2; popcount(8)=1 → byte[2]=1;
///   CRC = (8+2+1+seed_1) & 0xFF = 12.
///   Result: [0x08, 0x02, 0x01, 0x0C] at EEPROM offset 80.
/// (See Gps10Handler.as writeUnitSleepScreen / generateUpdateFlagData)
pub fn set_sleep_screen(port: &mut Box<dyn SerialPort>, payload: &[u8; 172]) -> Result<()> {
    let eeprom_vec = load_eeprom(port)?;
    let mut eeprom: [u8; 1024] = eeprom_vec
        .try_into()
        .map_err(|_| anyhow::anyhow!("EEPROM read returned unexpected length"))?;

    eeprom[96..96 + 172].copy_from_slice(payload);
    eeprom[80..84].copy_from_slice(&[0x08, 0x02, 0x01, 0x0C]);

    write_eeprom(port, &eeprom)
}

/// Reads 15 bytes from flash at AGPS_DATA_START (0x1000 = 4096).
/// Command sends len-1=14; response is 15+6 bytes. Date is at payload offsets [10..12].
pub fn get_agps_flash_header(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    let cmd = build_flash_read_cmd(0x1000, 14);
    send(port, &cmd)?;
    let raw = recv(port, 5 + 15 + 1)?;
    verify_checksum_seed0(&raw)?;
    Ok(raw[5..5 + 15].to_vec())
}

pub fn upload_agps(port: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    // Step 1: notify start (fire-and-forget) + CMD_SEND_AGPS
    send(port, CMD_TRANSFER_STARTED)?;
    send(port, CMD_SEND_AGPS)?;
    let reply = recv(port, 8)?;
    if reply.len() < 7 {
        bail!("CMD_SEND_AGPS response too short: {} bytes", reply.len());
    }
    debug!("CMD_SEND_AGPS reply: {:02X?}", reply);

    // Step 2: open stream
    send(port, CMD_SEND_START)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_START reply: {:02X?}", reply);
    std::thread::sleep(CHUNK_DELAY);

    // Step 3: send data in 64-byte chunks, 50 ms apart
    for chunk in data.chunks(CHUNK_SIZE) {
        send(port, chunk)?;
        std::thread::sleep(CHUNK_DELAY);
    }

    // Close stream
    send(port, CMD_SEND_END)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_END reply: {:02X?}", reply);
    std::thread::sleep(CHUNK_DELAY);

    // Step 4: confirm success
    send(port, CMD_TRANSFER_SUCCESS)?;
    Ok(())
}

/// Writes a modified 1024-byte EEPROM image back to the device.
///
/// Sequence mirrors AGPS upload: CMD_TRANSFER_STARTED → CMD_SEND_EEPROM (recv 8) →
/// CMD_SEND_START (recv 9) → 1024 bytes in 64-byte chunks → CMD_SEND_END (recv 9) →
/// CMD_TRANSFER_SUCCESS.
pub fn write_eeprom(port: &mut Box<dyn SerialPort>, eeprom: &[u8; 1024]) -> Result<()> {
    send(port, CMD_TRANSFER_STARTED)?;
    send(port, CMD_SEND_EEPROM)?;
    let reply = recv(port, 8)?;
    debug!("CMD_SEND_EEPROM reply: {:02X?}", reply);

    send(port, CMD_SEND_START)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_START reply: {:02X?}", reply);
    std::thread::sleep(CHUNK_DELAY);

    for chunk in eeprom.chunks(CHUNK_SIZE) {
        send(port, chunk)?;
        std::thread::sleep(CHUNK_DELAY);
    }

    send(port, CMD_SEND_END)?;
    let reply = recv(port, 9)?;
    debug!("CMD_SEND_END reply: {:02X?}", reply);
    std::thread::sleep(CHUNK_DELAY);

    send(port, CMD_TRANSFER_SUCCESS)?;
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

/// Reads the 27-byte point navigation (waypoint) block from EEPROM offset 336.
/// (See Gps10Handler.as: POSITION_POINT_NAVIGATION=336, LENGTH_POINT_NAVIGATION=27)
pub fn get_waypoint(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    const WAYPOINT_OFFSET: usize = 336;
    const WAYPOINT_SIZE: usize = 27;
    let eeprom = load_eeprom(port)?;
    eeprom
        .get(WAYPOINT_OFFSET..WAYPOINT_OFFSET + WAYPOINT_SIZE)
        .map(|s| s.to_vec())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "EEPROM too short for waypoint block ({} bytes)",
                eeprom.len()
            )
        })
}

/// Writes a 27-byte encoded point navigation payload to EEPROM offset 336 and triggers
/// UPDATE_FLAG_POINT_NAVIGATION (flag=128).
///
/// Flag bytes derived from generateUpdateFlagData(128) with default state [0,2,0,3]:
///   byte[0] |= 128 → 128; byte[1] |= 2 → 2; popcount(128)=1 → byte[2]=1;
///   CRC = (128+2+1+seed_1) & 0xFF = 132 = 0x84.
///   Result: [0x80, 0x02, 0x01, 0x84] at EEPROM offset 80.
/// (See Gps10Handler.as writePointNavigation / generateUpdateFlagData)
pub fn set_waypoint(port: &mut Box<dyn SerialPort>, payload: &[u8; 27]) -> Result<()> {
    let eeprom_vec = load_eeprom(port)?;
    let mut eeprom: [u8; 1024] = eeprom_vec
        .try_into()
        .map_err(|_| anyhow::anyhow!("EEPROM read returned unexpected length"))?;

    eeprom[336..336 + 27].copy_from_slice(payload);
    eeprom[80..84].copy_from_slice(&[0x80, 0x02, 0x01, 0x84]);

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
    send(port, CMD_GET_LOG_HEADER_COUNT)?;
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
    let cmd = build_flash_read_cmd(start, len - 1);
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
    let cmd = build_flash_read_cmd(start_addr, len);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{ClearBuffer, DataBits, FlowControl, Parity, StopBits};
    use std::collections::VecDeque;
    use std::io::{self, Read, Write};
    use std::sync::{Arc, Mutex};

    // ── MockPort ─────────────────────────────────────────────────────────────

    struct MockPort {
        responses: Arc<Mutex<VecDeque<u8>>>,
        written: Arc<Mutex<Vec<u8>>>,
    }

    impl MockPort {
        fn new(responses: &[u8]) -> (Self, Arc<Mutex<Vec<u8>>>) {
            let written = Arc::new(Mutex::new(Vec::new()));
            let port = Self {
                responses: Arc::new(Mutex::new(VecDeque::from(responses.to_vec()))),
                written: Arc::clone(&written),
            };
            (port, written)
        }

        fn into_box(self) -> Box<dyn serialport::SerialPort> {
            Box::new(self)
        }
    }

    impl Read for MockPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut q = self.responses.lock().unwrap();
            let n = buf.len().min(q.len());
            for (dst, src) in buf[..n].iter_mut().zip(q.drain(..n)) {
                *dst = src;
            }
            Ok(n)
        }
    }

    impl Write for MockPort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl serialport::SerialPort for MockPort {
        fn name(&self) -> Option<String> {
            None
        }
        fn baud_rate(&self) -> serialport::Result<u32> {
            Ok(115_200)
        }
        fn data_bits(&self) -> serialport::Result<DataBits> {
            Ok(DataBits::Eight)
        }
        fn flow_control(&self) -> serialport::Result<FlowControl> {
            Ok(FlowControl::None)
        }
        fn parity(&self) -> serialport::Result<Parity> {
            Ok(Parity::None)
        }
        fn stop_bits(&self) -> serialport::Result<StopBits> {
            Ok(StopBits::One)
        }
        fn timeout(&self) -> Duration {
            Duration::from_secs(5)
        }
        fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
            Ok(())
        }
        fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
            Ok(())
        }
        fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
            Ok(())
        }
        fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
            Ok(())
        }
        fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
            Ok(())
        }
        fn set_timeout(&mut self, _: Duration) -> serialport::Result<()> {
            Ok(())
        }
        fn write_request_to_send(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }
        fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }
        fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }
        fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }
        fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }
        fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }
        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(0)
        }
        fn bytes_to_read(&self) -> serialport::Result<u32> {
            Ok(self.responses.lock().unwrap().len() as u32)
        }
        fn try_clone(&self) -> serialport::Result<Box<dyn serialport::SerialPort>> {
            Err(serialport::Error::new(
                serialport::ErrorKind::Unknown,
                "mock does not support clone",
            ))
        }
        fn clear(&self, _: ClearBuffer) -> serialport::Result<()> {
            Ok(())
        }
        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }
        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// 1030-byte fake EEPROM response: 5-byte header + 1024 EEPROM bytes + 1 ignored byte.
    fn eeprom_response(eeprom: &[u8; 1024]) -> Vec<u8> {
        let mut v = vec![0u8; 5];
        v.extend_from_slice(eeprom);
        v.push(0);
        v
    }

    /// The three reply blobs the device sends back during a write_eeprom sequence.
    fn write_eeprom_replies() -> Vec<u8> {
        let mut v = vec![0u8; 8]; // CMD_SEND_EEPROM reply
        v.extend_from_slice(&[0u8; 9]); // CMD_SEND_START reply
        v.extend_from_slice(&[0u8; 9]); // CMD_SEND_END reply
        v
    }

    /// Appends a seed-0 checksum byte and returns the full frame.
    fn with_seed0_checksum(data: &[u8]) -> Vec<u8> {
        let crc = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        let mut v = data.to_vec();
        v.push(crc);
        v
    }

    // Byte offset of the EEPROM payload inside the written buffer.
    //
    // write_eeprom sends, in order:
    //   CMD_TRANSFER_STARTED (8) + CMD_SEND_EEPROM (12) + CMD_SEND_START (8) = 28 bytes
    // before the 1024-byte EEPROM data.
    const WRITE_EEPROM_DATA_OFFSET: usize = 28;

    // When preceded by a load_eeprom call (CMD_GET_COMPLETE_EEPROM = 12 bytes):
    const LOAD_THEN_WRITE_EEPROM_DATA_OFFSET: usize = 12 + WRITE_EEPROM_DATA_OFFSET;

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn load_unit_info_returns_raw_response() {
        let mut raw = vec![0u8; 76];
        raw[5] = 0x42;
        let (mock, _written) = MockPort::new(&raw);
        let mut port = mock.into_box();
        let result = load_unit_info(&mut port).unwrap();
        assert_eq!(result.len(), 76);
        assert_eq!(result[5], 0x42);
    }

    #[test]
    fn load_eeprom_strips_five_byte_header() {
        let mut eeprom = [0u8; 1024];
        eeprom[0] = 0xAB;
        eeprom[1023] = 0xCD;
        let (mock, _written) = MockPort::new(&eeprom_response(&eeprom));
        let mut port = mock.into_box();
        let result = load_eeprom(&mut port).unwrap();
        assert_eq!(result.len(), 1024);
        assert_eq!(result[0], 0xAB);
        assert_eq!(result[1023], 0xCD);
    }

    #[test]
    fn get_settings_returns_correct_slice() {
        let mut eeprom = [0u8; 1024];
        eeprom[272] = 0x11;
        eeprom[303] = 0x22;
        let (mock, _written) = MockPort::new(&eeprom_response(&eeprom));
        let mut port = mock.into_box();
        let result = get_settings(&mut port).unwrap();
        assert_eq!(result.len(), 32);
        assert_eq!(result[0], 0x11);
        assert_eq!(result[31], 0x22);
    }

    #[test]
    fn get_totals_returns_correct_slice() {
        let mut eeprom = [0u8; 1024];
        eeprom[304] = 0x55;
        eeprom[323] = 0x66;
        let (mock, _written) = MockPort::new(&eeprom_response(&eeprom));
        let mut port = mock.into_box();
        let result = get_totals(&mut port).unwrap();
        assert_eq!(result.len(), 20);
        assert_eq!(result[0], 0x55);
        assert_eq!(result[19], 0x66);
    }

    #[test]
    fn get_sleep_screen_returns_correct_slice() {
        let mut eeprom = [0u8; 1024];
        eeprom[96] = 0x77;
        eeprom[267] = 0x88;
        let (mock, _written) = MockPort::new(&eeprom_response(&eeprom));
        let mut port = mock.into_box();
        let result = get_sleep_screen(&mut port).unwrap();
        assert_eq!(result.len(), 172);
        assert_eq!(result[0], 0x77);
        assert_eq!(result[171], 0x88);
    }

    #[test]
    fn write_eeprom_sends_data_in_chunks() {
        let mut eeprom = [0u8; 1024];
        eeprom[0] = 0xDE;
        eeprom[1023] = 0xAD;
        let (mock, written) = MockPort::new(&write_eeprom_replies());
        let mut port = mock.into_box();
        write_eeprom(&mut port, &eeprom).unwrap();
        let w = written.lock().unwrap();
        let sent = &w[WRITE_EEPROM_DATA_OFFSET..WRITE_EEPROM_DATA_OFFSET + 1024];
        assert_eq!(sent[0], 0xDE);
        assert_eq!(sent[1023], 0xAD);
    }

    #[test]
    fn set_sleep_screen_patches_bitmap_and_update_flags() {
        let orig_eeprom = [0u8; 1024];
        let mut payload = [0u8; 172];
        payload[0] = 0x01; // active id
        payload[168] = 27; // clock_x
        let mut responses = eeprom_response(&orig_eeprom);
        responses.extend(write_eeprom_replies());
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        set_sleep_screen(&mut port, &payload).unwrap();
        let w = written.lock().unwrap();
        let sent =
            &w[LOAD_THEN_WRITE_EEPROM_DATA_OFFSET..LOAD_THEN_WRITE_EEPROM_DATA_OFFSET + 1024];
        assert_eq!(sent[96], 0x01);
        assert_eq!(sent[96 + 168], 27);
        assert_eq!(&sent[80..84], &[0x08, 0x02, 0x01, 0x0C]);
    }

    #[test]
    fn get_agps_flash_header_valid_checksum() {
        // 5-byte header + 15-byte payload = 20 bytes, then seed-0 checksum
        let mut data = [0u8; 20];
        data[10] = 24; // distinguishing marker
        let frame = with_seed0_checksum(&data);
        let (mock, _written) = MockPort::new(&frame);
        let mut port = mock.into_box();
        let result = get_agps_flash_header(&mut port).unwrap();
        assert_eq!(result.len(), 15);
        assert_eq!(result[5], 24); // payload byte at raw offset 10
    }

    #[test]
    fn get_agps_flash_header_bad_checksum_fails() {
        let mut frame = with_seed0_checksum(&[0u8; 20]);
        *frame.last_mut().unwrap() ^= 0xFF;
        let (mock, _written) = MockPort::new(&frame);
        let mut port = mock.into_box();
        assert!(get_agps_flash_header(&mut port).is_err());
    }

    #[test]
    fn get_log_header_count_reads_byte5() {
        let mut reply = [0u8; 8];
        reply[5] = 3;
        let (mock, _written) = MockPort::new(&reply);
        let mut port = mock.into_box();
        let meta = get_log_header_count(&mut port).unwrap();
        assert_eq!(meta.count, 3);
    }

    #[test]
    fn get_log_headers_strips_framing() {
        // count=1: len=65, total=71 (5 header + 65 payload + 1 checksum)
        let mut payload = [0u8; 65];
        payload[0] = 0xBE;
        payload[64] = 0xEF;
        let mut frame = vec![0u8; 5];
        frame.extend_from_slice(&payload);
        let frame = with_seed0_checksum(&frame);
        let (mock, _written) = MockPort::new(&frame);
        let mut port = mock.into_box();
        let result = get_log_headers(&mut port, 1).unwrap();
        assert_eq!(result.len(), 65);
        assert_eq!(result[0], 0xBE);
        assert_eq!(result[64], 0xEF);
    }

    #[test]
    fn get_log_headers_bad_checksum_fails() {
        let mut frame = with_seed0_checksum(&[0u8; 5 + 65]);
        *frame.last_mut().unwrap() ^= 0xFF;
        let (mock, _written) = MockPort::new(&frame);
        let mut port = mock.into_box();
        assert!(get_log_headers(&mut port, 1).is_err());
    }

    #[test]
    fn get_log_data_strips_framing() {
        // start=0x1000, stop=0x1004: len=5, total=12 (5+5+2)
        let payload = [0x11u8, 0x22, 0x33, 0x44, 0x55];
        let mut frame = vec![0u8; 5];
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&[0, 0]);
        let (mock, _written) = MockPort::new(&frame);
        let mut port = mock.into_box();
        let result = get_log_data(&mut port, 0x1000, 0x1004).unwrap();
        assert_eq!(result, &[0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn set_home_altitude_encodes_alt1_correctly() {
        let orig_eeprom = [0u8; 1024];
        let mut responses = eeprom_response(&orig_eeprom);
        responses.extend(write_eeprom_replies());
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        // 500 m → raw = 500*10 + 10000 = 15000 = 0x3A98
        set_home_altitude(&mut port, Some(500), None).unwrap();
        let w = written.lock().unwrap();
        let sent =
            &w[LOAD_THEN_WRITE_EEPROM_DATA_OFFSET..LOAD_THEN_WRITE_EEPROM_DATA_OFFSET + 1024];
        let settings = &sent[272..272 + 32];
        assert_eq!(settings[7], 0x98);
        assert_eq!(settings[8], 0x3A);
        assert_eq!(&sent[80..84], &[0x10, 0x02, 0x01, 0x14]);
    }

    #[test]
    fn set_home_altitude_encodes_alt2_correctly() {
        let orig_eeprom = [0u8; 1024];
        let mut responses = eeprom_response(&orig_eeprom);
        responses.extend(write_eeprom_replies());
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        // 200 m → raw = 200*10 + 10000 = 12000 = 0x2EE0
        set_home_altitude(&mut port, None, Some(200)).unwrap();
        let w = written.lock().unwrap();
        let sent =
            &w[LOAD_THEN_WRITE_EEPROM_DATA_OFFSET..LOAD_THEN_WRITE_EEPROM_DATA_OFFSET + 1024];
        let settings = &sent[272..272 + 32];
        assert_eq!(settings[9], 0xE0);
        assert_eq!(settings[10], 0x2E);
    }

    #[test]
    fn delete_tracks_memory_sets_update_flags() {
        let orig_eeprom = [0u8; 1024];
        let mut responses = eeprom_response(&orig_eeprom);
        responses.extend(write_eeprom_replies());
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        delete_tracks_memory(&mut port).unwrap();
        let w = written.lock().unwrap();
        let sent =
            &w[LOAD_THEN_WRITE_EEPROM_DATA_OFFSET..LOAD_THEN_WRITE_EEPROM_DATA_OFFSET + 1024];
        assert_eq!(&sent[80..84], &[0x00, 0x06, 0x01, 0x08]);
    }

    #[test]
    fn upload_agps_sends_data_to_port() {
        // CMD_SEND_AGPS reply (8) + CMD_SEND_START reply (9) + CMD_SEND_END reply (9)
        let mut responses = vec![0u8; 8];
        responses.extend_from_slice(&[0u8; 9]);
        responses.extend_from_slice(&[0u8; 9]);
        let data = vec![0xAAu8; 128]; // 2 × 64-byte chunks
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        upload_agps(&mut port, &data).unwrap();
        let w = written.lock().unwrap();
        // CMD_TRANSFER_STARTED(8) + CMD_SEND_AGPS(12) + CMD_SEND_START(8) = 28 bytes before data
        let data_offset = 28;
        let sent_data = &w[data_offset..data_offset + 128];
        assert!(sent_data.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn get_waypoint_returns_correct_slice() {
        let mut eeprom = [0u8; 1024];
        eeprom[336] = 0x41; // 'A'
        eeprom[362] = 0xFF; // last byte of 27-byte block
        let (mock, _written) = MockPort::new(&eeprom_response(&eeprom));
        let mut port = mock.into_box();
        let result = get_waypoint(&mut port).unwrap();
        assert_eq!(result.len(), 27);
        assert_eq!(result[0], 0x41);
        assert_eq!(result[26], 0xFF);
    }

    #[test]
    fn set_waypoint_patches_block_and_update_flags() {
        use crate::decoder::{Waypoint, encode_waypoint};
        let wp = Waypoint {
            text1: "Test".to_string(),
            text2: "".to_string(),
            lat: 47.0,
            lon: 8.0,
        };
        let payload = encode_waypoint(&wp).unwrap();
        let orig_eeprom = [0u8; 1024];
        let mut responses = eeprom_response(&orig_eeprom);
        responses.extend(write_eeprom_replies());
        let (mock, written) = MockPort::new(&responses);
        let mut port = mock.into_box();
        set_waypoint(&mut port, &payload).unwrap();
        let w = written.lock().unwrap();
        let sent =
            &w[LOAD_THEN_WRITE_EEPROM_DATA_OFFSET..LOAD_THEN_WRITE_EEPROM_DATA_OFFSET + 1024];
        assert_eq!(&sent[336..336 + 27], &payload);
        assert_eq!(&sent[80..84], &[0x80, 0x02, 0x01, 0x84]);
    }
}
