mod decoder;
mod device;
mod downloader;
mod gpx;
mod protocol;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "sigma-pure-gps-updater")]
#[command(about = "Manage the Sigma Pure GPS (Gps10) — update AGPS data and download tracks")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Serial port to use (e.g. COM3 on Windows, /dev/ttyACM0 on Linux).
    /// Auto-detected by USB VID 0x1D9D if omitted.
    #[arg(short, long)]
    port: Option<String>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Download AGPS data and upload it to the connected device
    Update,
    /// Only download AGPS data and save it to a file
    Download {
        #[arg(default_value = "agps.bin")]
        output: String,
    },
    /// List detected serial ports
    ListPorts,
    /// Show unit information from the connected device
    ShowUnitInfo,
    /// Download recorded tracks from the device and save as GPX files
    DownloadTracks {
        /// Directory to write GPX files into
        #[arg(default_value = ".")]
        output_dir: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .without_time()
        .init();

    match cli.command {
        Command::ListPorts => list_ports(),
        Command::Download { output } => cmd_download(&output).await,
        Command::Update => cmd_update(cli.port).await,
        Command::ShowUnitInfo => cmd_show_unit_info(cli.port).await,
        Command::DownloadTracks { output_dir } => cmd_download_tracks(cli.port, &output_dir).await,
    }
}

fn list_ports() -> Result<()> {
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

async fn cmd_download(output: &str) -> Result<()> {
    let client = build_http_client()?;
    info!("Downloading AGPS data from u-blox...");
    let agps = downloader::download(&client).await?;
    info!("Downloaded {} bytes", agps.bytes.len());
    tokio::fs::write(output, &agps.bytes)
        .await
        .with_context(|| format!("Failed to write {output}"))?;
    info!("Saved to {output}");
    Ok(())
}

async fn cmd_update(port_arg: Option<String>) -> Result<()> {
    // ── 1. Download AGPS data ──────────────────────────────────────────────
    let client = build_http_client()?;
    info!("Downloading AGPS data from u-blox...");
    let agps = downloader::download(&client).await?;
    info!("Downloaded {} bytes", agps.bytes.len());

    // ── 2. Find device ─────────────────────────────────────────────────────
    let port_name = match port_arg {
        Some(p) => p,
        None => {
            info!("Auto-detecting SIGMA device...");
            device::find_sigma_port()?
        }
    };
    info!("Using port: {port_name}");

    // ── 3. Upload ──────────────────────────────────────────────────────────
    // Spawn blocking task so serial I/O doesn't block the tokio runtime
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
    let mut port = protocol::open_port(&port_name)?;

    info!("Checking device connection...");
    if !protocol::check_device_connected(&mut port)? {
        // The device may not respond to the poll command in all states;
        // attempt the upload anyway if detection is ambiguous.
        info!("Device poll did not confirm connection — attempting upload anyway");
    }

    info!("Uploading {} bytes of AGPS data...", data.len());
    protocol::upload_agps(&mut port, &data)?;
    Ok(())
}

async fn cmd_show_unit_info(port_arg: Option<String>) -> Result<()> {
    let port_name = match port_arg {
        Some(p) => p,
        None => {
            info!("Auto-detecting SIGMA device...");
            device::find_sigma_port()?
        }
    };
    info!("Using port: {port_name}");

    tokio::task::spawn_blocking(move || {
        let mut port = protocol::open_port(&port_name)?;
        let raw = protocol::load_unit_info(&mut port)?;
        protocol::print_unit_info(&raw);
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("Unit info task panicked")??;

    Ok(())
}

async fn cmd_download_tracks(port_arg: Option<String>, output_dir: &str) -> Result<()> {
    let port_name = match port_arg {
        Some(p) => p,
        None => {
            info!("Auto-detecting SIGMA device...");
            device::find_sigma_port()?
        }
    };
    info!("Using port: {port_name}");

    let output_dir = output_dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut port = protocol::open_port(&port_name)?;

        let count = protocol::get_log_header_count(&mut port)?;
        info!("Found {count} track(s) on device");
        if count == 0 {
            return Ok(());
        }

        let header_bytes = protocol::get_log_headers(&mut port, count)?;

        std::fs::create_dir_all(&output_dir)?;

        for i in 0..count as usize {
            let h_slice = &header_bytes[i * 65..(i + 1) * 65];
            let header = match decoder::decode_log_header(h_slice) {
                Ok(h) => h,
                Err(e) => {
                    error!("Track {}: failed to decode header: {e:#}", i + 1);
                    continue;
                }
            };

            info!(
                "Track {}/{}: {} — {:.1} km",
                i + 1,
                count,
                header.start_date.format("%Y-%m-%d %H:%M"),
                header.distance_m as f64 / 1000.0
            );

            let data = match protocol::get_log_data(&mut port, header.start_addr, header.stop_addr)
            {
                Ok(d) => d,
                Err(e) => {
                    error!("Track {}: failed to read log data: {e:#}", i + 1);
                    continue;
                }
            };

            let points = decoder::decode_log_data(&data);
            info!("  {} track points decoded", points.len());

            let filename = gpx::track_filename(&header, i);
            let path = std::path::Path::new(&output_dir).join(&filename);
            gpx::write_gpx(&path, &header, &points)?;
            info!("  Saved to {}", path.display());
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("Download tracks task panicked")??;

    Ok(())
}

fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("sigma-pure-gps-updater/0.1")
        .build()
        .context("Failed to build HTTP client")
}
