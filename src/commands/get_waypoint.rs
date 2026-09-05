use anyhow::Result;

use crate::{decoder, protocol, util};

pub async fn run(port_arg: Option<String>) -> Result<()> {
    util::with_device(port_arg, |port| {
        let raw = protocol::get_waypoint(port)?;
        let wp = decoder::decode_waypoint(&raw)?;

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

        Ok(())
    })
    .await
}
