use anyhow::{Result, bail};
use tracing::info;

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

    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");
    info!("Setting waypoint: \"{text1}\" / \"{text2}\" at {lat:.6},{lon:.6}");

    crate::util::run_blocking(move || {
        let wp = crate::decoder::Waypoint {
            text1: text1.clone(),
            text2,
            lat,
            lon,
        };
        let payload = crate::decoder::encode_waypoint(&wp)?;
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::set_waypoint(&mut port, &payload)?;
        println!("Waypoint set: \"{text1}\" at {lat:.6},{lon:.6}");
        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}
