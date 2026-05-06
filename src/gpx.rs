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
