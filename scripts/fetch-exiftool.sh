#!/usr/bin/env bash
# Download a portable ExifTool into resources/exiftool/ (gitignored).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/resources/exiftool"
VERSION="${EXIFTOOL_VERSION:-12.25}"
URL="https://sourceforge.net/projects/exiftool/files/Image-ExifTool-${VERSION}.tar.gz/download"
TMP="$(mktemp -d)"

cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

echo "Fetching ExifTool ${VERSION}..."
curl -L --fail --silent --show-error -o "$TMP/exiftool.tar.gz" "$URL"
tar -xzf "$TMP/exiftool.tar.gz" -C "$TMP"

SRC="$(find "$TMP" -maxdepth 1 -type d -name 'Image-ExifTool-*' | head -n 1)"
if [[ -z "$SRC" ]]; then
  echo "Could not find extracted ExifTool directory" >&2
  exit 1
fi

mkdir -p "$DEST"
# Preserve .keep; replace everything else.
find "$DEST" -mindepth 1 -maxdepth 1 ! -name '.keep' -exec rm -rf {} +
cp "$SRC/exiftool" "$DEST/exiftool"
cp -R "$SRC/lib" "$DEST/lib"
chmod +x "$DEST/exiftool"

echo "Installed to $DEST"
"$DEST/exiftool" -ver
