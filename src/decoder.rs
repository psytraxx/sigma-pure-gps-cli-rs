use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub struct LogHeader {
    pub start_date: DateTime<Utc>,
    pub start_addr: u32,
    pub stop_addr: u32,
    pub distance_m: u32,
    #[allow(dead_code)]
    pub training_time_ms: u32,
    #[allow(dead_code)]
    pub max_speed_kmh: f64,
    #[allow(dead_code)]
    pub avg_speed_kmh: f64,
    #[allow(dead_code)]
    pub max_altitude_m: f64,
    #[allow(dead_code)]
    pub calories_kcal: u32,
}

pub struct TrackPoint {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub speed_ms: f64,
    pub temperature_c: i8,
    /// Elapsed time in milliseconds from track start
    pub training_time_ms: u64,
    pub is_pause: bool,
}

/// Decodes a 65-byte log header slice.
pub fn decode_log_header(h: &[u8]) -> Result<LogHeader> {
    if h.len() < 65 {
        bail!("Log header too short: {} bytes", h.len());
    }
    verify_checksum(h, 1)?;

    let year = (h[8] as u16) << 8 | h[7] as u16;
    let month = h[9] as u32; // stored 1-based
    let day = h[10] as u32;
    let hour = h[11] as u32;
    let minute = h[12] as u32;
    let second = h[13] as u32;

    let start_date = NaiveDate::from_ymd_opt(year as i32, month, day)
        .and_then(|d| d.and_hms_opt(hour, minute, second))
        .map(|ndt| Utc.from_utc_datetime(&ndt))
        .unwrap_or(DateTime::UNIX_EPOCH);

    let training_time_ms =
        (((h[18] as u32 & 0x07) << 16) | ((h[17] as u32) << 8) | h[16] as u32) * 100;

    let max_speed_kmh = ((h[21] as u16) << 8 | h[20] as u16) as f64 / 100.0;
    let avg_speed_kmh = ((h[25] as u16) << 8 | h[24] as u16) as f64 / 100.0;
    let max_altitude_m = (((h[27] as u16) << 8 | h[26] as u16) as i32 - 1000) as f64;

    let distance_m = ((h[32] as u32) << 16) | ((h[31] as u32) << 8) | h[30] as u32;

    let start_addr = ((h[36] as u32 & 0x03) << 24)
        | ((h[35] as u32) << 16)
        | ((h[34] as u32) << 8)
        | h[33] as u32;
    let stop_addr = ((h[40] as u32 & 0x03) << 24)
        | ((h[39] as u32) << 16)
        | ((h[38] as u32) << 8)
        | h[37] as u32;

    let calories_kcal = ((h[43] as u32 & 0x01) << 16) | ((h[42] as u32) << 8) | h[41] as u32;

    Ok(LogHeader {
        start_date,
        start_addr,
        stop_addr,
        distance_m,
        training_time_ms,
        max_speed_kmh,
        avg_speed_kmh,
        max_altitude_m,
        calories_kcal,
    })
}

/// Decodes raw log data bytes into a list of track points.
/// Sampling rate is 5 seconds per normal entry.
pub fn decode_log_data(data: &[u8]) -> Vec<TrackPoint> {
    let mut points = Vec::new();
    let mut pos = 0;
    let mut elapsed_ms: u64 = 0;
    const SAMPLE_MS: u64 = 5_000;

    while pos < data.len() {
        let entry_type = data[pos] & 0x01;
        let entry_size = if entry_type == 0 { 25 } else { 32 };

        if pos + entry_size > data.len() {
            break;
        }

        let entry = &data[pos..pos + entry_size];
        if verify_checksum(entry, 1).is_err() {
            break;
        }

        if entry_type == 0 {
            // Normal log entry (25 bytes)
            let pt = decode_log_entry(entry, elapsed_ms, false);
            elapsed_ms += SAMPLE_MS;
            points.push(pt);
        } else {
            // Pause entry (32 bytes)
            let training_time_units = entry[18] as u64;
            let pt = decode_log_entry(entry, elapsed_ms, true);
            elapsed_ms += training_time_units * 100;
            points.push(pt);
        }

        pos += entry_size;
    }

    points
}

fn decode_log_entry(e: &[u8], elapsed_ms: u64, is_pause: bool) -> TrackPoint {
    let speed_kmh = ((e[7] as u16) << 8 | e[6] as u16) as f64 / 100.0;
    let speed_ms = speed_kmh / 3.6;
    let altitude_mm = (((e[9] as i32) << 8 | e[8] as i32) - 1000) * 1000;
    let altitude_m = altitude_mm as f64 / 1000.0;
    let temperature_c = e[5] as i8 - 10;

    // Both direction bits live in e[13]: bit4=North/South, bit5=East/West
    let lat = decode_coord(e[10], e[11], e[12], e[13], true);
    let lon = decode_coord(e[14], e[15], e[16], e[13], false);

    TrackPoint {
        latitude: lat,
        longitude: lon,
        altitude_m,
        speed_ms,
        temperature_c,
        training_time_ms: elapsed_ms,
        is_pause,
    }
}

/// Decodes the DdMmmmmm coordinate format used by both log entries and headers.
///
/// Latitude (is_lat=true):  bit 4 of m2 → 1=North (+), 0=South (−)
/// Longitude (is_lat=false): bit 5 of m2 → 1=East (+), 0=West (−)
fn decode_coord(degree: u8, m0: u8, m1: u8, m2: u8, is_lat: bool) -> f64 {
    let minutes = (((m2 as u32 & 0x0F) << 16) | ((m1 as u32) << 8) | m0 as u32) as f64 / 10000.0;
    // bit=1 means positive direction (North / East); bit=0 means negative (South / West)
    let positive_bit = if is_lat {
        (m2 >> 4) & 0x01
    } else {
        (m2 >> 5) & 0x01
    };
    let decimal = degree as f64 + minutes / 60.0;
    if positive_bit == 0 { -decimal } else { decimal }
}

fn verify_checksum(data: &[u8], seed: u8) -> Result<()> {
    if data.is_empty() {
        bail!("Empty data");
    }
    let expected = data[data.len() - 1];
    let computed = data[..data.len() - 1]
        .iter()
        .fold(seed, |acc, &b| acc.wrapping_add(b));
    if computed != expected {
        bail!("Checksum mismatch: got {computed:#04x}, expected {expected:#04x}");
    }
    Ok(())
}
