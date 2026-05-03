//! Command bytes reverse-engineered from Gps10Handler.as (decimal literals converted to hex).

/// Tells the device a transfer is starting (no response expected).
pub const CMD_TRANSFER_STARTED: &[u8] = &[0x57, 0x08, 0x00, 0x00, 0x00, 0x00, 0x01, 0x60];

/// Prepares the device to receive AGPS flash data; device replies with 8 bytes.
pub const CMD_SEND_AGPS: &[u8] = &[
    0x52, 0x0C, 0x00, 0x00, 0x00, 0xF8, 0x7F, 0x00, 0x00, 0x10, 0x00, 0xE5,
];

/// Opens the data stream; device replies with 9 bytes.
pub const CMD_SEND_START: &[u8] = &[0x53, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x5B];

/// Closes the data stream; device replies with 9 bytes.
pub const CMD_SEND_END: &[u8] = &[0xAB, 0x08, 0x00, 0x00, 0x00, 0x01, 0x02, 0xB6];

/// Confirms a successful transfer (no response expected).
pub const CMD_TRANSFER_SUCCESS: &[u8] = &[0x57, 0x08, 0x00, 0x00, 0x00, 0x02, 0x01, 0x62];

/// Polls whether a unit is connected; device replies with 4 bytes (byte 0 == 0x11 → connected).
pub const CMD_CHECK_CONNECTED: &[u8] = &[0xF4];

/// Requests device identification; device replies with 76 bytes.
pub const CMD_LOAD_UNIT_INFO: &[u8] = &[
    0x56, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x45, 0x00, 0x00, 0xA7,
];

/// Requests the number of stored log headers; device replies with 8 bytes (`reply[5]` = count).
pub const CMD_GET_LOG_HEADER_COUNT: &[u8] = &[
    0x54, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x60,
];

/// Flash address where log headers end (top of log flash space).
pub const LOG_HEADER_END: u32 = 0x1F_FFFF;

/// Builds a `86 12 00 00 00 <start 24-bit LE> <len 24-bit LE> <checksum>` flash-read command.
pub fn build_flash_read_cmd(start: u32, len: u32) -> Vec<u8> {
    let mut cmd = vec![
        0x56,
        0x0C,
        0x00,
        0x00,
        0x00,
        (start & 0xFF) as u8,
        (start >> 8 & 0xFF) as u8,
        (start >> 16 & 0xFF) as u8,
        (len & 0xFF) as u8,
        (len >> 8 & 0xFF) as u8,
        (len >> 16 & 0xFF) as u8,
    ];
    let checksum: u8 = cmd.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    cmd.push(checksum);
    cmd
}
