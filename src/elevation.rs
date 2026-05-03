use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::decoder::TrackPoint;

const API_URL: &str = "https://elevation.sigma-dc-control.com/elevation.php";

#[derive(Serialize)]
struct ElevationRequest {
    intermediates: u8,
    track: GeoJsonLineString,
}

#[derive(Serialize)]
struct GeoJsonLineString {
    #[serde(rename = "type")]
    kind: &'static str,
    coordinates: Vec<[f64; 2]>,
}

#[derive(Deserialize)]
struct ElevationResponse {
    track: ElevationTrack,
}

#[derive(Deserialize)]
struct ElevationTrack {
    coordinates: Vec<[f64; 3]>,
}

/// Fetches DEM elevation for all points and replaces altitude_m in-place.
/// Uses Sigma's elevation service (single POST, all coordinates, elevation in mm).
pub async fn correct_elevation(client: &reqwest::Client, points: &mut [TrackPoint]) -> Result<()> {
    let coords: Vec<[f64; 2]> = points
        .iter()
        .filter(|p| !p.is_pause)
        .map(|p| {
            [
                (p.longitude * 100000.0).round() / 100000.0,
                (p.latitude * 100000.0).round() / 100000.0,
            ]
        })
        .collect();

    if coords.is_empty() {
        return Ok(());
    }

    let body = ElevationRequest {
        intermediates: 0,
        track: GeoJsonLineString {
            kind: "LineString",
            coordinates: coords,
        },
    };

    let resp = client
        .post(API_URL)
        .json(&body)
        .send()
        .await
        .context("Elevation API request failed")?
        .error_for_status()
        .context("Elevation API returned error status")?
        .json::<ElevationResponse>()
        .await
        .context("Failed to parse elevation API response")?;

    // Build lookup: "lon,lat" -> elevation_m (response stores mm)
    use std::collections::HashMap;
    let lookup: HashMap<String, f64> = resp
        .track
        .coordinates
        .iter()
        .map(|c| (format!("{},{}", c[0], c[1]), c[2]))
        .collect();

    for pt in points.iter_mut() {
        if pt.is_pause {
            continue;
        }
        let key = format!(
            "{},{}",
            (pt.longitude * 100000.0).round() / 100000.0,
            (pt.latitude * 100000.0).round() / 100000.0
        );
        if let Some(&elev) = lookup.get(&key) {
            pt.altitude_m = elev;
        }
    }

    Ok(())
}
