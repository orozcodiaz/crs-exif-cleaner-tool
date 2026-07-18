# CRS EXIF Cleaner

A small macOS utility for removing metadata from photos. Drop images into the
window and each original file is cleaned in place.

Counting and cleaning use the same **ExifTool** model as
[ExifCleaner](https://github.com/szTheory/exifcleaner): every ExifTool field
(except file/system noise) is counted, then `-all=` strips metadata.

## Setup

ExifTool is not committed to git. Fetch it once into `resources/exiftool/`:

```sh
./scripts/fetch-exiftool.sh
```

Or point `EXIFTOOL_PATH` at any ExifTool binary on your machine.

## Run locally

```sh
./scripts/fetch-exiftool.sh   # once
cargo run --release
```

## Make a copyable app

```sh
./scripts/fetch-exiftool.sh   # once, if needed
cargo install cargo-bundle    # once
cargo bundle --release
```

Then copy either:

- **macOS app:** `target/release/bundle/osx/CRS EXIF Cleaner.app`  
  (or from `dist/CRS EXIF Cleaner.app` if you copied it there)
- **DMG:** `target/release/bundle/dmg/CRS EXIF Cleaner.dmg`

Or a portable folder (binary + ExifTool side by side):

```sh
mkdir -p dist/portable/resources
cp target/release/exif-cleaner "dist/portable/CRS EXIF Cleaner"
cp -R resources/exiftool dist/portable/resources/
```

Copy the whole `dist/portable` folder — the binary alone is not enough; it needs `resources/exiftool/`.

## Important

Cleaning is immediate and changes the dropped files themselves. Date Modified
is preserved (`exiftool -P`). Keep backups of irreplaceable photos.
