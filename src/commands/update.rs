use anyhow::{Context, Result};
use tracing::{error, info};

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let client = crate::util::build_http_client()?;
    info!("Downloading AGPS data from u-blox...");
    let agps = crate::downloader::download(&client).await?;
    info!("Downloaded {} bytes", agps.bytes.len());

    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let result = tokio::task::spawn_blocking(move || upload(port_name, agps.bytes))
        .await
        .context("Upload task panicked")?;

    match result {
        Ok(()) => info!("AGPS update complete."),
        Err(e) => {
            error!("Upload failed: {e:#}");
            std::process::exit(1);
        }
    }
    Ok(())
}

fn upload(port_name: String, data: Vec<u8>) -> Result<()> {
    let mut port = crate::protocol::open_port(&port_name)?;
    info!("Checking device connection...");
    if !crate::protocol::check_device_connected(&mut port)? {
        info!("Device poll did not confirm connection — attempting upload anyway");
    }
    info!("Uploading {} bytes of AGPS data...", data.len());
    crate::protocol::upload_agps(&mut port, &data)?;
    Ok(())
}
