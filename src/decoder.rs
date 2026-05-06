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
