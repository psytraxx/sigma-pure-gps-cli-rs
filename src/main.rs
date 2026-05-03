mod commands;
mod decoder;
mod device;
mod downloader;
mod gpx;
mod protocol;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sigma-pure-gps-updater")]
#[command(about = "Manage the Sigma Pure GPS (Gps10) — update AGPS data and download tracks")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Serial port (e.g. COM3 on Windows, /dev/ttyACM0 on Linux).
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
    /// Download AGPS data and save it to a file (no device needed)
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
        Command::ListPorts => commands::list_ports::run(),
        Command::Download { output } => commands::download::run(&output).await,
        Command::Update => commands::update::run(cli.port).await,
        Command::ShowUnitInfo => commands::show_unit_info::run(cli.port).await,
        Command::DownloadTracks { output_dir } => {
            commands::download_tracks::run(cli.port, &output_dir).await
        }
    }
}
