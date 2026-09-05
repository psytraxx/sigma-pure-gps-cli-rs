use anyhow::Result;

use crate::{decoder, protocol, util};

pub async fn run(port_arg: Option<String>) -> Result<()> {
    util::with_device(port_arg, |port| {
        protocol::load_eeprom(port)?;
        let data = protocol::get_agps_flash_header(port)?;
        let date = decoder::decode_agps_date(&data)?;
        println!("AGPS data date: {}", date.format("%Y-%m-%d"));
        Ok(())
    })
    .await
}
