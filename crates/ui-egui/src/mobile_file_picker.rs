use std::path::{Path, PathBuf};

use std::time::SystemTime;


use egui::{

    Align, Align2, Color32, CornerRadius, Layout, Rect, RichText, ScrollArea, Sense, Stroke,

    TextEdit, vec2,

};


use crate::panels::human_size;

use crate::theme::{self, Tokens};

use crate::{icons, widgets};


#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]

pub enum FileSortKey {

    #[default]

    Name,

    Date,

    Size,

}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]

pub enum FileSortOrder {

    #[default]

    Ascending,

    Descending,

}


#[derive(Clone, Debug, PartialEq, Eq)]

pub struct FilePickerItem {

    pub name: String,

    pub path: PathBuf,

    pub is_dir: bool,

    pub size_bytes: u64,

    pub modified: Option<SystemTime>,

}


#[derive(Clone, Debug)]

pub struct MobileFilePicker {

    pub current_dir: PathBuf,

    pub previous_dir: Option<PathBuf>,

    pub search_query: String,

    pub items: Vec<FilePickerItem>,

    pub sort_key: FileSortKey,

    pub sort_order: FileSortOrder,

    pub error_message: Option<String>,

}


impl Default for MobileFilePicker {

    fn default() -> Self {

        Self::new()

    }

}


impl MobileFilePicker {

    pub fn new() -> Self {

        let current_dir = default_initial_dir();

        let mut picker = Self {

            current_dir,

            previous_dir: None,

            search_query: String::new(),

            items: Vec::new(),

            sort_key: FileSortKey::Name,

            sort_order: FileSortOrder::Ascending,

            error_message: None,

        };

        picker.refresh();

        picker

    }


    pub fn sort_items(&mut self) {

        let sort_key = self.sort_key;

        let sort_order = self.sort_order;

        self.items.sort_by(|a, b| {

            if a.is_dir != b.is_dir {

                return if a.is_dir {

                    std::cmp::Ordering::Less

                } else {

                    std::cmp::Ordering::Greater

                };

            }

            let ord = match sort_key {

                FileSortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),

                FileSortKey::Date => match (a.modified, b.modified) {

                    (Some(ta), Some(tb)) => ta.cmp(&tb),

                    (Some(_), None) => std::cmp::Ordering::Greater,

                    (None, Some(_)) => std::cmp::Ordering::Less,

                    (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),

                },

                FileSortKey::Size => {

                    if a.is_dir {

                        a.name.to_lowercase().cmp(&b.name.to_lowercase())

                    } else {

                        a.size_bytes.cmp(&b.size_bytes)

                    }

                }

            };

            let ord = if sort_order == FileSortOrder::Descending {

                ord.reverse()

            } else {

                ord

            };

            ord.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))

        });

    }


    pub fn refresh(&mut self) {

        self.items.clear();

        self.error_message = None;


        match std::fs::read_dir(&self.current_dir) {

            Ok(entries) => {

                for entry in entries.flatten() {

                    let path = entry.path();

                    let name = entry.file_name().to_string_lossy().to_string();


                    if name.starts_with('.') {

                        continue;

                    }


                    let file_type = entry.file_type();

                    let is_dir = file_type.as_ref().map(|t| t.is_dir()).unwrap_or(false);

                    let metadata = entry.metadata().ok();

                    let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

                    let modified = metadata.as_ref().and_then(|m| m.modified().ok());


                    if is_dir {

                        self.items.push(FilePickerItem {

                            name,

                            path,

                            is_dir: true,

                            size_bytes: 0,

                            modified,

                        });

                    } else {

                        let lower = name.to_lowercase();

                        if lower.ends_with(".pdf")

                            || lower.ends_with(".png")

                            || lower.ends_with(".jpg")

                            || lower.ends_with(".jpeg")

                            || lower.ends_with(".txt")

                        {

                            self.items.push(FilePickerItem {

                                name,

                                path,

                                is_dir: false,

                                size_bytes,

                                modified,

                            });

                        }

                    }

                }

                self.sort_items();

            }

            Err(e) => {

                let desc = if e.kind() == std::io::ErrorKind::PermissionDenied {

                    tl!("Permission denied: cannot access this folder.")

                } else if e.kind() == std::io::ErrorKind::NotFound {

                    tl!("Folder not found.")

                } else {

                    tl!("Could not access folder.")

                };

                self.error_message = Some(format!("{desc} ({e})"));

            }

        }

    }


    pub fn navigate_to(&mut self, path: PathBuf) {

        if path != self.current_dir {

            self.previous_dir = Some(self.current_dir.clone());

            self.current_dir = path;

            self.search_query.clear();

            self.refresh();

        }

    }


    pub fn navigate_up(&mut self) {

        if let Some(parent) = self.current_dir.parent() {

            if !parent.as_os_str().is_empty() {

                let parent_buf = parent.to_path_buf();

                self.navigate_to(parent_buf);

            }

        }

    }


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


            ui.horizontal(|ui| {

                icons::paint(ui, Rect::from_min_size(ui.cursor().min, vec2(22.0, 22.0)), "folder-open", 20.0, t.accent);

                ui.add_space(26.0);

                ui.label(RichText::new(tl!("Open File")).font(theme::semibold(18.0)).color(t.text));


                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

                    if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {

                        *close = true;

                    }

                });

            });


            ui.add_space(8.0);

            ui.separator();

            ui.add_space(4.0);


            ui.horizontal_wrapped(|ui| {

                ui.label(RichText::new(tl!("Locations:")).font(theme::medium(12.0)).color(t.text_muted));

                for (label, path) in quick_locations() {

                    let is_active = is_same_path(&self.current_dir, &path);

                    let chip_resp = chip_button(ui, label, is_active);

                    if chip_resp.clicked() {

                        nav_to = Some(path);

                    }

                }

            });


            ui.add_space(8.0);


            ui.horizontal(|ui| {

                let has_parent = self.current_dir.parent().is_some_and(|p| !p.as_os_str().is_empty());

                let up_btn = widgets::ghost_button_sized(ui, "chevron-up", tl!("Up"), 38.0);

                if has_parent && up_btn.clicked() {

                    nav_up = true;

                }


                let folder_name = self.current_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| self.current_dir.to_string_lossy().to_string());

                let path_display = self.current_dir.to_string_lossy().to_string();

                let short_path = if path_display.len() > 38 {

                    format!("…{}", &path_display[path_display.len().saturating_sub(35)..])

                } else {

                    path_display

                };


                ui.vertical(|ui| {

                    ui.label(RichText::new(folder_name).font(theme::semibold(14.0)).color(t.text));

                    ui.label(RichText::new(short_path).font(theme::regular(11.0)).color(t.text_muted));

                });

            });


            ui.add_space(6.0);


            ui.horizontal(|ui| {

                let has_query = !self.search_query.is_empty();

                let search_icon_rect = Rect::from_min_size(ui.cursor().min + vec2(0.0, 8.0), vec2(20.0, 20.0));

                icons::paint(ui, search_icon_rect, "search", 16.0, t.text_faint);

                ui.add_space(26.0);


                let clear_w = if has_query { 36.0 } else { 0.0 };

                let input_w = (ui.available_width() - clear_w).max(120.0);


                ui.add_sized(

                    vec2(input_w, 36.0),

                    TextEdit::singleline(&mut self.search_query)

                        .hint_text(tl!("Filter files..."))

                        .desired_width(input_w)

                        .background_color(t.field),

                );


                if has_query {

                    ui.add_space(4.0);

                    if icons::button(ui, "x", 32.0, false, tl!("Clear search")).clicked() {

                        self.search_query.clear();

                    }

                }

            });


            ui.add_space(6.0);


            ui.horizontal(|ui| {

                ui.label(RichText::new(tl!("Sort:")).font(theme::medium(12.0)).color(t.text_muted));

                for (key, label) in [

                    (FileSortKey::Name, tl!("Name")),

                    (FileSortKey::Date, tl!("Date")),

                    (FileSortKey::Size, tl!("Size")),

                ] {

                    let is_active = self.sort_key == key;

                    let suffix = if is_active {

                        if self.sort_order == FileSortOrder::Ascending {

                            " ↑"

                        } else {

                            " ↓"

                        }

                    } else {

                        ""

                    };

                    let chip_label = format!("{label}{suffix}");

                    let chip_resp = chip_button(ui, &chip_label, is_active);

                    if chip_resp.clicked() {

                        if is_active {

                            self.sort_order = match self.sort_order {

                                FileSortOrder::Ascending => FileSortOrder::Descending,

                                FileSortOrder::Descending => FileSortOrder::Ascending,

                            };

                        } else {

                            self.sort_key = key;

                            self.sort_order = match key {

                                FileSortKey::Name => FileSortOrder::Ascending,

                                FileSortKey::Date | FileSortKey::Size => FileSortOrder::Descending,

                            };

                        }

                        self.sort_items();

                    }

                }

            });


            ui.add_space(6.0);

            ui.separator();


            if let Some(err) = &self.error_message {

                ui.add_space(4.0);

                let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());


                let alert_bg = Color32::from_rgba_unmultiplied(220, 38, 38, 28);

                let alert_border = Color32::from_rgba_unmultiplied(220, 38, 38, 140);

                let alert_text = Color32::from_rgb(239, 68, 68);


                ui.painter().rect(

                    rect,

                    CornerRadius::same(8),

                    alert_bg,

                    Stroke::new(1.0, alert_border),

                    egui::StrokeKind::Inside,

                );


                let icon_rect = Rect::from_min_size(rect.min + vec2(10.0, 18.0), vec2(20.0, 20.0));

                icons::paint(ui, icon_rect, "triangle-alert", 18.0, alert_text);


                let text_pos = egui::pos2(rect.min.x + 36.0, rect.min.y + 16.0);

                widgets::paint_left(ui, text_pos, err, theme::medium(12.0), alert_text, rect.width() - 44.0);


                let hint_pos = egui::pos2(rect.min.x + 36.0, rect.min.y + 36.0);

                widgets::paint_left(

                    ui,

                    hint_pos,

                    tl!("Choose a location above or tap Go Back to return."),

                    theme::regular(11.0),

                    t.text_muted,

                    rect.width() - 44.0,

                );


                ui.add_space(4.0);

            }


            let query = self.search_query.trim().to_lowercase();

            let filtered: Vec<&FilePickerItem> = self

                .items

                .iter()

                .filter(|item| query.is_empty() || item.name.to_lowercase().contains(&query))

                .collect();


            if self.error_message.is_some() && self.items.is_empty() {

                ui.vertical_centered(|ui| {

                    ui.add_space(28.0);

                    icons::paint(ui, Rect::from_center_size(ui.cursor().center(), vec2(36.0, 36.0)), "triangle-alert", 32.0, Color32::from_rgb(239, 68, 68));

                    ui.add_space(40.0);

                    ui.label(RichText::new(tl!("Folder Inaccessible")).font(theme::semibold(15.0)).color(t.text));

                    ui.add_space(8.0);

                    ui.label(RichText::new(tl!("PdfCraft cannot read this directory. Choose another location.")).font(theme::regular(12.0)).color(t.text_muted));

                    ui.add_space(16.0);

                    ui.horizontal(|ui| {

                        if let Some(prev) = &self.previous_dir {

                            if chip_button(ui, tl!("Go Back"), true).clicked() {

                                nav_to = Some(prev.clone());

                            }

                        }

                        if chip_button(ui, tl!("Storage"), false).clicked() {

                            nav_to = Some(default_initial_dir());

                        }

                    });

                });

            } else if filtered.is_empty() {

                ui.vertical_centered(|ui| {

                    ui.add_space(32.0);

                    let empty_icon = if !query.is_empty() { "search" } else { "file-text" };

                    icons::paint(ui, Rect::from_center_size(ui.cursor().center(), vec2(36.0, 36.0)), empty_icon, 32.0, t.text_faint);

                    ui.add_space(40.0);

                    let empty_msg = if !query.is_empty() {

                        tl!("No matching files or folders")

                    } else {

                        tl!("No PDF files found in this folder")

                    };

                    ui.label(RichText::new(empty_msg).font(theme::medium(14.0)).color(t.text_muted));

                    ui.add_space(8.0);

                    let hint_msg = if !query.is_empty() {

                        tl!("Try searching for another name or clear the filter.")

                    } else {

                        tl!("Tap a location chip above to check Downloads or Documents.")

                    };

                    ui.label(RichText::new(hint_msg).font(theme::regular(12.0)).color(t.text_faint));

                    if !query.is_empty() {

                        ui.add_space(16.0);

                        if chip_button(ui, tl!("Clear filter"), true).clicked() {

                            self.search_query.clear();

                        }

                    }

                });

            } else {

                ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {

                    for item in filtered {

                        let is_dir = item.is_dir;

                        let row_height = 54.0;

                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());

                        let press = theme::Press::track(ui, &resp);


                        let bg_fill = press.fill(Color32::TRANSPARENT, t.hover, t.pressed);

                        if bg_fill.a() > 0 {

                            ui.painter().rect_filled(rect, CornerRadius::same(8), bg_fill);

                        }


                        let content_rect = rect.translate(press.offset());

                        let text_left = content_rect.min.x + 42.0;


                        if is_dir {

                            let icon_box = Rect::from_min_size(

                                content_rect.min + vec2(10.0, (row_height - 22.0) / 2.0),

                                vec2(22.0, 22.0),

                            );

                            icons::paint(ui, icon_box, "folder", 20.0, t.accent);


                            let chevron_box = Rect::from_min_size(

                                content_rect.right_top() + vec2(-30.0, (row_height - 18.0) / 2.0),

                                vec2(18.0, 18.0),

                            );

                            icons::paint(ui, chevron_box, "chevron-right", 16.0, t.text_faint);


                            let max_w = (content_rect.max.x - 36.0 - text_left).max(40.0);

                            widgets::paint_left(

                                ui,

                                egui::pos2(text_left, content_rect.min.y + 16.0),

                                &item.name,

                                theme::medium(14.0),

                                t.text,

                                max_w,

                            );

                            widgets::paint_left(

                                ui,

                                egui::pos2(text_left, content_rect.min.y + 36.0),

                                tl!("Folder"),

                                theme::regular(11.0),

                                t.text_faint,

                                max_w,

                            );

                        } else {

                            let icon_name = if item.name.to_lowercase().ends_with(".pdf") {

                                "file-text"

                            } else if item.name.to_lowercase().ends_with(".png")

                                || item.name.to_lowercase().ends_with(".jpg")

                                || item.name.to_lowercase().ends_with(".jpeg")

                            {

                                "image"

                            } else {

                                "file-text"

                            };


                            let icon_box = Rect::from_min_size(

                                content_rect.min + vec2(10.0, (row_height - 22.0) / 2.0),

                                vec2(22.0, 22.0),

                            );

                            icons::paint(ui, icon_box, icon_name, 20.0, t.text);


                            let size_str = human_size(item.size_bytes as usize);

                            let badge_font = theme::medium(11.0);

                            let badge_w = widgets::text_width(ui, &size_str, &badge_font);

                            let badge_rect = Rect::from_min_size(

                                egui::pos2(content_rect.max.x - badge_w - 24.0, content_rect.center().y - 11.0),

                                vec2(badge_w + 14.0, 22.0),

                            );


                            ui.painter().rect(

                                badge_rect,

                                CornerRadius::same(5),

                                t.field,

                                Stroke::new(1.0, t.border),

                                egui::StrokeKind::Inside,

                            );

                            ui.painter().text(

                                badge_rect.center(),

                                Align2::CENTER_CENTER,

                                &size_str,

                                badge_font,

                                t.text_muted,

                            );


                            let max_w = (badge_rect.min.x - 12.0 - text_left).max(40.0);

                            widgets::paint_left(

                                ui,

                                egui::pos2(text_left, content_rect.min.y + 16.0),

                                &item.name,

                                theme::medium(13.5),

                                t.text,

                                max_w,

                            );


                            let meta_str = item.modified.map(crate::combine_ui::ago).unwrap_or_else(|| {

                                if item.name.to_lowercase().ends_with(".pdf") {

                                    "PDF".to_string()

                                } else {

                                    "".to_string()

                                }

                            });

                            widgets::paint_left(

                                ui,

                                egui::pos2(text_left, content_rect.min.y + 36.0),

                                &meta_str,

                                theme::regular(11.0),

                                t.text_muted,

                                max_w,

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


fn chip_button(ui: &mut egui::Ui, label: &str, is_active: bool) -> egui::Response {

    let t = Tokens::get(ui.ctx());

    let font = if is_active { theme::semibold(12.0) } else { theme::medium(12.0) };

    let text_w = widgets::text_width(ui, label, &font);

    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 20.0, 34.0), Sense::click());

    let press = theme::Press::track(ui, &resp);


    let fill = if is_active {

        press.fill(t.accent, t.accent.lerp_to_gamma(Color32::WHITE, 0.1), t.accent.lerp_to_gamma(Color32::BLACK, 0.15))

    } else {

        press.fill(t.field, t.hover, t.pressed)

    };

    let stroke = if is_active {

        Stroke::NONE

    } else {

        Stroke::new(1.0, t.border)

    };

    let text_color = if is_active {

        Color32::WHITE

    } else {

        t.text

    };


    ui.painter().rect(rect, CornerRadius::same(17), fill, stroke, egui::StrokeKind::Inside);

    ui.painter().text(rect.translate(press.offset()).center(), Align2::CENTER_CENTER, label, font, text_color);

    theme::hand(resp)

}


fn is_same_path(a: &Path, b: &Path) -> bool {

    if a == b {

        return true;

    }

    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {

        (Ok(ca), Ok(cb)) => ca == cb,

        _ => false,

    }

}


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


fn quick_locations() -> Vec<(&'static str, PathBuf)> {

    #[cfg(target_os = "android")]

    {

        vec![

            ("Downloads", PathBuf::from("/sdcard/Download")),

            ("Documents", PathBuf::from("/sdcard/Documents")),

            ("Storage", PathBuf::from("/sdcard")),

            ("App Files", PathBuf::from("/sdcard/Android/data/com.storytold.pdfcraft/files")),

        ]

    }


    #[cfg(not(target_os = "android"))]

    {

        let mut locs = Vec::new();

        if let Ok(curr) = std::env::current_dir() {

            locs.push(("Current", curr));

        }

        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().map(PathBuf::from);

        if let Some(h) = &home {

            let downloads = h.join("Downloads");

            if downloads.exists() {

                locs.push(("Downloads", downloads));

            }

            let docs = h.join("Documents");

            if docs.exists() {

                locs.push(("Documents", docs));

            }

            locs.push(("Home", h.clone()));

        }

        locs

    }

}

