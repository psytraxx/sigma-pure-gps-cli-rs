use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>, output_dir: &str) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let output_dir = output_dir.to_owned();
    let tracks =
        crate::util::run_blocking(move || super::download_tracks::download_from_device(&port_name))
            .await?;

    if tracks.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(&output_dir)?;

    for track in tracks {
        let meta = crate::gpx::GpxMeta::from(&track.header);
        let filename = crate::gpx::track_filename(&meta, track.index);
        let path = std::path::Path::new(&output_dir).join(&filename);
        crate::gpx::write_gpx(&path, &meta, &track.points)?;
        info!("  Saved to {}", path.display());
    }

    Ok(())
}
