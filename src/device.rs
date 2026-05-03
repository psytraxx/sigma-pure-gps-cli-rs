use anyhow::{Context, Result, bail};
use serialport::SerialPortInfo;
use tracing::debug;

// Sigma Sport USB Vendor ID (used across all SIGMA devices with CDC ACM serial)
const SIGMA_USB_VID: u16 = 0x1D9D;

pub fn find_sigma_port() -> Result<String> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;
    debug!("Found {} serial port(s)", ports.len());

    // Prefer a port with a matching USB VID
    if let Some(port) = ports.iter().find(|p| is_sigma_port(p)) {
        let name = port.port_name.clone();
        debug!("Found SIGMA device on {name} (VID match)");
        return Ok(name);
    }

    bail!(
        "No SIGMA device found. Is the Pure GPS connected via USB?\n\
        Available ports: {}",
        ports
            .iter()
            .map(|p| p.port_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn is_sigma_port(port: &SerialPortInfo) -> bool {
    match &port.port_type {
        serialport::SerialPortType::UsbPort(info) => {
            debug!(
                "  {} — VID:{:04X} PID:{:04X} ({})",
                port.port_name,
                info.vid,
                info.pid,
                info.product.as_deref().unwrap_or("unknown")
            );
            info.vid == SIGMA_USB_VID
        }
        _ => false,
    }
}
