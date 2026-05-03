use anyhow::{Context, Result};
use tracing::info;

pub fn resolve_port(port_arg: Option<String>) -> Result<String> {
    match port_arg {
        Some(p) => Ok(p),
        None => {
            info!("Auto-detecting SIGMA device...");
            crate::device::find_sigma_port()
        }
    }
}

pub fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("sigma-pure-gps-updater/0.1")
        .build()
        .context("Failed to build HTTP client")
}
