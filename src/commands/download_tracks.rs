use anyhow::{Context, Result};
use tracing::{error, info};

pub async fn run(port_arg: Option<String>, output_dir: &str) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let output_dir = output_dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::load_eeprom(&mut port)?;

        let meta = crate::protocol::get_log_header_count(&mut port)?;
        info!("Found {} track(s) on device", meta.count);
        if meta.count == 0 {
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_millis(1500));

        let header_bytes = crate::protocol::get_log_headers(&mut port, meta.count)?;
        std::fs::create_dir_all(&output_dir)?;

        for i in 0..meta.count as usize {
            let h_slice = &header_bytes[i * 65..(i + 1) * 65];
            let header = match crate::decoder::decode_log_header(h_slice) {
                Ok(h) => h,
                Err(e) => {
                    error!("Track {}: failed to decode header: {e:#}", i + 1);
                    continue;
                }
            };

            info!(
                "Track {}/{}: {} — {:.1} km",
                i + 1,
                meta.count,
                header.start_date.format("%Y-%m-%d %H:%M"),
                header.distance_m as f64 / 1000.0
            );

            let data =
                match crate::protocol::get_log_data(&mut port, header.start_addr, header.stop_addr)
                {
                    Ok(d) => d,
                    Err(e) => {
                        error!("Track {}: failed to read log data: {e:#}", i + 1);
                        continue;
                    }
                };

            let points = crate::decoder::decode_log_data(&data);
            info!("  {} track points decoded", points.len());

            let filename = crate::gpx::track_filename(&header, i);
            let path = std::path::Path::new(&output_dir).join(&filename);
            crate::gpx::write_gpx(&path, &header, &points)?;
            info!("  Saved to {}", path.display());
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("Download tracks task panicked")??;

    Ok(())
}
