use anyhow::Result;
use std::io::{self, Write};
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    print!("This will permanently erase all activity data on the device. Continue? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::delete_tracks_memory(&mut port)?;
        println!("Activity memory erased.");
        Ok(())
    })
    .await?;

    Ok(())
}
