use anyhow::Result;
use tracing::{info, warn};

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        let raw = crate::protocol::load_unit_info(&mut port)?;
        print_unit_info(&raw);
        Ok(())
    })
    .await?;

    Ok(())
}

fn print_unit_info(raw: &[u8]) {
    if raw.len() < 76 {
        println!("Unit info response too short ({} bytes)", raw.len());
        return;
    }
    let serial_bytes = &raw[5..11];
    let firmware_bytes = &raw[69..75];
    let serial: u64 = serial_bytes
        .iter()
        .enumerate()
        .fold(0u64, |acc, (i, &b)| acc + (b as u64) * (1u64 << (i * 8)));
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
