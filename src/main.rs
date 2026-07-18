#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use serde_json::Value;

const BG: Color32 = Color32::from_rgb(245, 246, 248);
const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
const TEXT: Color32 = Color32::from_rgb(28, 30, 34);
const MUTED: Color32 = Color32::from_rgb(110, 116, 128);
const LINE: Color32 = Color32::from_rgb(220, 224, 230);
const DROP_BG: Color32 = Color32::from_rgb(236, 239, 244);
const DROP_HOVER: Color32 = Color32::from_rgb(224, 236, 255);
const ACCENT: Color32 = Color32::from_rgb(20, 105, 220);
const FAIL: Color32 = Color32::from_rgb(196, 48, 48);
const COL_COUNT: f32 = 64.0;
const WINDOW_SIZE: f32 = 360.0;
const FOOTER_HEIGHT: f32 = 34.0;

/// Keys ExifCleaner removes before counting `Object.keys(...).length`.
const IGNORED_COUNT_KEYS: &[&str] = &["SourceFile", "ImageSize", "Megapixels"];

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CRS EXIF Cleaner")
            .with_inner_size([WINDOW_SIZE, WINDOW_SIZE])
            .with_min_inner_size([WINDOW_SIZE, WINDOW_SIZE])
            .with_max_inner_size([WINDOW_SIZE, WINDOW_SIZE])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_fullscreen(false)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "CRS EXIF Cleaner",
        options,
        Box::new(|cc| Ok(Box::new(CleanerApp::new(cc)))),
    )
}

#[derive(Clone)]
enum CleanState {
    Cleaning,
    Done { before: usize, after: usize },
    Failed(String),
}

struct FileRow {
    id: u64,
    name: String,
    state: CleanState,
}

struct CleanResult {
    id: u64,
    result: Result<(usize, usize), String>,
}

struct CleanerApp {
    rows: Vec<FileRow>,
    next_id: u64,
    stay_on_top: bool,
    result_tx: Sender<CleanResult>,
    result_rx: Receiver<CleanResult>,
}

impl CleanerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = BG;
        visuals.window_fill = BG;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.widgets.inactive.bg_fill = SURFACE;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(170, 176, 186));
        visuals.widgets.hovered.bg_fill = DROP_HOVER;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.active.bg_fill = ACCENT;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.scroll.floating = false;
            style.spacing.icon_width = 16.0;
            style.spacing.icon_width_inner = 10.0;
            style.spacing.icon_spacing = 8.0;
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        });

        let (result_tx, result_rx) = mpsc::channel();
        Self {
            rows: Vec::new(),
            next_id: 0,
            stay_on_top: false,
            result_tx,
            result_rx,
        }
    }

    fn enqueue(&mut self, paths: Vec<PathBuf>) {
        // A new drop replaces the previous batch entirely.
        self.rows.clear();

        for path in paths {
            let id = self.next_id;
            self.next_id += 1;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .map(display_file_name)
                .unwrap_or_else(|| "Unnamed file".to_owned());

            self.rows.push(FileRow {
                id,
                name,
                state: CleanState::Cleaning,
            });

            let tx = self.result_tx.clone();
            thread::spawn(move || {
                let result = clean_file(&path).map_err(|error| format!("{error:#}"));
                let _ = tx.send(CleanResult { id, result });
            });
        }
    }

    fn receive_results(&mut self) {
        while let Ok(message) = self.result_rx.try_recv() {
            if let Some(row) = self.rows.iter_mut().find(|row| row.id == message.id) {
                row.state = match message.result {
                    Ok((before, after)) => CleanState::Done { before, after },
                    Err(error) => CleanState::Failed(error),
                };
            }
        }
    }
}

impl eframe::App for CleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_results();

        let dropped = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        if !dropped.is_empty() {
            self.enqueue(dropped);
        }

        let is_hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                let content_height =
                    (ui.available_height() - FOOTER_HEIGHT - 10.0).max(120.0);

                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), content_height),
                    Layout::top_down(Align::Center),
                    |ui| {
                        if self.rows.is_empty() || is_hovering {
                            drop_zone(ui, is_hovering);
                        } else {
                            egui::Frame::new()
                                .fill(SURFACE)
                                .stroke(Stroke::new(1.0, LINE))
                                .corner_radius(10)
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.set_min_size(ui.available_size());
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            file_table(ui, &self.rows);
                                        });
                                });
                        }
                    },
                );

                ui.add_space(10.0);
                if stay_on_top_checkbox(ui, &mut self.stay_on_top) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        if self.stay_on_top {
                            egui::WindowLevel::AlwaysOnTop
                        } else {
                            egui::WindowLevel::Normal
                        },
                    ));
                }
            });

        if self
            .rows
            .iter()
            .any(|row| matches!(row.state, CleanState::Cleaning))
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }
}

fn stay_on_top_checkbox(ui: &mut egui::Ui, stay_on_top: &mut bool) -> bool {
    let mut changed = false;

    ui.vertical_centered(|ui| {
        let fill = if *stay_on_top {
            Color32::from_rgb(232, 240, 255)
        } else {
            Color32::from_rgb(250, 251, 252)
        };
        let stroke = if *stay_on_top {
            Stroke::new(1.0, Color32::from_rgb(180, 206, 245))
        } else {
            Stroke::new(1.0, LINE)
        };

        egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(14, 6))
            .show(ui, |ui| {
                let label = RichText::new("Stay on Top")
                    .size(12.5)
                    .color(if *stay_on_top { ACCENT } else { MUTED });

                let response = ui.add(egui::Checkbox::new(stay_on_top, label));
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                changed = response.changed();
            });
    });

    changed
}

fn drop_zone(ui: &mut egui::Ui, is_hovering: bool) {
    egui::Frame::new()
        .fill(if is_hovering { DROP_HOVER } else { DROP_BG })
        .stroke(Stroke::new(
            1.5,
            if is_hovering {
                ACCENT
            } else {
                Color32::from_rgb(196, 204, 216)
            },
        ))
        .corner_radius(12)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(if is_hovering {
                        "Release to clean"
                    } else {
                        "Drop photos here"
                    })
                    .size(16.0)
                    .strong()
                    .color(TEXT),
                );
            });
        });
}

fn file_table(ui: &mut egui::Ui, rows: &[FileRow]) {
    let file_width = (ui.available_width() - COL_COUNT * 2.0 - 24.0).max(140.0);

    egui::Grid::new("file_table")
        .num_columns(3)
        .spacing([12.0, 4.0])
        .min_col_width(COL_COUNT)
        .striped(true)
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(file_width, 28.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(RichText::new("File").size(11.0).strong().color(MUTED));
                },
            );
            count_label(ui, "Before", MUTED, true);
            count_label(ui, "After", MUTED, true);
            ui.end_row();

            for row in rows {
                ui.allocate_ui_with_layout(
                    egui::vec2(file_width, 30.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&row.name).size(13.5).color(TEXT))
                                .truncate(),
                        )
                        .on_hover_text(&row.name);
                    },
                );

                match &row.state {
                    CleanState::Cleaning => {
                        count_label(ui, "…", MUTED, false);
                        ui.allocate_ui_with_layout(
                            egui::vec2(COL_COUNT, 30.0),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                ui.spinner();
                            },
                        );
                    }
                    CleanState::Done { before, after } => {
                        count_label(ui, &before.to_string(), TEXT, false);
                        count_label(
                            ui,
                            &after.to_string(),
                            if *after < *before { ACCENT } else { TEXT },
                            false,
                        );
                    }
                    CleanState::Failed(error) => {
                        count_label(ui, "—", MUTED, false);
                        ui.allocate_ui_with_layout(
                            egui::vec2(COL_COUNT, 30.0),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                ui.label(RichText::new("Failed").size(13.0).color(FAIL).strong())
                                    .on_hover_text(error);
                            },
                        );
                    }
                }
                ui.end_row();
            }
        });
}

fn count_label(ui: &mut egui::Ui, text: &str, color: Color32, header: bool) {
    ui.allocate_ui_with_layout(
        egui::vec2(COL_COUNT, if header { 28.0 } else { 30.0 }),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.label(
                RichText::new(text)
                    .size(if header { 11.0 } else { 13.5 })
                    .color(color)
                    .strong(),
            );
        },
    );
}

fn clean_file(path: &Path) -> Result<(usize, usize)> {
    if !path.is_file() {
        return Err(anyhow!("Only individual files are supported"));
    }

    let exiftool = find_exiftool()?;
    let before = count_tags_exiftool(&exiftool, path)?;
    strip_metadata_exiftool(&exiftool, path)?;
    let after = count_tags_exiftool(&exiftool, path)?;
    Ok((before, after))
}

/// Same counting model as ExifCleaner / node-exiftool:
/// `readMetadata(file, ["charset filename=UTF8", "-File:all", "-ExifToolVersion"])`
/// then drop SourceFile / ImageSize / Megapixels and use `Object.keys(...).length`.
fn count_tags_exiftool(exiftool: &Path, path: &Path) -> Result<usize> {
    let output = Command::new(exiftool)
        .args([
            "--File:all",
            "--ExifToolVersion",
            "-json",
            "-s",
            "-charset",
            "filename=UTF8",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("Could not run {}", exiftool.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ExifTool could not read metadata: {}",
            stderr.trim()
        ));
    }

    let tags: Vec<BTreeMap<String, Value>> =
        serde_json::from_slice(&output.stdout).context("Could not parse ExifTool JSON output")?;
    let Some(mut record) = tags.into_iter().next() else {
        return Ok(0);
    };

    for key in IGNORED_COUNT_KEYS {
        record.remove(*key);
    }

    Ok(record.len())
}

/// Same clean model as ExifCleaner: `writeMetadata(file, { all: "" }, ...)`.
/// `-P` keeps the original Date Modified.
fn strip_metadata_exiftool(exiftool: &Path, path: &Path) -> Result<()> {
    let output = Command::new(exiftool)
        .args([
            "-all=",
            "-overwrite_original",
            "-charset",
            "filename=UTF8",
            "-P",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("Could not run {}", exiftool.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "ExifTool could not clean metadata: {}",
            stderr.trim()
        ));
    }

    Ok(())
}

fn find_exiftool() -> Result<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(custom) = env::var("EXIFTOOL_PATH") {
        candidates.push(PathBuf::from(custom));
    }

    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("exiftool"));
        candidates.push(dir.join("resources/exiftool/exiftool"));
        // macOS .app (cargo-bundle): Contents/MacOS → Contents/Resources/...
        candidates.push(dir.join("../Resources/exiftool/exiftool"));
        candidates.push(dir.join("../Resources/resources/exiftool/exiftool"));
    }

    // Dev builds: resources next to Cargo.toml
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/exiftool/exiftool"));

    for path in &candidates {
        if path.is_file() {
            return Ok(path.canonicalize().unwrap_or_else(|_| path.clone()));
        }
    }

    // Last resort: PATH
    if let Ok(output) = Command::new("which").arg("exiftool").output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    Err(anyhow!(
        "ExifTool not found. Expected resources/exiftool/exiftool next to the app."
    ))
}

/// macOS screenshot names use narrow no-break spaces (U+202F) before AM/PM.
/// egui's default fonts don't include that glyph, so replace Unicode spaces for display.
fn display_file_name(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_whitespace() { ' ' } else { ch })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::{Duration, SystemTime};

    fn sample_png_with_exif(path: &Path) {
        // Minimal valid PNG via ImageMagick/sips unavailable — write via Python pillow if present,
        // otherwise copy a tiny fixture built with printf PNG + exiftool.
        // Use macOS `sips` to make a png, then exiftool to add a tag.
        let status = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                "/System/Library/CoreServices/DefaultDesktop.heic",
            ])
            .arg("--out")
            .arg(path)
            .status();

        if status.map(|s| s.success()).unwrap_or(false) && path.is_file() {
            let exiftool = find_exiftool().unwrap();
            let _ = Command::new(&exiftool)
                .args([
                    "-UserComment=Test",
                    "-overwrite_original",
                    "-charset",
                    "filename=UTF8",
                ])
                .arg(path)
                .status();
            return;
        }

        // Fallback: require the Dropbox screenshot used in development.
        let fallback = PathBuf::from(
            "/Users/fred/Library/CloudStorage/Dropbox/Screenshots/Screenshot 2026-06-18 at 9.20.04\u{202f}PM.png",
        );
        if fallback.is_file() {
            fs::copy(&fallback, path).unwrap();
            return;
        }

        panic!("Could not create a sample PNG for tests");
    }

    #[test]
    fn matches_exifcleaner_count_on_screenshot() {
        let path = PathBuf::from(
            "/Users/fred/Library/CloudStorage/Dropbox/Screenshots/Screenshot 2026-06-18 at 9.20.04\u{202f}PM.png",
        );
        if !path.is_file() {
            return;
        }

        let exiftool = find_exiftool().unwrap();
        let before = count_tags_exiftool(&exiftool, &path).unwrap();
        assert_eq!(before, 45);
    }

    #[test]
    fn cleans_and_preserves_modification_time() {
        let dir = tempfile_dir();
        let path = dir.join("photo.png");
        sample_png_with_exif(&path);

        let original = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let times = fs::FileTimes::new()
            .set_accessed(original)
            .set_modified(original);
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(times)
            .unwrap();

        let (before, after) = clean_file(&path).unwrap();
        assert!(before > after);

        let restored = fs::metadata(&path).unwrap().modified().unwrap();
        let delta = restored
            .duration_since(original)
            .or_else(|_| original.duration_since(restored))
            .unwrap();
        assert!(delta < Duration::from_secs(2));
    }

    #[test]
    fn normalizes_macos_narrow_spaces_in_display_names() {
        let raw = "Screenshot 2026-07-10 at 4.21.06\u{202f}PM.png";
        assert_eq!(
            display_file_name(raw),
            "Screenshot 2026-07-10 at 4.21.06 PM.png"
        );
    }

    fn tempfile_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!("crs-exif-cleaner-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
