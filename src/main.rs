mod device;
mod downloader;
mod protocol;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "sigma-pure-gps-updater")]
#[command(about = "Update AGPS satellite prediction data on the Sigma Pure GPS (Gps10)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

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
    /// Download AGPS data and upload it to the connected device (default)
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

    let command = cli.command.unwrap_or(Command::Update);

    match command {
        Command::ListPorts => list_ports(),
        Command::Download { output } => cmd_download(&output).await,
        Command::Update => cmd_update(cli.port).await,
        Command::ShowUnitInfo => cmd_show_unit_info(cli.port).await,
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

fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("sigma-pure-gps-updater/0.1")
        .build()
        .context("Failed to build HTTP client")
}
