use anyhow::{Result, bail};
use tracing::info;

use crate::{protocol, util};

/// Encoding is `raw = altitude_m * 10 + 10000` packed into a u16 (see
/// `protocol::set_home_altitude`), so this is the widest range of metres that round-trips
/// without over/underflowing the 16-bit field.
const MIN_ALTITUDE_M: i32 = -1000;
const MAX_ALTITUDE_M: i32 = 5553;

pub async fn run(port_arg: Option<String>, alt1_m: Option<i32>, alt2_m: Option<i32>) -> Result<()> {
    if alt1_m.is_none() && alt2_m.is_none() {
        bail!("Provide at least one of --alt1 or --alt2");
    }
    for m in [alt1_m, alt2_m].into_iter().flatten() {
        if !(MIN_ALTITUDE_M..=MAX_ALTITUDE_M).contains(&m) {
            bail!("Altitude must be between {MIN_ALTITUDE_M} and {MAX_ALTITUDE_M} m, got {m}");
        }
    }

    if let Some(m) = alt1_m {
        info!("Setting home altitude 1 to {m} m");
    }
    if let Some(m) = alt2_m {
        info!("Setting home altitude 2 to {m} m");
    }

    util::with_device(port_arg, move |port| {
        protocol::set_home_altitude(port, alt1_m, alt2_m)?;
        match (alt1_m, alt2_m) {
            (Some(a), Some(b)) => {
                println!("Home altitude 1 set to {a} m, home altitude 2 set to {b} m.")
            }
            (Some(a), None) => println!("Home altitude 1 set to {a} m."),
            (None, Some(b)) => println!("Home altitude 2 set to {b} m."),
            (None, None) => unreachable!(),
        }
        Ok(())
    })
    .await
}
