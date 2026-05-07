use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        let raw = crate::protocol::get_waypoint(&mut port)?;
        let wp = crate::decoder::decode_waypoint(&raw)?;

        if wp.text1.is_empty() && wp.text2.is_empty() {
            println!("No waypoint set.");
        } else {
            if !wp.text1.is_empty() {
                println!("Name:      {}", wp.text1);
            }
            if !wp.text2.is_empty() {
                println!("Label:     {}", wp.text2);
            }
            println!("Latitude:  {:.6}", wp.lat);
            println!("Longitude: {:.6}", wp.lon);
        }

        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}
