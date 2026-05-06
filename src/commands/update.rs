use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let client = crate::util::build_http_client()?;
    info!("Downloading AGPS data from u-blox...");
    let agps = crate::downloader::download(&client).await?;
    info!("Downloaded {} bytes", agps.bytes.len());

    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || upload(port_name, agps.bytes)).await?;
    info!("AGPS update complete.");
    Ok(())
}

fn upload(port_name: String, data: Vec<u8>) -> Result<()> {
    let mut port = crate::protocol::open_port(&port_name)?;
    info!("Loading unit info...");
    crate::protocol::load_unit_info(&mut port)?;
    info!("Reading EEPROM...");
    crate::protocol::load_eeprom(&mut port)?;
    info!("Uploading {} bytes of AGPS data...", data.len());
    crate::protocol::upload_agps(&mut port, &data)?;
    Ok(())
}
