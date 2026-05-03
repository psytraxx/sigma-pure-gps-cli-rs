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

/// Returns `true` if the device acknowledged the poll.
pub fn check_device_connected(port: &mut Box<dyn SerialPort>) -> Result<bool> {
    send(port, commands::CMD_CHECK_CONNECTED)?;
    let reply = recv(port, 4)?;
    Ok(reply.first() == Some(&0x11))
}

/// Returns the raw 76-byte unit info response.
pub fn load_unit_info(port: &mut Box<dyn SerialPort>) -> Result<Vec<u8>> {
    send(port, commands::CMD_LOAD_UNIT_INFO)?;
    recv(port, 76)
}

pub fn print_unit_info(raw: &[u8]) {
    if raw.len() < 76 {
        println!("Unit info response too short ({} bytes)", raw.len());
        return;
    }
    // [0..4] = 5-byte header, [5..10] = serial number, [69..74] = firmware version
    let serial = &raw[5..11];
    let firmware = &raw[69..75];
    let serial_str = String::from_utf8_lossy(serial);
    let firmware_str = String::from_utf8_lossy(firmware);
    println!("Serial number:    {serial_str}");
    println!("Firmware version: {firmware_str}");
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

fn send(port: &mut Box<dyn SerialPort>, data: &[u8]) -> Result<()> {
    port.write_all(data).context("Serial write failed")
}

fn recv(port: &mut Box<dyn SerialPort>, n: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    port.read_exact(&mut buf).context("Serial read failed")?;
    Ok(buf)
}
