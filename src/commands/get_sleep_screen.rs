use anyhow::Result;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::{decoder, protocol, util};

pub async fn run(port_arg: Option<String>, output: &str) -> Result<()> {
    let output = output.to_string();
    util::with_device(port_arg, move |port| {
        let raw = protocol::get_sleep_screen(port)?;
        let screen = decoder::decode_sleep_screen(&raw)?;

        if !screen.active {
            println!("Sleep screen: none (not configured)");
            return Ok(());
        }

        println!("Sleep screen: active");
        println!("Clock position: x={}, y={}", screen.clock_x, screen.clock_y);
        println!(
            "Name position:  {}",
            if screen.name_bottom { "bottom" } else { "top" }
        );

        let path = Path::new(&output);
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        decoder::sleep_screen_to_png(&screen, writer)?;
        println!("Bitmap saved to: {output}");

        Ok(())
    })
    .await
}
