use anyhow::{Result, bail};
use tracing::info;

use crate::{decoder, protocol, util};

pub async fn run(
    port_arg: Option<String>,
    text1: String,
    text2: String,
    lat: f64,
    lon: f64,
) -> Result<()> {
    if !(-90.0..=90.0).contains(&lat) {
        bail!("Latitude must be between -90 and 90");
    }
    if !(-180.0..=180.0).contains(&lon) {
        bail!("Longitude must be between -180 and 180");
    }

    info!("Setting waypoint: \"{text1}\" / \"{text2}\" at {lat:.6},{lon:.6}");

    util::with_device(port_arg, move |port| {
        let wp = decoder::Waypoint {
            text1: text1.clone(),
            text2,
            lat,
            lon,
        };
        let payload = decoder::encode_waypoint(&wp)?;
        protocol::set_waypoint(port, &payload)?;
        println!("Waypoint set: \"{text1}\" at {lat:.6},{lon:.6}");
        Ok(())
    })
    .await
}
