mod commands;
mod decoder;
mod device;
mod downloader;
mod elevation;
mod gpx;
mod protocol;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sigma-pure-gps-cli")]
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
    DownloadAgps {
        #[arg(default_value = "agps.bin")]
        output: String,
    },
    /// List detected serial ports
    ListPorts,
    /// Show unit information from the connected device
    Info,
    /// Read device settings (timezone, language, units, contrast, …)
    GetSettings,
    /// Read cumulative totals (distance, time, calories, climb)
    GetTotals,
    /// Read the sleep screen / watch face bitmap from the device and save it as a PNG
    GetSleepScreen {
        /// Output PNG file path
        #[arg(default_value = "sleep_screen.png")]
        output: String,
    },
    /// Show the date of the AGPS data currently on the device
    AgpsDate,
    /// Download recorded tracks from the device, correct elevation via Sigma elevation service
    DownloadTracks {
        /// Directory to write GPX files into
        #[arg(default_value = ".")]
        output_dir: String,
    },
    /// Download recorded tracks from the device with raw barometric elevation (no correction)
    DownloadTracksRaw {
        /// Directory to write GPX files into
        #[arg(default_value = ".")]
        output_dir: String,
    },
    /// Permanently erase all activity data from the device (prompts for confirmation)
    DeleteTracks,
    /// Set home altitude 1 and/or home altitude 2 on the device
    SetHomeAltitude {
        /// Home altitude 1 in metres
        #[arg(long)]
        alt1: Option<i32>,
        /// Home altitude 2 in metres
        #[arg(long)]
        alt2: Option<i32>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

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
        Command::DownloadAgps { output } => commands::download_agps::run(&output).await,
        Command::Update => commands::update::run(cli.port).await,
        Command::Info => commands::info::run(cli.port).await,
        Command::GetSettings => commands::get_settings::run(cli.port).await,
        Command::GetTotals => commands::get_totals::run(cli.port).await,
        Command::GetSleepScreen { output } => {
            commands::get_sleep_screen::run(cli.port, &output).await
        }
        Command::AgpsDate => commands::agps_date::run(cli.port).await,
        Command::DownloadTracks { output_dir } => {
            commands::download_tracks::run(cli.port, &output_dir).await
        }
        Command::DownloadTracksRaw { output_dir } => {
            commands::download_tracks_raw::run(cli.port, &output_dir).await
        }
        Command::DeleteTracks => commands::delete_tracks::run(cli.port).await,
        Command::SetHomeAltitude { alt1, alt2 } => {
            commands::set_home_altitude::run(cli.port, alt1, alt2).await
        }
    }
}
