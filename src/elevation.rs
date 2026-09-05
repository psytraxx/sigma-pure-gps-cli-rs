use anyhow::{Context, Result, bail};
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

    apply_elevations(points, &resp.track.coordinates)
}

/// Applies elevation results to non-pause points in order. The API is sent an ordered
/// `LineString` with `intermediates: 0`, so the response coordinates come back in the same
/// order as the request, one per non-pause point — pairing by index rather than by
/// re-formatting coordinates as lookup keys avoids float-formatting mismatches.
fn apply_elevations(points: &mut [TrackPoint], coordinates: &[[f64; 3]]) -> Result<()> {
    let expected = points.iter().filter(|p| !p.is_pause).count();
    if coordinates.len() != expected {
        bail!(
            "Elevation API returned {} coordinate(s), expected {} (one per non-pause point)",
            coordinates.len(),
            expected
        );
    }

    let mut results = coordinates.iter();
    for pt in points.iter_mut() {
        if pt.is_pause {
            continue;
        }
        // Length was just verified equal to the non-pause point count, so this always yields.
        let [_, _, elev_m] = results.next().expect("coordinate count already verified");
        pt.altitude_m = *elev_m;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(is_pause: bool) -> TrackPoint {
        TrackPoint {
            latitude: 47.0,
            longitude: 8.0,
            altitude_m: 0.0,
            speed_ms: 0.0,
            temperature_c: 0,
            training_time_ms: 0,
            is_pause,
        }
    }

    #[test]
    fn applies_elevations_to_non_pause_points_in_order() {
        let mut points = vec![pt(false), pt(true), pt(false), pt(false)];
        let coordinates = vec![[8.0, 47.0, 100.0], [8.0, 47.0, 200.0], [8.0, 47.0, 300.0]];

        apply_elevations(&mut points, &coordinates).unwrap();

        assert_eq!(points[0].altitude_m, 100.0);
        assert_eq!(points[1].altitude_m, 0.0); // pause point untouched
        assert_eq!(points[2].altitude_m, 200.0);
        assert_eq!(points[3].altitude_m, 300.0);
    }

    #[test]
    fn errors_on_coordinate_count_mismatch() {
        let mut points = vec![pt(false), pt(false)];
        let coordinates = vec![[8.0, 47.0, 100.0]];

        let err = apply_elevations(&mut points, &coordinates).unwrap_err();
        assert!(err.to_string().contains("expected 2"));
    }

    #[test]
    fn no_points_to_correct_is_ok() {
        let mut points: Vec<TrackPoint> = vec![];
        assert!(apply_elevations(&mut points, &[]).is_ok());
    }

    #[test]
    fn all_pause_points_expects_zero_coordinates() {
        let mut points = vec![pt(true), pt(true)];
        assert!(apply_elevations(&mut points, &[]).is_ok());
        assert_eq!(points[0].altitude_m, 0.0);
    }
}
