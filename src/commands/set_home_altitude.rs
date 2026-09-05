use anyhow::{Result, bail};
use tracing::info;

use crate::{protocol, util};

pub async fn run(port_arg: Option<String>, alt1_m: Option<i32>, alt2_m: Option<i32>) -> Result<()> {
    if alt1_m.is_none() && alt2_m.is_none() {
        bail!("Provide at least one of --alt1 or --alt2");
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
