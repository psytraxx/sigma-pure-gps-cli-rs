use anyhow::Result;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tracing::info;

pub async fn run(port_arg: Option<String>, output: &str) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    let output = output.to_string();
    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        let raw = crate::protocol::get_sleep_screen(&mut port)?;
        let screen = crate::decoder::decode_sleep_screen(&raw)?;

        if !screen.active {
            println!("Sleep screen: none (not configured)");
            return Ok::<_, anyhow::Error>(());
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
        crate::decoder::sleep_screen_to_png(&screen, writer)?;
        println!("Bitmap saved to: {output}");

        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}
