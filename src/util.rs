use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serialport::{SerialPort, SerialPortInfo};
use tokio::task::spawn_blocking;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Device detection
// ---------------------------------------------------------------------------

const SIGMA_USB_VID: u16 = 0x1D9D;

pub fn find_sigma_port() -> Result<String> {
    let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;
    debug!("Found {} serial port(s)", ports.len());
    if let Some(port) = ports.iter().find(|p| is_sigma_port(p)) {
        let name = port.port_name.clone();
        debug!("Found SIGMA device on {name} (VID match)");
        return Ok(name);
    }
    bail!(
        "No SIGMA device found. Is the Pure GPS connected via USB?\nAvailable ports: {}",
        ports
            .iter()
            .map(|p| p.port_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn is_sigma_port(port: &SerialPortInfo) -> bool {
    match &port.port_type {
        serialport::SerialPortType::UsbPort(info) => {
            debug!(
                "  {} — VID:{:04X} PID:{:04X} ({})",
                port.port_name,
                info.vid,
                info.pid,
                info.product.as_deref().unwrap_or("unknown")
            );
            info.vid == SIGMA_USB_VID
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// AGPS downloader
// ---------------------------------------------------------------------------

const MAX_AGPS_BYTES: usize = 32760;
const URL_OFFLINE_1: &str = "https://offline-live1.services.u-blox.com/GetOfflineData.ashx";
const URL_OFFLINE_2: &str = "https://offline-live2.services.u-blox.com/GetOfflineData.ashx";

pub struct AgpsData {
    pub bytes: Vec<u8>,
}

pub async fn download_agps(client: &reqwest::Client) -> Result<AgpsData> {
    let token = std::env::var("UBLOX_AGPS_TOKEN")
        .context("UBLOX_AGPS_TOKEN environment variable not set (add it to .env or export it)")?;
    let url1 = format!("{URL_OFFLINE_1}?token={token};gnss=gps;period=2;resolution=1");
    let url2 = format!("{URL_OFFLINE_2}?token={token};gnss=gps;period=2;resolution=1");
    let bytes = match try_download_agps(client, &url1).await {
        Ok(b) => b,
        Err(e) => {
            warn!("Primary server failed ({e}), trying fallback");
            try_download_agps(client, &url2)
                .await
                .context("Both u-blox AGPS servers failed")?
        }
    };
    if let Some(d) = decode_agps_validity_date(&bytes) {
        info!("AGPS data valid until {d}");
    }
    Ok(AgpsData { bytes })
}

async fn try_download_agps(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    debug!("Downloading AGPS data from {url}");
    let resp = client
        .get(url)
        .send()
        .await
        .context("HTTP request failed")?;
    if !resp.status().is_success() {
        bail!("Server returned {}", resp.status());
    }
    let bytes = resp.bytes().await.context("Reading response body")?;
    if bytes.is_empty() {
        bail!("Empty response");
    }
    let truncated = bytes.len().min(MAX_AGPS_BYTES);
    debug!("Downloaded {} bytes (using {})", bytes.len(), truncated);
    Ok(bytes[..truncated].to_vec())
}

fn decode_agps_validity_date(data: &[u8]) -> Option<NaiveDate> {
    if data.len() < 13 {
        return None;
    }
    NaiveDate::from_ymd_opt(data[10] as i32 + 2000, data[11] as u32, data[12] as u32)
}

/// Runs a blocking closure on Tokio's blocking thread pool and propagates both
/// panics (as errors) and the closure's own `Result`.
///
/// `spawn_blocking` requires a `'static` closure — callers must `.to_owned()` any
/// borrowed data before moving it in.
pub async fn run_blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f).await.context("Blocking task panicked")?
}

pub fn resolve_port(port_arg: Option<String>) -> Result<String> {
    match port_arg {
        Some(p) => Ok(p),
        None => {
            info!("Auto-detecting SIGMA device...");
            find_sigma_port()
        }
    }
}

pub fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("sigma-pure-gps-cli/0.1")
        .build()
        .context("Failed to build HTTP client")
}

/// Resolves the port, opens it, loads unit info (required before any other device command),
/// then runs `f` against the open port — all on Tokio's blocking pool.
///
/// This is the shared preamble used by most subcommands: resolve → log → open →
/// load_unit_info → do the actual work. Commands with extra setup before opening the port
/// (e.g. `update`, `download-tracks`) call `resolve_port`/`run_blocking` directly instead.
pub async fn with_device<T, F>(port_arg: Option<String>, f: F) -> Result<T>
where
    F: FnOnce(&mut Box<dyn SerialPort>) -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let port_name = resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        f(&mut port)
    })
    .await
}
