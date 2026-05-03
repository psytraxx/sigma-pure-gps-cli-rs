use crate::decoder::{LogHeader, TrackPoint};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::fmt::Write as FmtWrite;
use std::path::Path;

pub fn write_gpx(path: &Path, header: &LogHeader, points: &[TrackPoint]) -> Result<()> {
    let mut out = String::new();

    writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        out,
        r#"<gpx version="1.1" creator="sigma-pure-gps-updater""#
    )?;
    writeln!(out, r#"  xmlns="http://www.topografix.com/GPX/1/1""#)?;
    writeln!(
        out,
        r#"  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    )?;
    writeln!(
        out,
        r#"  xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">"#
    )?;

    let track_name = header.start_date.format("%Y-%m-%d %H:%M:%S UTC");
    writeln!(out, "  <trk>")?;
    writeln!(out, "    <name>{track_name}</name>")?;
    writeln!(out, "    <trkseg>")?;

    for pt in points {
        if pt.is_pause {
            continue;
        }
        let timestamp: DateTime<Utc> =
            header.start_date + Duration::milliseconds(pt.training_time_ms as i64);

        writeln!(
            out,
            r#"      <trkpt lat="{:.7}" lon="{:.7}">"#,
            pt.latitude, pt.longitude
        )?;
        writeln!(out, "        <ele>{:.1}</ele>", pt.altitude_m)?;
        writeln!(
            out,
            "        <time>{}</time>",
            timestamp.format("%Y-%m-%dT%H:%M:%SZ")
        )?;
        writeln!(out, "        <extensions>")?;
        writeln!(out, "          <speed>{:.3}</speed>", pt.speed_ms)?;
        writeln!(
            out,
            "          <temperature>{}</temperature>",
            pt.temperature_c
        )?;
        writeln!(out, "        </extensions>")?;
        writeln!(out, "      </trkpt>")?;
    }

    writeln!(out, "    </trkseg>")?;
    writeln!(out, "  </trk>")?;
    writeln!(out, "</gpx>")?;

    std::fs::write(path, out)?;
    Ok(())
}

pub fn track_filename(header: &LogHeader, index: usize) -> String {
    format!(
        "track_{:03}_{}.gpx",
        index + 1,
        header.start_date.format("%Y%m%d_%H%M%S")
    )
}
