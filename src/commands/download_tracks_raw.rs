use anyhow::Result;

pub async fn run(port_arg: Option<String>, output_dir: &str) -> Result<()> {
    super::download_tracks::run_with_options(port_arg, output_dir, false).await
}
