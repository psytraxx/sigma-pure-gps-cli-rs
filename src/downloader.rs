use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use tracing::{debug, info, warn};

const MAX_AGPS_BYTES: usize = 32760;

const URL_OFFLINE_1: &str = "https://offline-live1.services.u-blox.com/GetOfflineData.ashx\
    ?token=UBLOX_TOKEN_REMOVED;gnss=gps;period=2;resolution=1";
const URL_OFFLINE_2: &str = "https://offline-live2.services.u-blox.com/GetOfflineData.ashx\
    ?token=UBLOX_TOKEN_REMOVED;gnss=gps;period=2;resolution=1";

pub struct AgpsData {
    pub bytes: Vec<u8>,
    #[allow(dead_code)]
    pub valid_until: Option<NaiveDate>,
}

pub async fn download(client: &reqwest::Client) -> Result<AgpsData> {
    let bytes = match try_download(client, URL_OFFLINE_1).await {
        Ok(b) => b,
        Err(e) => {
            warn!("Primary server failed ({e}), trying fallback");
            try_download(client, URL_OFFLINE_2)
                .await
                .context("Both u-blox AGPS servers failed")?
        }
    };

    let valid_until = decode_validity_date(&bytes);
    if let Some(d) = valid_until {
        info!("AGPS data valid until {d}");
    }

    Ok(AgpsData { bytes, valid_until })
}

async fn try_download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
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

// AGPS offline data encodes a validity date at bytes 10-12:
// year = data[10] + 2000, month = data[11] (1-based), day = data[12]
fn decode_validity_date(data: &[u8]) -> Option<NaiveDate> {
    if data.len() < 13 {
        return None;
    }
    let year = data[10] as i32 + 2000;
    let month = data[11] as u32;
    let day = data[12] as u32;
    NaiveDate::from_ymd_opt(year, month, day)
}
