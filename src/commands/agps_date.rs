use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    tokio::task::spawn_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::load_eeprom(&mut port)?;
        let data = crate::protocol::get_agps_flash_header(&mut port)?;
        let date = crate::decoder::decode_agps_date(&data)?;
        println!("AGPS data date: {}", date.format("%Y-%m-%d"));
        Ok::<_, anyhow::Error>(())
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("Task panicked: {e}")))?;

    Ok(())
}
