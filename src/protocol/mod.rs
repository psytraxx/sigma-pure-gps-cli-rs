mod commands;

use anyhow::{Context, Result, bail};
use serialport::SerialPort;
use std::time::Duration;
use tracing::debug;

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

    // Firmware: byte[1] treated as BCD-style decimal (toString(16) parsed as base-10)
    // e.g. 0x32 -> "32" -> 32 -> "3.2"
    let fw_raw = format!("{:02X}", firmware_bytes[1]);
    let fw_val: u32 = fw_raw.parse().unwrap_or(0);
    let firmware = format!("{}.{}", fw_val / 10, fw_val % 10);

    println!("Serial number:    {serial}");
    println!("Firmware version: {firmware}");
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
