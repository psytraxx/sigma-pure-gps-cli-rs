use anyhow::{Context, Result};

pub fn run() -> Result<()> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;
    if ports.is_empty() {
        println!("No serial ports found.");
        return Ok(());
    }
    for p in &ports {
        match &p.port_type {
            serialport::SerialPortType::UsbPort(info) => {
                println!(
                    "{:15} USB  VID:{:04X} PID:{:04X}  {}",
                    p.port_name,
                    info.vid,
                    info.pid,
                    info.product.as_deref().unwrap_or("")
                );
            }
            _ => println!("{:15} (non-USB)", p.port_name),
        }
    }
    Ok(())
}
