use anyhow::{Context, Result};
use tokio::task::spawn_blocking;
use tracing::info;

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
            crate::device::find_sigma_port()
        }
    }
}

pub fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("sigma-pure-gps-cli/0.1")
        .build()
        .context("Failed to build HTTP client")
}
