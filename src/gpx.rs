use crate::decoder::{LogHeader, TrackPoint};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};
use std::io::BufWriter;
use std::path::Path;

/// Metadata for a GPX track, independent of the data source (device or database).
pub struct GpxMeta {
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub distance_m: Option<f64>,
    pub training_time_s: Option<f64>,
    pub avg_speed_kmh: Option<f64>,
    pub max_speed_kmh: Option<f64>,
    pub calories_kcal: Option<u32>,
}

impl From<&LogHeader> for GpxMeta {
    fn from(h: &LogHeader) -> Self {
        Self {
            name: h.start_date.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            start_date: h.start_date,
            distance_m: Some(h.distance_m as f64),
            training_time_s: Some(h.training_time_ms as f64 / 1000.0),
            avg_speed_kmh: Some(h.avg_speed_kmh),
            max_speed_kmh: Some(h.max_speed_kmh),
            calories_kcal: Some(h.calories_kcal),
        }
    }
}

pub fn write_gpx(path: &Path, meta: &GpxMeta, points: &[TrackPoint]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut w = Writer::new_with_indent(BufWriter::new(file), b' ', 2);

    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    w.create_element("gpx")
        .with_attribute(("version", "1.1"))
        .with_attribute(("creator", "sigma-pure-gps-cli"))
        .with_attribute(("xmlns", "http://www.topografix.com/GPX/1/1"))
        .with_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"))
        .with_attribute((
            "xsi:schemaLocation",
            "http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd",
        ))
        .write_inner_content(|w| {
            w.create_element("trk").write_inner_content(|w| {
                w.create_element("name")
                    .write_text_content(BytesText::new(&meta.name))?;

                if let Some(desc) = build_desc(meta) {
                    w.create_element("desc")
                        .write_text_content(BytesText::new(&desc))?;
                }

                w.create_element("trkseg").write_inner_content(|w| {
                    for pt in points {
                        if pt.is_pause {
                            continue;
                        }
                        let timestamp: DateTime<Utc> =
                            meta.start_date + Duration::milliseconds(pt.training_time_ms as i64);

                        let lat = format!("{:.7}", pt.latitude);
                        let lon = format!("{:.7}", pt.longitude);
                        w.create_element("trkpt")
                            .with_attribute(("lat", lat.as_str()))
                            .with_attribute(("lon", lon.as_str()))
                            .write_inner_content(|w| {
                                let ele = format!("{:.1}", pt.altitude_m);
                                w.create_element("ele")
                                    .write_text_content(BytesText::new(&ele))?;

                                let time = timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                                w.create_element("time")
                                    .write_text_content(BytesText::new(&time))?;

                                w.create_element("extensions").write_inner_content(|w| {
                                    let speed = format!("{:.3}", pt.speed_ms);
                                    w.create_element("speed")
                                        .write_text_content(BytesText::new(&speed))?;
                                    let temp = pt.temperature_c.to_string();
                                    w.create_element("temperature")
                                        .write_text_content(BytesText::new(&temp))?;
                                    Ok(())
                                })?;
                                Ok(())
                            })?;
                    }
                    Ok(())
                })?;
                Ok(())
            })?;
            Ok(())
        })?;

    Ok(())
}

pub fn track_filename(meta: &GpxMeta, index: usize) -> String {
    format!(
        "track_{:03}_{}.gpx",
        index + 1,
        meta.start_date.format("%Y%m%d_%H%M%S")
    )
}

fn build_desc(meta: &GpxMeta) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(d) = meta.distance_m {
        parts.push(format!("Distance: {:.2} km", d / 1000.0));
    }
    if let Some(t) = meta.training_time_s {
        parts.push(format!("Duration: {}", format_duration(t)));
    }
    if let Some(avg) = meta.avg_speed_kmh {
        parts.push(format!("Avg: {:.1} km/h", avg));
    }
    if let Some(max) = meta.max_speed_kmh {
        parts.push(format!("Max: {:.1} km/h", max));
    }
    if let Some(cal) = meta.calories_kcal {
        parts.push(format!("Calories: {cal} kcal"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{LogHeader, TrackPoint};
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn make_header(start: DateTime<Utc>) -> LogHeader {
        LogHeader {
            start_date: start,
            start_addr: 0,
            stop_addr: 0,
            distance_m: 1000,
            training_time_ms: 60_000,
            max_speed_kmh: 30.0,
            avg_speed_kmh: 20.0,
            max_altitude_m: 500.0,
            calories_kcal: 100,
        }
    }

    fn make_point(
        lat: f64,
        lon: f64,
        alt: f64,
        speed: f64,
        temp: i8,
        ms: u64,
        pause: bool,
    ) -> TrackPoint {
        TrackPoint {
            latitude: lat,
            longitude: lon,
            altitude_m: alt,
            speed_ms: speed,
            temperature_c: temp,
            training_time_ms: ms,
            is_pause: pause,
        }
    }

    fn write_to_string(header: &LogHeader, points: &[TrackPoint]) -> String {
        let tmp = NamedTempFile::new().unwrap();
        let meta = GpxMeta::from(header);
        write_gpx(tmp.path(), &meta, points).unwrap();
        std::fs::read_to_string(tmp.path()).unwrap()
    }

    // ── track_filename ────────────────────────────────────────────────────────

    #[test]
    fn track_filename_index_1_based() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 7, 49).unwrap();
        let hdr = make_header(start);
        let meta = GpxMeta::from(&hdr);
        assert_eq!(track_filename(&meta, 0), "track_001_20240412_100749.gpx");
        assert_eq!(track_filename(&meta, 9), "track_010_20240412_100749.gpx");
    }

    // ── write_gpx — structure ─────────────────────────────────────────────────

    #[test]
    fn write_gpx_valid_xml_prologue() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let gpx = write_to_string(&make_header(start), &[]);
        assert!(gpx.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(gpx.contains(r#"<gpx version="1.1""#));
        assert!(gpx.contains(r#"xmlns="http://www.topografix.com/GPX/1/1""#));
    }

    #[test]
    fn write_gpx_track_name_is_start_date() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 7, 49).unwrap();
        let gpx = write_to_string(&make_header(start), &[]);
        assert!(gpx.contains("<name>2024-04-12 10:07:49 UTC</name>"));
    }

    #[test]
    fn write_gpx_empty_points_produces_empty_segment() {
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let gpx = write_to_string(&make_header(start), &[]);
        assert!(gpx.contains("<trkseg>"));
        assert!(gpx.contains("</trkseg>"));
        assert!(!gpx.contains("<trkpt"));
    }

    // ── write_gpx — track points ──────────────────────────────────────────────

    #[test]
    fn write_gpx_point_coordinates() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let pt = make_point(47.3667, 8.5500, 442.0, 5.0, 20, 0, false);
        let gpx = write_to_string(&make_header(start), &[pt]);
        assert!(gpx.contains(r#"lat="47.3667000""#));
        assert!(gpx.contains(r#"lon="8.5500000""#));
    }

    #[test]
    fn write_gpx_point_elevation() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let pt = make_point(47.0, 8.0, 500.3, 0.0, 0, 0, false);
        let gpx = write_to_string(&make_header(start), &[pt]);
        assert!(gpx.contains("<ele>500.3</ele>"));
    }

    #[test]
    fn write_gpx_point_timestamp_offset_from_start() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        // 5 000 ms after start → 10:00:05Z
        let pt = make_point(47.0, 8.0, 400.0, 0.0, 0, 5_000, false);
        let gpx = write_to_string(&make_header(start), &[pt]);
        assert!(gpx.contains("<time>2024-04-12T10:00:05Z</time>"));
    }

    #[test]
    fn write_gpx_point_extensions() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let pt = make_point(47.0, 8.0, 400.0, 10.5, 22, 0, false);
        let gpx = write_to_string(&make_header(start), &[pt]);
        assert!(gpx.contains("<speed>10.500</speed>"));
        assert!(gpx.contains("<temperature>22</temperature>"));
    }

    #[test]
    fn write_gpx_pause_points_are_skipped() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let pause = make_point(47.0, 8.0, 400.0, 0.0, 0, 0, true);
        let normal = make_point(47.1, 8.1, 410.0, 5.0, 20, 5_000, false);
        let gpx = write_to_string(&make_header(start), &[pause, normal]);
        // Only one trkpt — the pause is not written
        assert_eq!(gpx.matches("<trkpt").count(), 1);
        assert!(gpx.contains(r#"lat="47.1000000""#));
    }

    #[test]
    fn write_gpx_multiple_points_order_preserved() {
        let start = Utc.with_ymd_and_hms(2024, 4, 12, 10, 0, 0).unwrap();
        let p1 = make_point(47.0, 8.0, 400.0, 5.0, 20, 0, false);
        let p2 = make_point(47.1, 8.1, 410.0, 6.0, 21, 5_000, false);
        let gpx = write_to_string(&make_header(start), &[p1, p2]);
        let first = gpx.find(r#"lat="47.0000000""#).unwrap();
        let second = gpx.find(r#"lat="47.1000000""#).unwrap();
        assert!(first < second);
    }

    #[test]
    fn write_gpx_file_is_created() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let path: PathBuf = tmp_dir.path().join("out.gpx");
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let meta = GpxMeta::from(&make_header(start));
        write_gpx(&path, &meta, &[]).unwrap();
        assert!(path.exists());
    }
}
