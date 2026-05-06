use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        let raw = crate::protocol::get_settings(&mut port)?;
        let s = crate::decoder::decode_settings(&raw)?;

        println!("Language:          {}", s.language);
        println!("Clock:             {}-hour", s.clock_mode);
        println!("Time zone:         {}", s.time_zone);
        println!(
            "Summer time:       {}",
            if s.summer_time { "on" } else { "off" }
        );
        println!("Date format:       {}", s.date_format);
        println!("Speed unit:        {}", s.speed_unit);
        println!("Temperature unit:  {}", s.temperature_unit);
        println!("Altitude unit:     {}", s.altitude_unit);
        println!("Altitude ref:      {}", s.altitude_reference);
        println!("Actual altitude:   {} m", s.actual_altitude_m);
        println!("Sea level press.:  {:.1} mb", s.sea_level_mb);
        println!("Home altitude 1:   {} m", s.home_altitude1_m);
        println!("Home altitude 2:   {} m", s.home_altitude2_m);
        println!("Contrast:          {}/4", s.contrast);
        println!(
            "System tone:       {}",
            if s.system_tone { "on" } else { "off" }
        );
        println!(
            "NFC:               {}",
            if s.nfc_active { "on" } else { "off" }
        );
        println!(
            "Auto pause:        {}",
            if s.auto_pause { "on" } else { "off" }
        );
        println!("Auto lap distance: {} m", s.auto_lap_distance_m);
        if !s.name.is_empty() {
            println!("Name:              {}", s.name);
        }

        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}
