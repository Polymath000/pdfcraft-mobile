//! In-app mobile file picker for Android and mobile touch devices.

use std::path::PathBuf;

use egui::{
    Align, Align2, Color32, CornerRadius, Layout, Rect, RichText, ScrollArea, Sense, Stroke,
    TextEdit, vec2,
};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{icons, widgets};

/// Item representation in the file picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePickerItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size_bytes: u64,
}

/// State for the mobile file picker modal dialog.
#[derive(Clone, Debug)]
pub struct MobileFilePicker {
    pub current_dir: PathBuf,
    pub search_query: String,
    pub items: Vec<FilePickerItem>,
    pub error_message: Option<String>,
}

impl Default for MobileFilePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileFilePicker {
    /// Create a new mobile file picker starting at default location.
    pub fn new() -> Self {
        let current_dir = default_initial_dir();
        let mut picker = Self {
            current_dir,
            search_query: String::new(),
            items: Vec::new(),
            error_message: None,
        };
        picker.refresh();
        picker
    }

    /// Refresh directory listing.
    pub fn refresh(&mut self) {
        self.items.clear();
        self.error_message = None;

        let read_res = std::fs::read_dir(&self.current_dir);
        match read_res {
            Ok(entries) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();

                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip hidden files/directories starting with '.'
                    if name.starts_with('.') {
                        continue;
                    }

                    let file_type = entry.file_type();
                    let is_dir = file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false);
                    let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);

                    if is_dir {
                        dirs.push(FilePickerItem {
                            name,
                            path,
                            is_dir: true,
                            size_bytes: 0,
                        });
                    } else {
                        // Show PDFs and convertible files
                        let lower = name.to_lowercase();
                        if lower.ends_with(".pdf")
                            || lower.ends_with(".png")
                            || lower.ends_with(".jpg")
                            || lower.ends_with(".jpeg")
                            || lower.ends_with(".txt")
                        {
                            files.push(FilePickerItem {
                                name,
                                path,
                                is_dir: false,
                                size_bytes,
                            });
                        }
                    }
                }

                dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

                self.items.extend(dirs);
                self.items.extend(files);
            }
            Err(e) => {
                self.error_message = Some(format!("{}: {e}", tl!("Could not access folder")));
            }
        }
    }

    /// Navigate into a subdirectory.
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.current_dir = path;
        self.search_query.clear();
        self.refresh();
    }

    /// Navigate to parent folder.
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            if !parent.as_os_str().is_empty() {
                self.current_dir = parent.to_path_buf();
                self.search_query.clear();
                self.refresh();
            }
        }
    }

    /// Render the file picker modal dialog. Returns `Some(PathBuf)` when a file is selected.
    /// Sets `close` to true if cancelled.
    pub fn show(&mut self, ctx: &egui::Context, close: &mut bool) -> Option<PathBuf> {
        let t = Tokens::get(ctx);
        let screen_rect = ctx.content_rect();
        let mut selected_file: Option<PathBuf> = None;

        let modal_width = (screen_rect.width() - 24.0).clamp(280.0, 560.0);
        let modal_height = (screen_rect.height() - 48.0).clamp(360.0, 720.0);

        let mut nav_to: Option<PathBuf> = None;
        let mut nav_up = false;

        widgets::show_modal(ctx, "mobile_file_picker", |ui| {
            ui.set_width(modal_width);
            ui.set_height(modal_height);

            // --- Header: Icon + Title + Close Button ---
            ui.horizontal(|ui| {
                icons::paint(ui, Rect::from_min_size(ui.cursor().min, vec2(22.0, 22.0)), "folder-open", 20.0, t.accent);
                ui.add_space(26.0);
                ui.label(RichText::new(tl!("Open File")).font(theme::semibold(18.0)).color(t.text));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icons::button(ui, "x", 24.0, false, tl!("Close")).clicked() {
                        *close = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // --- Quick Access Shortcut Chips ---
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tl!("Locations:")).font(theme::medium(12.0)).color(t.text_muted));
                for (label, path_str) in quick_locations() {
                    let path = PathBuf::from(path_str);
                    let is_active = self.current_dir == path;
                    let chip_resp = widgets::mode_tab(ui, label, is_active);
                    if chip_resp.clicked() {
                        nav_to = Some(path);
                    }
                }
            });

            ui.add_space(8.0);

            // --- Navigation Bar: Up Button + Current Path ---
            ui.horizontal(|ui| {
                let has_parent = self.current_dir.parent().is_some_and(|p| !p.as_os_str().is_empty());
                let up_btn = widgets::ghost_button_sized(ui, "chevron-up", tl!("Up"), 32.0);
                if has_parent && up_btn.clicked() {
                    nav_up = true;
                }

                let path_display = self.current_dir.to_string_lossy();
                let short_path = if path_display.len() > 36 {
                    format!("…{}", &path_display[path_display.len().saturating_sub(33)..])
                } else {
                    path_display.into_owned()
                };

                ui.label(RichText::new(short_path).font(theme::medium(12.0)).color(t.text_muted));
            });

            ui.add_space(6.0);

            // --- Search / Filter bar ---
            ui.horizontal(|ui| {
                icons::paint(ui, Rect::from_min_size(ui.cursor().min, vec2(18.0, 18.0)), "search", 16.0, t.text_faint);
                ui.add_space(22.0);
                ui.add(
                    TextEdit::singleline(&mut self.search_query)
                        .hint_text(tl!("Filter files..."))
                        .desired_width(ui.available_width() - 8.0)
                        .background_color(t.field),
                );
            });

            ui.add_space(6.0);
            ui.separator();

            // --- Error / Notification display ---
            if let Some(err) = &self.error_message {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(err).color(Color32::from_rgb(230, 80, 80)).font(theme::medium(12.0)));
                });
                ui.add_space(4.0);
            }

            // --- Scrollable Files & Directories List ---
            let query = self.search_query.trim().to_lowercase();
            let filtered: Vec<&FilePickerItem> = self
                .items
                .iter()
                .filter(|item| query.is_empty() || item.name.to_lowercase().contains(&query))
                .collect();

            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(32.0);
                    icons::paint(ui, Rect::from_center_size(ui.cursor().center(), vec2(36.0, 36.0)), "file-text", 32.0, t.text_faint);
                    ui.add_space(40.0);
                    ui.label(RichText::new(tl!("No PDF files found in this folder")).font(theme::medium(14.0)).color(t.text_muted));
                    ui.add_space(8.0);
                    ui.label(RichText::new(tl!("Tap a location chip above to check Downloads or Documents.")).font(theme::regular(12.0)).color(t.text_faint));
                });
            } else {
                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for item in filtered {
                        let is_dir = item.is_dir;
                        let row_height = if is_dir { 44.0 } else { 50.0 };
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());
                        let press = theme::Press::track(ui, &resp);

                        let bg_fill = if resp.hovered() {
                            t.hover
                        } else {
                            Color32::TRANSPARENT
                        };

                        ui.painter().rect(
                            rect,
                            CornerRadius::same(6),
                            bg_fill,
                            Stroke::NONE,
                            egui::StrokeKind::Inside,
                        );

                        let content_rect = rect.translate(press.offset());

                        // Icon
                        let icon_name = if is_dir { "folder" } else { "file-text" };
                        let icon_tint = if is_dir { t.accent } else { t.text };
                        let icon_box = Rect::from_min_size(content_rect.min + vec2(10.0, (row_height - 22.0) / 2.0), vec2(22.0, 22.0));
                        icons::paint(ui, icon_box, icon_name, 20.0, icon_tint);

                        // File / Folder Name & Detail
                        let text_left = content_rect.min.x + 40.0;
                        if is_dir {
                            ui.painter().text(
                                egui::pos2(text_left, content_rect.center().y),
                                Align2::LEFT_CENTER,
                                &item.name,
                                theme::medium(14.0),
                                t.text,
                            );

                            let chevron_box = Rect::from_min_size(content_rect.right_top() + vec2(-28.0, (row_height - 18.0) / 2.0), vec2(18.0, 18.0));
                            icons::paint(ui, chevron_box, "chevron-right", 16.0, t.text_faint);
                        } else {
                            ui.painter().text(
                                egui::pos2(text_left, content_rect.min.y + 14.0),
                                Align2::LEFT_CENTER,
                                &item.name,
                                theme::medium(13.5),
                                t.text,
                            );

                            let size_str = human_size(item.size_bytes as usize);
                            ui.painter().text(
                                egui::pos2(text_left, content_rect.min.y + 34.0),
                                Align2::LEFT_CENTER,
                                size_str,
                                theme::regular(11.0),
                                t.text_muted,
                            );
                        }

                        if theme::hand(resp).clicked() {
                            if is_dir {
                                nav_to = Some(item.path.clone());
                                break;
                            } else {
                                selected_file = Some(item.path.clone());
                                *close = true;
                                break;
                            }
                        }
                    }
                });
            }
        });

        if nav_up {
            self.navigate_up();
        } else if let Some(path) = nav_to {
            self.navigate_to(path);
        }

        selected_file
    }
}

/// Determine default initial folder based on platform.
fn default_initial_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        for candidate in [
            "/sdcard/Download",
            "/storage/emulated/0/Download",
            "/sdcard/Documents",
            "/storage/emulated/0/Documents",
            "/sdcard",
            "/storage/emulated/0",
        ] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return p;
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        if let Ok(dir) = std::env::current_dir() {
            return dir;
        }
    }

    PathBuf::from("/")
}

/// Quick shortcut locations for the file picker chips.
fn quick_locations() -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "android")]
    {
        vec![
            ("Downloads", "/sdcard/Download"),
            ("Documents", "/sdcard/Documents"),
            ("Storage", "/sdcard"),
            ("Android Data", "/sdcard/Android/data/com.storytold.pdfcraft/files"),
        ]
    }

    #[cfg(not(target_os = "android"))]
    {
        vec![("Current", "."), ("Home", "..")]
    }
}
