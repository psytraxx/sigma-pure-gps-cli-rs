use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        crate::protocol::load_unit_info(&mut port)?;
        let raw = crate::protocol::get_totals(&mut port)?;
        let t = crate::decoder::decode_totals(&raw)?;

        let total_secs = t.total_training_time_ms / 1000;
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;

        println!(
            "Total distance:    {} km",
            fmt_thousands(t.total_distance_km, 3)
        );
        println!(
            "Total time:        {}:{:02}:{:02} (h:m:s)",
            fmt_thousands_int(h),
            m,
            s
        );
        println!(
            "Total calories:    {} kcal",
            fmt_thousands_int(t.total_calories_kcal as u64)
        );
        println!("Total climb:       {} m", fmt_thousands(t.total_climb_m, 3));
        if let Some(d) = t.reset_date {
            println!("Totals reset:      {}", d.format("%Y-%m-%d"));
        }

        Ok::<_, anyhow::Error>(())
    })
    .await?;

    Ok(())
}

/// Format a float with dot-thousands and comma-decimal, e.g. 19359.28 → "19.359,280"
fn fmt_thousands(val: f64, decimals: usize) -> String {
    let factor = 10f64.powi(decimals as i32);
    let integer = val.trunc() as u64;
    let frac = ((val.fract().abs() * factor).round()) as u64;
    format!(
        "{},{:0>width$}",
        fmt_thousands_int(integer),
        frac,
        width = decimals
    )
}

/// Format an integer with dot-thousands separator, e.g. 19359 → "19.359"
fn fmt_thousands_int(val: u64) -> String {
    let s = val.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}
