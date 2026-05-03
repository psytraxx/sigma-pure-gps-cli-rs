use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub struct Settings {
    pub time_zone: &'static str,
    pub summer_time: bool,
    pub clock_mode: u8,
    pub language: &'static str,
    pub auto_pause: bool,
    pub speed_unit: &'static str,
    pub altitude_reference: &'static str,
    pub contrast: u8,
    pub date_format: &'static str,
    pub temperature_unit: &'static str,
    pub altitude_unit: &'static str,
    pub nfc_active: bool,
    pub system_tone: bool,
    pub actual_altitude_m: i32,
    pub sea_level_mb: f64,
    pub home_altitude1_m: i32,
    pub home_altitude2_m: i32,
    pub name: String,
    pub auto_lap_distance_m: u16,
}

// GPS10 timezone index table — matches DATA_PROVIDER_GPS_10 in CommonTimeZoneDataProvider.as
const TIMEZONE_LABELS: &[&str] = &[
    "GMT -12:00",
    "GMT -11:00",
    "GMT -10:00",
    "GMT -09:30",
    "GMT -09:00",
    "GMT -08:00",
    "GMT -07:00",
    "GMT -06:00",
    "GMT -05:00",
    "GMT -04:30",
    "GMT -04:00",
    "GMT -03:30",
    "GMT -03:00",
    "GMT -02:00",
    "GMT -01:00",
    "GMT +00:00",
    "GMT +01:00",
    "GMT +02:00",
    "GMT +03:00",
    "GMT +03:30",
    "GMT +04:00",
    "GMT +04:30",
    "GMT +05:00",
    "GMT +05:30",
    "GMT +05:45",
    "GMT +06:00",
    "GMT +06:30",
    "GMT +07:00",
    "GMT +08:00",
    "GMT +08:45",
    "GMT +09:00",
    "GMT +09:30",
    "GMT +10:00",
    "GMT +10:30",
    "GMT +11:00",
    "GMT +11:30",
    "GMT +12:00",
    "GMT +12:45",
    "GMT +13:00",
    "GMT +14:00",
];

/// Decodes a 32-byte settings slice from EEPROM offset 272 (ported from `Gps10Decoder.decodeSettings`).
pub fn decode_settings(data: &[u8]) -> Result<Settings> {
    if data.len() < 32 {
        bail!("Settings data too short: {} bytes", data.len());
    }
    verify_checksum(data, 1)?;

    let language = match data[1] & 0x07 {
        0 => "en",
        1 => "de",
        2 => "fr",
        3 => "it",
        4 => "es",
        5 => "nl",
        6 => "pl",
        _ => "?",
    };

    let mut name = String::new();
    for &b in &data[11..20] {
        if b == 0 {
            break;
        }
        if b.is_ascii() {
            name.push(b as char);
        }
    }

    Ok(Settings {
        time_zone: TIMEZONE_LABELS
            .get((data[0] & 0x3F) as usize)
            .copied()
            .unwrap_or("?"),
        summer_time: (data[0] >> 6) & 1 == 1,
        clock_mode: if (data[0] >> 7) & 1 == 0 { 24 } else { 12 },
        language,
        auto_pause: (data[1] >> 3) & 1 == 1,
        speed_unit: if (data[1] >> 4) & 1 == 0 {
            "km/h"
        } else {
            "mph"
        },
        altitude_reference: if (data[1] >> 5) & 1 == 0 {
            "actual altitude"
        } else {
            "sea level"
        },
        contrast: ((data[1] >> 6) & 0x03) + 1,
        date_format: if data[2] & 1 == 0 {
            "DD-MM-YY"
        } else {
            "MM-DD-YY"
        },
        temperature_unit: if (data[2] >> 1) & 1 == 0 {
            "°C"
        } else {
            "°F"
        },
        altitude_unit: if (data[2] >> 2) & 1 == 0 { "m" } else { "ft" },
        nfc_active: (data[2] >> 3) & 1 == 1,
        system_tone: (data[2] >> 4) & 1 == 1,
        // AS3: (raw - 10000) * 100 cm → divide by 100 = raw - 10000 metres
        actual_altitude_m: ((data[4] as i32) << 8 | data[3] as i32) - 10000,
        sea_level_mb: ((((data[6] as u16) << 8 | data[5] as u16) & 0x07FF) as f64) / 10.0 + 900.0,
        home_altitude1_m: ((data[8] as i32) << 8 | data[7] as i32) - 10000,
        home_altitude2_m: ((data[10] as i32) << 8 | data[9] as i32) - 10000,
        name,
        auto_lap_distance_m: (data[21] as u16) << 8 | data[20] as u16,
    })
}

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

    // e[13] carries both direction flags: bit4=North(+)/South(-), bit5=East(+)/West(-)
    // Latitude minutes high nibble: e[13] & 0x0F; longitude minutes high nibble: e[17] & 0x0F
    let lat_north = (e[13] >> 4) & 0x01 == 1;
    let lon_east = (e[13] >> 5) & 0x01 == 1;
    let lat = decode_coord(e[10], e[11], e[12], e[13], lat_north);
    let lon = decode_coord(e[14], e[15], e[16], e[17], lon_east);

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

/// Decodes the DdMmmmmm coordinate format.
/// `m2` supplies the upper 4 bits of the 20-bit minutes field (lower nibble).
/// `positive` is true for North (latitude) or East (longitude).
fn decode_coord(degree: u8, m0: u8, m1: u8, m2: u8, positive: bool) -> f64 {
    let minutes = (((m2 as u32 & 0x0F) << 16) | ((m1 as u32) << 8) | m0 as u32) as f64 / 10000.0;
    let decimal = degree as f64 + minutes / 60.0;
    if positive { decimal } else { -decimal }
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
