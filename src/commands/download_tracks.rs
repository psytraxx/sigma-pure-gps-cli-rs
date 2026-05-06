use anyhow::Result;
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

    // The device needs ~1.5 s after CMD_GET_LOG_HEADER_COUNT before it will respond to
    // CMD_GET_LOG_HEADERS. Without this delay the header read times out. Value determined
    // empirically by the original ActionScript implementation (Gps10Handler.as).
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
    let tracks = crate::util::run_blocking(move || download_from_device(&port_name)).await?;

    if tracks.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(&output_dir)?;

    let client = crate::util::build_http_client()?;

    // Correct elevation for all tracks concurrently — each track is one HTTP request,
    // so running them in parallel cuts total wait time from N×latency to ~1×latency.
    let mut join_set: tokio::task::JoinSet<Result<Track>> = tokio::task::JoinSet::new();
    for mut track in tracks {
        let client = client.clone();
        join_set.spawn(async move {
            info!("  Correcting elevation via Sigma elevation service...");
            crate::elevation::correct_elevation(&client, &mut track.points).await?;
            Ok(track)
        });
    }

    // Collect in completion order; abort all remaining tasks on first error.
    let mut corrected: Vec<Track> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(track)) => corrected.push(track),
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(anyhow::anyhow!("Elevation correction task panicked: {e}")),
        }
    }

    // Sort by original index so output files are written in track order.
    corrected.sort_by_key(|t| t.index);

    for track in corrected {
        let meta = crate::gpx::GpxMeta::from(&track.header);
        let filename = crate::gpx::track_filename(&meta, track.index);
        let path = std::path::Path::new(&output_dir).join(&filename);
        crate::gpx::write_gpx(&path, &meta, &track.points)?;
        info!("  Saved to {}", path.display());
    }

    Ok(())
}
