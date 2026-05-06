#!/usr/bin/env bash
# Generates test sleep screen PNG bitmaps in bitmaps/.
# Uses ImageMagick for pixel drawing and Python for injecting PNG tEXt metadata chunks.
# Usage: bash scripts/generate_bitmaps.sh
set -euo pipefail

MAGICK=$(command -v magick 2>/dev/null || command -v convert)
OUTDIR="$(dirname "$0")/../bitmaps"
mkdir -p "$OUTDIR"

# ---------------------------------------------------------------------------
# Dot coordinate translation from SleepScreenSign.as bikeAndHills():
#   XML: <dot x="col_on_56-wide_sign" y="row_on_14-high_sign"/>
#   getBytes(xml, param2=16, param3=59) iterates:
#     outer _loc9_=0..58, inner _loc16_=0..15
#     lookup key: _loc16_ + "," + _loc9_
#     storage key: @y-1 + "," + @x-1
#   → ImageMagick pixel: x(col) = y_xml-1,  y(row) = x_xml-1
#
# clockX=27, clockY=4, myNamePos="top"  (from bikeAndHills XML)
# ---------------------------------------------------------------------------

DOTS=(
  "4,0"  "8,0"
  "4,1"  "8,1"
  "4,2"  "5,2"  "8,2"
  "5,3"  "6,3"  "8,3"
  "6,4"  "7,4"  "8,4"
  "7,5"  "8,5"
  "3,6"  "4,6"  "5,6"  "8,6"  "9,6"
  "2,7"  "3,7"  "5,7"  "6,7"  "7,7"  "8,7"  "9,7"  "10,7"
  "2,8"  "7,8"  "8,8"  "11,8"
  "1,9"  "6,9"  "11,9"
  "1,10" "5,10" "7,10" "11,10"
  "1,11" "4,11" "7,11" "9,11" "11,11"
  "1,12" "4,12" "7,12" "9,12" "11,12"
  "1,13" "7,13" "10,13"
  "1,14" "7,14"
  "2,15" "6,15"
  "2,16" "3,16" "5,16" "6,16"
  "3,17" "4,17" "5,17"
  "7,18"
  "8,19"
  "8,20" "9,20"
  "9,21"
  "9,22" "10,22"
  "10,23"
  "10,24" "11,24"
  "11,25"
  "10,26"
  "10,27"
  "9,28"
  "9,29"
  "8,30" "9,30"
  "8,31"
  "9,32"
  "10,33"
  "11,34"
  "12,35"
  "12,36"
  "13,37"
  "12,38" "13,38"
  "11,39" "12,39"
  "10,40" "11,40"
  "9,41"  "10,41"
  "8,42"  "9,42"  "10,42"
  "8,43"
  "7,44"  "8,44"
  "6,45"  "7,45"
  "6,46"
  "7,47"
  "7,48"  "8,48"
  "9,49"
  "10,50"
  "11,51"
  "10,52" "11,52"
  "9,53"  "10,53"
  "9,54"
  "8,55"
)

TMP=$(mktemp /tmp/sleep_raw_XXXXXX.png)

# Build -draw arguments
DRAW_ARGS=()
for dot in "${DOTS[@]}"; do
  DRAW_ARGS+=(-draw "point $dot")
done

# Create 16×59 1-bit PNG with white dots on black background
"$MAGICK" -size 16x59 xc:black \
  -fill white \
  "${DRAW_ARGS[@]}" \
  -type Grayscale -depth 1 \
  PNG:"$TMP"

# Inject tEXt metadata chunks (clock position + name position)
python3 - "$TMP" "$OUTDIR/bike_and_hills.png" <<'PYEOF'
import struct, zlib, sys

def text_chunk(keyword, value):
    data = keyword.encode('latin-1') + b'\x00' + value.encode('latin-1')
    crc = zlib.crc32(b'tEXt' + data) & 0xFFFFFFFF
    return struct.pack('>I', len(data)) + b'tEXt' + data + struct.pack('>I', crc)

with open(sys.argv[1], 'rb') as f:
    png = f.read()

# Insert tEXt chunks right after IHDR (8-byte sig + 25-byte IHDR = 33 bytes)
insert = (
    text_chunk('clock_x', '27') +
    text_chunk('clock_y', '4') +
    text_chunk('name_pos', 'top')
)
with open(sys.argv[2], 'wb') as f:
    f.write(png[:33] + insert + png[33:])
print(f"Written: {sys.argv[2]}")
PYEOF

rm -f "$TMP"
