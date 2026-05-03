use anyhow::{Context, Result};
use tracing::info;

pub async fn run(output: &str) -> Result<()> {
    let client = crate::util::build_http_client()?;
    info!("Downloading AGPS data from u-blox...");
    let agps = crate::downloader::download(&client).await?;
    info!("Downloaded {} bytes", agps.bytes.len());
    tokio::fs::write(output, &agps.bytes)
        .await
        .with_context(|| format!("Failed to write {output}"))?;
    info!("Saved to {output}");
    Ok(())
}
