use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use std::io::{BufRead, Seek, Write};

/// Decodes the AGPS sync date from 13 bytes read at flash offset 0x1000.
/// Bytes [10]=year-2000, [11]=month (1-based), [12]=day (ported from AgpsLoader.decodeAgpsOfflineDataUploadDate).
pub fn decode_agps_date(data: &[u8]) -> Result<NaiveDate> {
    if data.len() < 13 {
        bail!("AGPS header too short: {} bytes", data.len());
    }
    let year = data[10] as i32 + 2000;
    let month = data[11] as u32;
    let day = data[12] as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| anyhow!("Invalid AGPS date: {year}-{month:02}-{day:02}"))
}

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

pub struct Totals {
    pub total_distance_km: f64,
    pub total_training_time_ms: u64,
    pub total_calories_kcal: u32,
    pub total_climb_m: f64,
    pub reset_date: Option<chrono::NaiveDate>,
}

/// Decodes a 20-byte totals slice from EEPROM offset 304 (ported from `Gps10Decoder.decodeTotals`).
pub fn decode_totals(data: &[u8]) -> Result<Totals> {
    if data.len() < 20 {
        bail!("Totals data too short: {} bytes", data.len());
    }
    verify_checksum(data, 1)?;

    // Raw integer = metres; frac bytes = sub-km metres (0–999); convert to km
    let dist_m = ((data[3] & 0x0F) as u64) << 24
        | (data[2] as u64) << 16
        | (data[1] as u64) << 8
        | data[0] as u64;
    let dist_frac_m = ((data[5] & 0x03) as u32) << 8 | data[4] as u32;
    let total_distance_km = dist_m as f64 / 1000.0 + dist_frac_m as f64 / 1000.0 / 1000.0;

    // encodeTotals stores trainingTime_ms/100 but display unit is deciseconds → *1000
    let total_training_time_ms = (((data[9] & 0x03) as u64) << 24
        | (data[8] as u64) << 16
        | (data[7] as u64) << 8
        | data[6] as u64)
        * 1000;

    let total_calories_kcal =
        ((data[12] & 0x01) as u32) << 16 | (data[11] as u32) << 8 | data[10] as u32;

    // encodeTotals stores climbMeter_mm/100; raw_bits/10000 = metres
    let climb_raw = ((data[15] & 0x0F) as u32) << 16 | (data[14] as u32) << 8 | data[13] as u32;
    let total_climb_m = climb_raw as f64 / 10000.0;

    let year = (data[16] & 0x3F) as i32 + 2000;
    let month = data[17];
    let day = data[18] & 0x1F;
    let reset_date = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32);

    Ok(Totals {
        total_distance_km,
        total_training_time_ms,
        total_calories_kcal,
        total_climb_m,
        reset_date,
    })
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
        // AS3: (raw - 10000) * 100 cm; raw unit = dm → / 10 = metres
        actual_altitude_m: (((data[4] as i32) << 8 | data[3] as i32) - 10000) / 10,
        sea_level_mb: ((((data[6] as u16) << 8 | data[5] as u16) & 0x07FF) as f64) / 10.0 + 900.0,
        home_altitude1_m: (((data[8] as i32) << 8 | data[7] as i32) - 10000) / 10,
        home_altitude2_m: (((data[10] as i32) << 8 | data[9] as i32) - 10000) / 10,
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
    // The device samples one GPS point every 5 seconds during normal recording.
    const SAMPLE_MS: u64 = 5_000;

    while pos < data.len() {
        // Least-significant bit of the first byte indicates entry type:
        //   0 = normal GPS track point (25 bytes)
        //   1 = pause/stop marker    (32 bytes — 7 extra bytes carry pause metadata)
        // (See Gps10Decoder.as, decodeLogData)
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
            // Normal GPS point — advance elapsed time by one sample interval.
            let pt = decode_log_entry(entry, elapsed_ms, false);
            elapsed_ms += SAMPLE_MS;
            points.push(pt);
        } else {
            // Pause marker — byte 18 holds the pause duration in 100 ms units.
            // Advance elapsed time by the actual pause length so subsequent
            // track points carry correct timestamps.
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

/// Decodes a GPS coordinate stored in the device's proprietary DdMmmmmm format.
///
/// The coordinate is split across four bytes:
/// - `degree`: integer degrees (0–180)
/// - `m0`, `m1`, `m2`: a 20-bit little-endian minutes value, where only the
///   lower nibble of `m2` is used (bits 19:16)
///
/// Minutes are stored multiplied by 10 000, so divide by 10 000.0 to get
/// decimal minutes, then convert: decimal_degrees = degrees + minutes / 60.
///
/// Example: 47°04'30" → degrees=47, minutes=4.5, stored=45000
///   m0=0x28 m1=0xAF m2=0x00 → (0<<16)|(0xAF<<8)|0x28 = 45 000 → 45000/10000=4.5 → 47+4.5/60≈47.075
///
/// `positive` is true for North (latitude) or East (longitude).
/// (See Gps10Decoder.as, decodeCoordinates)
fn decode_coord(degree: u8, m0: u8, m1: u8, m2: u8, positive: bool) -> f64 {
    // Reconstruct the 20-bit minutes value from the three bytes.
    let minutes = (((m2 as u32 & 0x0F) << 16) | ((m1 as u32) << 8) | m0 as u32) as f64 / 10000.0;
    let decimal = degree as f64 + minutes / 60.0;
    if positive { decimal } else { -decimal }
}

/// Raw 16×59 pixel bitmap + metadata decoded from the 172-byte sleep screen EEPROM block.
/// Bitmap encoding: row-major, LSB-first; 2 bytes per row × 59 rows = 118 bytes.
/// (See Gps10Decoder.encodeSleepScreen / SleepScreenSign.getBytes in the ActionScript source)
pub struct SleepScreen {
    /// `false` means no sleep screen is configured on the device.
    pub active: bool,
    /// X pixel position of the clock on the watch face.
    pub clock_x: u8,
    /// Y pixel position of the clock on the watch face.
    pub clock_y: u8,
    /// `true` = user name shown at the bottom of the screen; `false` = at the top.
    pub name_bottom: bool,
    /// Raw 118-byte bit-packed bitmap (16 columns × 59 rows, 2 bytes/row, LSB-first).
    pub bitmap: Box<[u8; 118]>,
}

/// Decodes a 172-byte sleep screen block from EEPROM offset 96.
/// CRC covers bytes 0–169 (seed=1); stored at byte 171. Byte 170 is myNamePos.
/// (See Gps10Decoder.encodeSleepScreen in the ActionScript source)
pub fn decode_sleep_screen(data: &[u8]) -> Result<SleepScreen> {
    if data.len() < 172 {
        bail!("Sleep screen data too short: {} bytes", data.len());
    }

    let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let active = id != 0;

    if active {
        // CRC covers bytes 0..169 (170 bytes, slice(0,170) in AS3), stored at byte 171
        let computed: u8 = data[..170].iter().fold(1u8, |acc, &b| acc.wrapping_add(b));
        if computed != data[171] {
            bail!(
                "Sleep screen checksum mismatch: computed {computed:#04x}, stored {:#04x}",
                data[171]
            );
        }
    }

    let bitmap: Box<[u8; 118]> = Box::new(
        data[4..122]
            .try_into()
            .map_err(|_| anyhow::anyhow!("bitmap slice length mismatch"))?,
    );

    Ok(SleepScreen {
        active,
        clock_x: data[168],
        clock_y: data[169],
        name_bottom: data[170] == 1,
        bitmap,
    })
}

/// Writes a `SleepScreen` as a 16×59 1-bit grayscale PNG.
/// Three `tEXt` chunks carry the metadata needed to round-trip back to the device:
///   `clock_x`, `clock_y` (pixel positions), `name_pos` ("top" or "bottom").
///
/// Bit ordering: device stores pixels LSB-first (bit 0 = leftmost); PNG 1-bit stores
/// them MSB-first (bit 7 = leftmost). `reverse_bits()` converts between the two.
pub fn sleep_screen_to_png<W: Write>(screen: &SleepScreen, writer: W) -> Result<()> {
    let mut encoder = png::Encoder::new(writer, 16, 59);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::One);
    encoder.add_text_chunk("clock_x".to_string(), screen.clock_x.to_string())?;
    encoder.add_text_chunk("clock_y".to_string(), screen.clock_y.to_string())?;
    encoder.add_text_chunk(
        "name_pos".to_string(),
        if screen.name_bottom { "bottom" } else { "top" }.to_string(),
    )?;
    let mut png_writer = encoder.write_header()?;
    // Reverse bits in each byte so leftmost pixel maps to bit 7 (PNG MSB-first convention)
    let png_rows: Vec<u8> = screen.bitmap.iter().map(|b| b.reverse_bits()).collect();
    png_writer.write_image_data(&png_rows)?;
    Ok(())
}

/// Encodes a `SleepScreen` into the 172-byte payload written to EEPROM offset 96.
/// Inverse of `decode_sleep_screen`.
/// CRC covers bytes 0–169 (seed=1), stored at byte 171.
/// (See Gps10Decoder.encodeSleepScreen in the ActionScript source)
pub fn encode_sleep_screen(screen: &SleepScreen) -> [u8; 172] {
    let mut buf = [0u8; 172];
    if !screen.active {
        // "no screen" sentinel: all bytes zero except byte 171 = 1
        buf[171] = 1;
        return buf;
    }
    // id = 1 (LE u32)
    buf[0] = 1;
    // bitmap at bytes 4..122
    buf[4..122].copy_from_slice(screen.bitmap.as_ref());
    // clock position
    buf[168] = screen.clock_x;
    buf[169] = screen.clock_y;
    // name position
    buf[170] = u8::from(screen.name_bottom);
    // CRC over bytes 0..169, seed=1, stored at byte 171
    buf[171] = buf[..170].iter().fold(1u8, |acc, &b| acc.wrapping_add(b));
    buf
}

/// Reads a PNG file written by `sleep_screen_to_png` and reconstructs a `SleepScreen`.
/// The PNG must be 16×59 1-bit grayscale; `tEXt` chunks supply clock position and name pos.
pub fn sleep_screen_from_png<R: BufRead + Seek>(reader: R) -> Result<SleepScreen> {
    let decoder = png::Decoder::new(reader);
    let mut png_reader = decoder.read_info()?;

    // Extract tEXt metadata before consuming the pixel data
    let mut clock_x: u8 = 0;
    let mut clock_y: u8 = 0;
    let mut name_bottom = false;
    for chunk in &png_reader.info().uncompressed_latin1_text {
        match chunk.keyword.as_str() {
            "clock_x" => clock_x = chunk.text.trim().parse().unwrap_or(0),
            "clock_y" => clock_y = chunk.text.trim().parse().unwrap_or(0),
            "name_pos" => name_bottom = chunk.text.trim() == "bottom",
            _ => {}
        }
    }

    let info = png_reader.info();
    if info.width != 16 || info.height != 59 {
        bail!(
            "PNG dimensions must be 16×59, got {}×{}",
            info.width,
            info.height
        );
    }

    let buf_size = png_reader
        .output_buffer_size()
        .ok_or_else(|| anyhow!("could not determine PNG output buffer size"))?;
    let mut buf = vec![0u8; buf_size];
    let frame_info = png_reader.next_frame(&mut buf)?;

    if frame_info.buffer_size() < 118 {
        bail!(
            "PNG pixel data too short: {} bytes",
            frame_info.buffer_size()
        );
    }

    // Reverse bits: PNG MSB-first → device LSB-first
    let bitmap: [u8; 118] = buf[..118]
        .iter()
        .map(|b| b.reverse_bits())
        .collect::<Vec<u8>>()
        .try_into()
        .map_err(|_| anyhow!("bitmap conversion failed"))?;

    Ok(SleepScreen {
        active: true,
        clock_x,
        clock_y,
        name_bottom,
        bitmap: Box::new(bitmap),
    })
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

// ---------------------------------------------------------------------------
// Helpers shared across tests
// ---------------------------------------------------------------------------

/// Appends a seed-1 checksum byte to a slice and returns the full Vec.
#[cfg(test)]
fn with_checksum(data: &[u8]) -> Vec<u8> {
    let crc = data.iter().fold(1u8, |acc, &b| acc.wrapping_add(b));
    let mut v = data.to_vec();
    v.push(crc);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // -----------------------------------------------------------------------
    // decode_agps_date
    // -----------------------------------------------------------------------

    #[test]
    fn agps_date_valid() {
        let mut data = vec![0u8; 13];
        data[10] = 24; // year offset from 2000
        data[11] = 5; // month
        data[12] = 6; // day
        let date = decode_agps_date(&data).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 5, 6).unwrap());
    }

    #[test]
    fn agps_date_too_short() {
        assert!(decode_agps_date(&[0u8; 12]).is_err());
    }

    #[test]
    fn agps_date_invalid_calendar() {
        let mut data = vec![0u8; 13];
        data[10] = 24;
        data[11] = 13; // month 13 — invalid
        data[12] = 1;
        assert!(decode_agps_date(&data).is_err());
    }

    // -----------------------------------------------------------------------
    // decode_totals
    // -----------------------------------------------------------------------

    fn make_totals_bytes(
        dist_m: u32,
        dist_frac: u16,
        time_units: u32,
        cal: u32,
        climb_raw: u32,
        year_off: u8,
        month: u8,
        day: u8,
    ) -> Vec<u8> {
        let mut d = [0u8; 19];
        d[0] = (dist_m & 0xFF) as u8;
        d[1] = (dist_m >> 8 & 0xFF) as u8;
        d[2] = (dist_m >> 16 & 0xFF) as u8;
        d[3] = (dist_m >> 24 & 0x0F) as u8;
        d[4] = (dist_frac & 0xFF) as u8;
        d[5] = (dist_frac >> 8 & 0x03) as u8;
        d[6] = (time_units & 0xFF) as u8;
        d[7] = (time_units >> 8 & 0xFF) as u8;
        d[8] = (time_units >> 16 & 0xFF) as u8;
        d[9] = (time_units >> 24 & 0x03) as u8;
        d[10] = (cal & 0xFF) as u8;
        d[11] = (cal >> 8 & 0xFF) as u8;
        d[12] = (cal >> 16 & 0x01) as u8;
        d[13] = (climb_raw & 0xFF) as u8;
        d[14] = (climb_raw >> 8 & 0xFF) as u8;
        d[15] = (climb_raw >> 16 & 0x0F) as u8;
        d[16] = year_off & 0x3F;
        d[17] = month;
        d[18] = day & 0x1F;
        with_checksum(&d)
    }

    #[test]
    fn totals_zero() {
        let data = make_totals_bytes(0, 0, 0, 0, 0, 24, 5, 6);
        let t = decode_totals(&data).unwrap();
        assert_eq!(t.total_distance_km, 0.0);
        assert_eq!(t.total_training_time_ms, 0);
        assert_eq!(t.total_calories_kcal, 0);
        assert_eq!(t.total_climb_m, 0.0);
        assert_eq!(t.reset_date, NaiveDate::from_ymd_opt(2024, 5, 6));
    }

    #[test]
    fn totals_distance_1000km() {
        // 1 000 000 m integer part, 500 fractional metres
        let data = make_totals_bytes(1_000_000, 500, 0, 0, 0, 0, 1, 1);
        let t = decode_totals(&data).unwrap();
        assert!((t.total_distance_km - 1000.0005).abs() < 1e-9);
    }

    #[test]
    fn totals_training_time() {
        // time_units = 36000 → 36000 * 1000 ms = 36_000_000 ms = 10 h
        let data = make_totals_bytes(0, 0, 36_000, 0, 0, 0, 1, 1);
        let t = decode_totals(&data).unwrap();
        assert_eq!(t.total_training_time_ms, 36_000_000);
    }

    #[test]
    fn totals_calories() {
        let data = make_totals_bytes(0, 0, 0, 500, 0, 0, 1, 1);
        let t = decode_totals(&data).unwrap();
        assert_eq!(t.total_calories_kcal, 500);
    }

    #[test]
    fn totals_climb() {
        // climb_raw = 10000 → 10000 / 10000 = 1.0 m
        let data = make_totals_bytes(0, 0, 0, 0, 10_000, 0, 1, 1);
        let t = decode_totals(&data).unwrap();
        assert!((t.total_climb_m - 1.0).abs() < 1e-9);
    }

    #[test]
    fn totals_bad_checksum() {
        let mut data = make_totals_bytes(0, 0, 0, 0, 0, 0, 1, 1);
        *data.last_mut().unwrap() ^= 0xFF;
        assert!(decode_totals(&data).is_err());
    }

    #[test]
    fn totals_too_short() {
        assert!(decode_totals(&[0u8; 5]).is_err());
    }

    // -----------------------------------------------------------------------
    // decode_settings
    // -----------------------------------------------------------------------

    fn make_settings_bytes() -> Vec<u8> {
        let mut d = [0u8; 31];
        // byte 0: timeZone=16 (GMT+01:00), summerTime=1, clockMode=24h
        d[0] = 16 | (1 << 6); // tz=16, summer=1, clock=0→24h
        // byte 1: language=1(de), autoPause=0, speed=0(km/h), altRef=0(actual), contrast=2→stored as 1
        d[1] = 1 | (1 << 6); // language=de, contrast=2 (bits 7:6 = 01)
        // byte 2: dateFormat=eu, temp=celsius, alt=meter, nfc=1, systemTone=0; also bits 7:5 = 101
        d[2] = (1 << 3) | 0xA0; // nfc=1, plus 0xA0 from encodeSettings constant
        // actualAltitude: raw = 442*10 + 10000 = 14420 = 0x3854
        d[3] = 0x54;
        d[4] = 0x38;
        // seaLevel: raw = (1013.25 - 900) * 10 = 1132 ≈ 1132 = 0x046C (masked to 11 bits)
        let sl: u16 = 1132;
        d[5] = (sl & 0xFF) as u8;
        d[6] = (sl >> 8) as u8;
        // homeAlt1 = 500m: raw = 500*10+10000=15000=0x3A98
        d[7] = 0x98;
        d[8] = 0x3A;
        // homeAlt2 = 0m: raw = 10000=0x2710
        d[9] = 0x10;
        d[10] = 0x27;
        // name: "Test" = 0x54 0x65 0x73 0x74
        d[11] = b'T';
        d[12] = b'e';
        d[13] = b's';
        d[14] = b't';
        // auto-lap: 5000m = 0x1388
        d[20] = 0x88;
        d[21] = 0x13;
        with_checksum(&d)
    }

    #[test]
    fn settings_decode_fields() {
        let data = make_settings_bytes();
        let s = decode_settings(&data).unwrap();
        assert_eq!(s.time_zone, "GMT +01:00");
        assert!(s.summer_time);
        assert_eq!(s.clock_mode, 24);
        assert_eq!(s.language, "de");
        assert!(!s.auto_pause);
        assert_eq!(s.speed_unit, "km/h");
        assert_eq!(s.altitude_reference, "actual altitude");
        assert_eq!(s.contrast, 2);
        assert_eq!(s.date_format, "DD-MM-YY");
        assert_eq!(s.temperature_unit, "°C");
        assert_eq!(s.altitude_unit, "m");
        assert!(s.nfc_active);
        assert!(!s.system_tone);
        assert_eq!(s.actual_altitude_m, 442);
        assert_eq!(s.home_altitude1_m, 500);
        assert_eq!(s.home_altitude2_m, 0);
        assert_eq!(s.name, "Test");
        assert_eq!(s.auto_lap_distance_m, 5000);
    }

    #[test]
    fn settings_bad_checksum() {
        let mut data = make_settings_bytes();
        *data.last_mut().unwrap() ^= 0xFF;
        assert!(decode_settings(&data).is_err());
    }

    #[test]
    fn settings_too_short() {
        assert!(decode_settings(&[0u8; 10]).is_err());
    }

    // -----------------------------------------------------------------------
    // decode_log_header
    // -----------------------------------------------------------------------

    fn make_log_header() -> Vec<u8> {
        let mut h = [0u8; 64];
        // start date: 2024-04-12 10:07:49
        h[7] = 0xE8; // year lo: 2024 = 0x07E8
        h[8] = 0x07; // year hi
        h[9] = 4; // month
        h[10] = 12; // day
        h[11] = 10; // hour
        h[12] = 7; // minute
        h[13] = 49; // second
        // training time: 3600 s = 36000 * 100ms units → stored as 36000 = 0x8CA0
        h[16] = 0xA0;
        h[17] = 0x8C;
        // max speed: 3600 cm/s = 36.00 km/h → stored as 3600
        h[20] = 0x10;
        h[21] = 0x0E;
        // avg speed: 2520 = 25.20 km/h (0x09D8)
        h[24] = 0xD8;
        h[25] = 0x09;
        // max altitude: raw = 1500 + 1000 = 2500 = 0x09C4
        h[26] = 0xC4;
        h[27] = 0x09;
        // distance: 42195 m (marathon)
        h[30] = (42195 & 0xFF) as u8;
        h[31] = (42195 >> 8 & 0xFF) as u8;
        h[32] = (42195 >> 16 & 0xFF) as u8;
        // start_addr = 0x00001000
        h[33] = 0x00;
        h[34] = 0x10;
        h[35] = 0x00;
        h[36] = 0x00;
        // stop_addr = 0x00002000
        h[37] = 0x00;
        h[38] = 0x20;
        h[39] = 0x00;
        h[40] = 0x00;
        // calories = 1234
        h[41] = (1234 & 0xFF) as u8;
        h[42] = (1234 >> 8 & 0xFF) as u8;
        with_checksum(&h)
    }

    #[test]
    fn log_header_decode() {
        let data = make_log_header();
        let hdr = decode_log_header(&data).unwrap();
        assert_eq!(
            hdr.start_date,
            Utc.from_utc_datetime(
                &NaiveDate::from_ymd_opt(2024, 4, 12)
                    .unwrap()
                    .and_hms_opt(10, 7, 49)
                    .unwrap()
            )
        );
        assert_eq!(hdr.distance_m, 42195);
        assert_eq!(hdr.start_addr, 0x1000);
        assert_eq!(hdr.stop_addr, 0x2000);
        assert_eq!(hdr.calories_kcal, 1234);
        assert!((hdr.max_speed_kmh - 36.0).abs() < 0.01);
        assert!((hdr.avg_speed_kmh - 25.2).abs() < 0.01);
        assert!((hdr.max_altitude_m - 1500.0).abs() < 0.01);
    }

    #[test]
    fn log_header_too_short() {
        assert!(decode_log_header(&[0u8; 10]).is_err());
    }

    #[test]
    fn log_header_bad_checksum() {
        let mut data = make_log_header();
        *data.last_mut().unwrap() ^= 0xFF;
        assert!(decode_log_header(&data).is_err());
    }

    // -----------------------------------------------------------------------
    // decode_log_data — normal GPS point
    // -----------------------------------------------------------------------

    fn make_normal_entry(
        lat_deg: u8,
        lat_min_raw: u32,
        north: bool,
        lon_deg: u8,
        lon_min_raw: u32,
        east: bool,
        alt_raw: u16,
        speed_cmps: u16,
        temp_raw: u8,
    ) -> Vec<u8> {
        let mut e = [0u8; 24];
        // byte 0 bit 0 = 0 → normal entry; bit 5 = 0 → positive incline sign
        e[0] = 0;
        e[5] = temp_raw;
        e[6] = (speed_cmps & 0xFF) as u8;
        e[7] = (speed_cmps >> 8) as u8;
        e[8] = (alt_raw & 0xFF) as u8;
        e[9] = (alt_raw >> 8) as u8;
        e[10] = lat_deg;
        e[11] = (lat_min_raw & 0xFF) as u8;
        e[12] = (lat_min_raw >> 8 & 0xFF) as u8;
        // byte 13: high nibble = lat minutes bits 19:16; bit4 = north; bit5 = east
        e[13] = ((lat_min_raw >> 16) & 0x0F) as u8
            | if north { 1 << 4 } else { 0 }
            | if east { 1 << 5 } else { 0 };
        e[14] = lon_deg;
        e[15] = (lon_min_raw & 0xFF) as u8;
        e[16] = (lon_min_raw >> 8 & 0xFF) as u8;
        e[17] = (lon_min_raw >> 16 & 0x0F) as u8;
        with_checksum(&e)
    }

    #[test]
    fn log_data_single_normal_point() {
        // Zurich approx: 47°22'N, 8°33'E
        // lat minutes: 22.0 * 10000 = 220000 = 0x035B60
        // lon minutes:  33.0 * 10000 = 330000 = 0x050AD0
        let lat_min = 220_000u32;
        let lon_min = 330_000u32;
        // altitude: raw = (500 + 1000) = 1500 = 0x05DC; actual = (1500-1000)*1 = 500 m
        let alt_raw: u16 = 1500;
        // speed: 3600 cm/s → 36.00 km/h / 3.6 = 10.0 m/s
        let speed: u16 = 3600;
        // temp_raw = 35 → 35 - 10 = 25 °C
        let temp: u8 = 35;

        let entry = make_normal_entry(47, lat_min, true, 8, lon_min, true, alt_raw, speed, temp);
        let points = decode_log_data(&entry);
        assert_eq!(points.len(), 1);
        let pt = &points[0];
        assert!((pt.latitude - (47.0 + 22.0 / 60.0)).abs() < 1e-4);
        assert!((pt.longitude - (8.0 + 33.0 / 60.0)).abs() < 1e-4);
        assert!((pt.altitude_m - 500.0).abs() < 0.1);
        assert!((pt.speed_ms - 10.0).abs() < 0.01);
        assert_eq!(pt.temperature_c, 25);
        assert_eq!(pt.training_time_ms, 0); // first point starts at t=0
        assert!(!pt.is_pause);
    }

    #[test]
    fn log_data_elapsed_time_advances() {
        // Two consecutive normal entries — second should have training_time_ms = 5000
        let entry = make_normal_entry(47, 220_000, true, 8, 330_000, true, 1500, 0, 20);
        let two = [entry.clone(), entry].concat();
        let points = decode_log_data(&two);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].training_time_ms, 0);
        assert_eq!(points[1].training_time_ms, 5_000);
    }

    #[test]
    fn log_data_south_west_coords() {
        // South (-lat), West (-lon)
        let entry = make_normal_entry(33, 550_000, false, 70, 450_000, false, 1000, 0, 20);
        let points = decode_log_data(&entry);
        assert_eq!(points.len(), 1);
        assert!(points[0].latitude < 0.0);
        assert!(points[0].longitude < 0.0);
    }

    fn make_pause_entry(pause_units: u8) -> Vec<u8> {
        // byte 0 bit 0 = 1 → pause entry; 32 bytes total (31 payload + 1 CRC)
        let mut e = [0u8; 31];
        e[0] = 1; // entry_type = pause
        e[18] = pause_units;
        // fill coord bytes with a valid zero-ish coord (north, east)
        e[13] = (1 << 4) | (1 << 5); // north + east
        with_checksum(&e)
    }

    #[test]
    fn log_data_pause_marker_advances_time() {
        // pause_units = 100 → 100 * 100 ms = 10 000 ms elapsed after the pause point
        let pause = make_pause_entry(100);
        let normal = make_normal_entry(47, 220_000, true, 8, 330_000, true, 1500, 0, 20);
        let data = [pause, normal].concat();
        let points = decode_log_data(&data);
        assert_eq!(points.len(), 2);
        let pause_pt = &points[0];
        let normal_pt = &points[1];
        assert!(pause_pt.is_pause);
        assert_eq!(pause_pt.training_time_ms, 0);
        // normal point after 100-unit pause: 100 * 100 ms = 10 000 ms
        assert_eq!(normal_pt.training_time_ms, 10_000);
        assert!(!normal_pt.is_pause);
    }

    #[test]
    fn log_data_bad_checksum_stops_parsing() {
        let mut entry = make_normal_entry(47, 220_000, true, 8, 330_000, true, 1500, 0, 20);
        *entry.last_mut().unwrap() ^= 0xFF;
        let points = decode_log_data(&entry);
        assert!(points.is_empty());
    }

    #[test]
    fn log_data_empty() {
        assert!(decode_log_data(&[]).is_empty());
    }

    // -----------------------------------------------------------------------
    // decode_sleep_screen
    // -----------------------------------------------------------------------

    fn make_sleep_screen_payload(
        active: bool,
        clock_x: u8,
        clock_y: u8,
        name_bottom: bool,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 172];
        if !active {
            buf[171] = 1;
            return buf;
        }
        buf[0] = 1; // id = 1
        buf[168] = clock_x;
        buf[169] = clock_y;
        buf[170] = u8::from(name_bottom);
        let crc = buf[..170].iter().fold(1u8, |acc, &b| acc.wrapping_add(b));
        buf[171] = crc;
        buf
    }

    #[test]
    fn sleep_screen_inactive() {
        let data = make_sleep_screen_payload(false, 0, 0, false);
        let s = decode_sleep_screen(&data).unwrap();
        assert!(!s.active);
    }

    #[test]
    fn sleep_screen_active_metadata() {
        let data = make_sleep_screen_payload(true, 27, 4, false);
        let s = decode_sleep_screen(&data).unwrap();
        assert!(s.active);
        assert_eq!(s.clock_x, 27);
        assert_eq!(s.clock_y, 4);
        assert!(!s.name_bottom);
    }

    #[test]
    fn sleep_screen_name_bottom() {
        let data = make_sleep_screen_payload(true, 10, 10, true);
        let s = decode_sleep_screen(&data).unwrap();
        assert!(s.name_bottom);
    }

    #[test]
    fn sleep_screen_bad_checksum() {
        let mut data = make_sleep_screen_payload(true, 27, 4, false);
        data[171] ^= 0xFF;
        assert!(decode_sleep_screen(&data).is_err());
    }

    #[test]
    fn sleep_screen_too_short() {
        assert!(decode_sleep_screen(&[0u8; 50]).is_err());
    }

    // -----------------------------------------------------------------------
    // encode_sleep_screen — inverse of decode_sleep_screen
    // -----------------------------------------------------------------------

    #[test]
    fn encode_inactive_screen() {
        let screen = SleepScreen {
            active: false,
            clock_x: 0,
            clock_y: 0,
            name_bottom: false,
            bitmap: Box::new([0u8; 118]),
        };
        let buf = encode_sleep_screen(&screen);
        // All zero except byte 171 = 1 (sentinel)
        assert!(buf[..171].iter().all(|&b| b == 0));
        assert_eq!(buf[171], 1);
    }

    #[test]
    fn encode_active_screen_id_and_crc() {
        let mut bitmap = [0u8; 118];
        bitmap[0] = 0b10110001; // some pixel data
        let screen = SleepScreen {
            active: true,
            clock_x: 27,
            clock_y: 4,
            name_bottom: false,
            bitmap: Box::new(bitmap),
        };
        let buf = encode_sleep_screen(&screen);
        // id bytes
        assert_eq!(buf[0], 1);
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        // bitmap
        assert_eq!(buf[4], 0b10110001);
        // metadata
        assert_eq!(buf[168], 27);
        assert_eq!(buf[169], 4);
        assert_eq!(buf[170], 0);
        // CRC
        let expected_crc = buf[..170].iter().fold(1u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(buf[171], expected_crc);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let mut bitmap = [0u8; 118];
        for (i, b) in bitmap.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7);
        }
        let original = SleepScreen {
            active: true,
            clock_x: 12,
            clock_y: 7,
            name_bottom: true,
            bitmap: Box::new(bitmap),
        };
        let encoded = encode_sleep_screen(&original);
        let decoded = decode_sleep_screen(&encoded).unwrap();
        assert!(decoded.active);
        assert_eq!(decoded.clock_x, 12);
        assert_eq!(decoded.clock_y, 7);
        assert!(decoded.name_bottom);
        assert_eq!(*decoded.bitmap, bitmap);
    }

    // -----------------------------------------------------------------------
    // PNG round-trip: sleep_screen_to_png → sleep_screen_from_png
    // -----------------------------------------------------------------------

    #[test]
    fn png_roundtrip() {
        let mut bitmap = [0u8; 118];
        bitmap[0] = 0b10101010;
        bitmap[60] = 0b11001100;
        let original = SleepScreen {
            active: true,
            clock_x: 27,
            clock_y: 4,
            name_bottom: false,
            bitmap: Box::new(bitmap),
        };

        let mut buf = Vec::new();
        sleep_screen_to_png(&original, &mut buf).unwrap();

        let cursor = Cursor::new(buf);
        let decoded = sleep_screen_from_png(cursor).unwrap();

        assert!(decoded.active);
        assert_eq!(decoded.clock_x, 27);
        assert_eq!(decoded.clock_y, 4);
        assert!(!decoded.name_bottom);
        assert_eq!(*decoded.bitmap, bitmap);
    }

    #[test]
    fn png_roundtrip_name_bottom() {
        let original = SleepScreen {
            active: true,
            clock_x: 5,
            clock_y: 10,
            name_bottom: true,
            bitmap: Box::new([0u8; 118]),
        };
        let mut buf = Vec::new();
        sleep_screen_to_png(&original, &mut buf).unwrap();
        let decoded = sleep_screen_from_png(Cursor::new(buf)).unwrap();
        assert!(decoded.name_bottom);
    }
}
