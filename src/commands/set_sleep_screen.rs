use anyhow::Result;
use std::fs::File;
use std::io::BufReader;

use crate::{decoder, protocol, util};

pub async fn run(port_arg: Option<String>, input: &str) -> Result<()> {
    let input = input.to_string();
    util::with_device(port_arg, move |port| {
        let file = File::open(&input)?;
        let reader = BufReader::new(file);
        let screen = decoder::sleep_screen_from_png(reader)?;
        let payload = decoder::encode_sleep_screen(&screen);

        protocol::set_sleep_screen(port, &payload)?;

        println!("Sleep screen uploaded from: {input}");
        println!(
            "  Clock position: x={}, y={}",
            screen.clock_x, screen.clock_y
        );
        println!(
            "  Name position:  {}",
            if screen.name_bottom { "bottom" } else { "top" }
        );
        Ok(())
    })
    .await
}
