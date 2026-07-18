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
cargo run --release
```

## Build a macOS app

```sh
./scripts/fetch-exiftool.sh
cargo install cargo-bundle
cargo bundle --release
```

The app is written to `target/release/bundle/osx/CRS EXIF Cleaner.app`.

## Important

Cleaning is immediate and changes the dropped files themselves. Date Modified
is preserved (`exiftool -P`). Keep backups of irreplaceable photos.
