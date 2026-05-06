use anyhow::Result;
use tracing::info;

pub async fn run(port_arg: Option<String>) -> Result<()> {
    let port_name = crate::util::resolve_port(port_arg)?;
    info!("Using port: {port_name}");

    crate::util::run_blocking(move || {
        let mut port = crate::protocol::open_port(&port_name)?;
        let raw = crate::protocol::load_unit_info(&mut port)?;
        crate::protocol::print_unit_info(&raw);
        Ok(())
    })
    .await?;

    Ok(())
}
