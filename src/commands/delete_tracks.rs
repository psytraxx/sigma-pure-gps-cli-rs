use anyhow::Result;
use std::io::{self, Write};

use crate::{protocol, util};

pub async fn run(port_arg: Option<String>) -> Result<()> {
    print!("This will permanently erase all activity data on the device. Continue? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        println!("Aborted.");
        return Ok(());
    }

    util::with_device(port_arg, |port| {
        protocol::delete_tracks_memory(port)?;
        println!("Activity memory erased.");
        Ok(())
    })
    .await
}
