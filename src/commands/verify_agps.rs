use anyhow::{Context, Result, bail};
use tracing::info;

use crate::{protocol, util};

const MAX_AGPS_BYTES: usize = 32760;

pub async fn run(port_arg: Option<String>, file: Option<String>) -> Result<()> {
    let mut expected = match &file {
        Some(path) => tokio::fs::read(path)
            .await
            .with_context(|| format!("Failed to read {path}"))?,
        None => {
            info!("No file given, downloading fresh AGPS data from u-blox...");
            info!("(If u-blox has published newer data since the upload, a mismatch is expected)");
            let client = util::build_http_client()?;
            util::download_agps(&client).await?.bytes
        }
    };
    expected.truncate(MAX_AGPS_BYTES);
    if expected.is_empty() {
        bail!("Reference AGPS data is empty");
    }

    let len = expected.len();
    info!("Reading {len} bytes of AGPS data from device flash...");
    let actual = util::with_device(port_arg, move |port| {
        protocol::load_eeprom(port)?;
        protocol::read_agps_flash(port, len)
    })
    .await?;

    let cmp = compare(&expected, &actual);
    if cmp.differing == 0 {
        println!("AGPS data on device matches ({len} bytes).");
        return Ok(());
    }
    println!(
        "AGPS data on device does NOT match: {} of {len} bytes differ.",
        cmp.differing
    );
    println!(
        "First {} bytes match; first mismatch at offset {} (0x{:X}).",
        cmp.matching_prefix, cmp.matching_prefix, cmp.matching_prefix
    );
    bail!("AGPS verification failed")
}

#[derive(Debug, PartialEq)]
struct Comparison {
    matching_prefix: usize,
    differing: usize,
}

fn compare(expected: &[u8], actual: &[u8]) -> Comparison {
    let matching_prefix = expected
        .iter()
        .zip(actual)
        .take_while(|(a, b)| a == b)
        .count();
    let differing = expected.iter().zip(actual).filter(|(a, b)| a != b).count()
        + expected.len().abs_diff(actual.len());
    Comparison {
        matching_prefix,
        differing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_data_matches() {
        let data = [1u8, 2, 3, 4];
        assert_eq!(
            compare(&data, &data),
            Comparison {
                matching_prefix: 4,
                differing: 0
            }
        );
    }

    #[test]
    fn partial_write_reports_prefix() {
        // Typical partial flash write: tail left erased (0xFF).
        let expected = [1u8, 2, 3, 4, 5, 6];
        let actual = [1u8, 2, 3, 0xFF, 0xFF, 0xFF];
        assert_eq!(
            compare(&expected, &actual),
            Comparison {
                matching_prefix: 3,
                differing: 3
            }
        );
    }

    #[test]
    fn length_difference_counts_as_differing() {
        assert_eq!(
            compare(&[1, 2, 3], &[1, 2]),
            Comparison {
                matching_prefix: 2,
                differing: 1
            }
        );
    }
}
