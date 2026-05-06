use anyhow::{Context, Result};
use tracing::{error, info};

use crate::decoder::{LogHeader, TrackPoint};

pub struct Track {
    pub header: LogHeader,
    pub points: Vec<TrackPoint>,
    pub index: usize,
}

/// Downloads all tracks from the device. Shared by download-tracks and download-tracks-raw.
pub fn download_from_device(port_name: &str) -> Result<Vec<Track>> {
    let mut port = crate::protocol::open_port(port_name)?;
    crate::protocol::load_unit_info(&mut port)?;
    crate::protocol::load_eeprom(&mut port)?;

    let meta = crate::protocol::get_log_header_count(&mut port)?;
    info!("Found {} track(s) on device", meta.count);
    if meta.count == 0 {
        return Ok(vec![]);
    }

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let header_bytes = crate::protocol::get_log_headers(&mut port, meta.count)?;
    let mut tracks = Vec::new();

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
            match crate::protocol::get_log_data(&mut port, header.start_addr, header.stop_addr) {
                Ok(d) => d,
                Err(e) => {
                    error!("Track {}: failed to read log data: {e:#}", i + 1);
                    continue;
                }
            };

        let points = crate::decoder::decode_log_data(&data);
        info!("  {} track points decoded", points.len());

        tracks.push(Track {
            header,
            points,
            index: i,
        });
    }

    Ok(tracks)
}

pub async fn run(port_arg: Option<String>, output_dir: &str) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let output_dir = output_dir.to_owned();
    let tracks = tokio::task::spawn_blocking(move || download_from_device(&port_name))
        .await
        .context("Download tracks task panicked")??;

    if tracks.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(&output_dir)?;

    let client = crate::util::build_http_client()?;
    for mut track in tracks {
        info!("  Correcting elevation via Sigma elevation service...");
        crate::elevation::correct_elevation(&client, &mut track.points).await?;

        let meta = crate::gpx::GpxMeta::from(&track.header);
        let filename = crate::gpx::track_filename(&meta, track.index);
        let path = std::path::Path::new(&output_dir).join(&filename);
        crate::gpx::write_gpx(&path, &meta, &track.points)?;
        info!("  Saved to {}", path.display());
    }

    Ok(())
}
