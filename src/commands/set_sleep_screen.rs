use anyhow::Result;
use std::fs::File;
use std::io::BufReader;
use tracing::info;

pub async fn run(port_arg: Option<String>, input: &str) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let input = input.to_string();
    crate::util::run_blocking(move || {
        let file = File::open(&input)?;
        let reader = BufReader::new(file);
        let screen = crate::decoder::sleep_screen_from_png(reader)?;
        let payload = crate::decoder::encode_sleep_screen(&screen);

        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::set_sleep_screen(&mut port, &payload)?;

        println!("Sleep screen uploaded from: {input}");
        println!(
            "  Clock position: x={}, y={}",
            screen.clock_x, screen.clock_y
        );
        println!(
            "  Name position:  {}",
            if screen.name_bottom { "bottom" } else { "top" }
        );
        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}
