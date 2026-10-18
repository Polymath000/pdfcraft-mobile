

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};


use crate::canvas::{DocView, Fit, PageLayout};

use crate::theme::{self, ThemePreference, Tokens};

use crate::{Dialog, Mode, PdfCraftApp, PropsTab, RightPanel, icons, widgets, window_controls};







pub const MAC_TITLE_BAR: f32 = 28.0;




const MAC_LIGHT: f32 = 12.0;

const MAC_LIGHT_PITCH: f32 = 20.0;

const MAC_LIGHTS_LEFT: f32 = 10.0;



pub const MAC_CONTENT_LEFT: f32 = MAC_LIGHTS_LEFT + 2.0 * MAC_LIGHT_PITCH + MAC_LIGHT + (MAC_TITLE_BAR - MAC_LIGHT) / 2.0;


const TAB_STRIP: f32 = 38.0;

const TAB_HEIGHT: f32 = 30.0;

const STRIP_BUTTON: f32 = 28.0;


pub fn tab_strip(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());



    let mac = cfg!(target_os = "macos") && app.integrated_titlebar;

    let left = if mac { MAC_CONTENT_LEFT as i8 } else { 8 };

    let (tab_h, button) = if mac { (MAC_TITLE_BAR - 2.0, 24.0) } else { (TAB_HEIGHT, STRIP_BUTTON) };

    egui::Panel::top("tab_strip")

        .exact_size(if mac { MAC_TITLE_BAR } else { TAB_STRIP })

        .frame(egui::Frame::NONE.fill(t.titlebar).inner_margin(egui::Margin { left, right: 10, top: 0, bottom: 0 }))

        .show(ui, |ui| {

            let full = ui.max_rect();

            let drag = ui.interact(full, ui.id().with("titledrag"), Sense::click_and_drag());

            if drag.drag_started() {

                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);

            }

            if drag.double_clicked() {

                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));

                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));

            }


            let fullscreen = ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false));

            let controls = app

                .window_controls

                .clone()

                .filter(|_| app.draws_window_frame() && !fullscreen)

                .unwrap_or_else(|| window_controls::Layout { left: Vec::new(), right: Vec::new() });

            ui.horizontal_centered(|ui| {

                ui.spacing_mut().item_spacing.x = 4.0;

                if !controls.left.is_empty() {

                    ui.scope(|ui| {

                        ui.spacing_mut().item_spacing.x = 0.0;

                        window_controls::buttons(ui, &controls.left);

                    });

                    ui.add_space(window_controls::SIDE_GAP - 4.0);

                }

                if icons::button(ui, "house", button, app.active.is_none() && !app.combine_showing(), tl!("Home")).clicked() {

                    app.active = None;

                    app.combine_tab.focused = false;

                }

                let compact = crate::layout_traits::is_compact(ui);

                let controls_width = if compact {

                    button + window_controls::width(&controls.right)

                } else {

                    ui.fonts_mut(|f| f.layout_no_wrap("Discord".into(), theme::medium(13.0), t.text).size().x)

                        + 2.0 * button

                        + 50.0

                        + window_controls::width(&controls.right)

                };


                let open_width = ui.fonts_mut(|f| f.layout_no_wrap(tl!("Open").into(), theme::medium(13.0), t.text).size().x) + 38.0 + 12.0;

                let mut tabs_width = (ui.available_width() - controls_width - open_width).max(0.0);



                let mut natural: Vec<f32> = Vec::with_capacity(app.views.len() + 1);

                for i in 0..app.views.len() {

                    if let Some(name) = crate::canvas::tab_label(app, i) {

                        natural.push(tab_natural_width(ui, &t, &name));

                    }

                }

                if app.combine_tab.open {

                    natural.push(tab_natural_width(ui, &t, tl!("Combine files")));

                }

                let gap = ui.spacing().item_spacing.x;

                let overflow = tabs_overflow(&natural, tabs_width, gap, TAB_MIN_WIDTH);

                if overflow {

                    tabs_width = (tabs_width - 2.0 * (TAB_ARROW + gap)).max(0.0);

                }

                let cap = tab_cap(&natural, tabs_width, gap, TAB_MIN_WIDTH);


                let scroll_id = ui.id().with("tab_scroll");

                let (offset, max_offset) = ui.data(|d| d.get_temp::<(f32, f32)>(scroll_id)).unwrap_or((0.0, 0.0));

                let mut scroll_to = None;

                if overflow && arrow(ui, "chevron-left", offset > 0.5, tl!("Scroll tabs left")) {

                    scroll_to = Some((offset - (TAB_MIN_WIDTH + gap)).max(0.0));

                }



                let state = (app.active_ids().map(|(_, id)| id), app.views.len(), app.combine_showing());

                let changed = ui.data_mut(|data| {

                    let id = ui.id().with("active_tab");

                    let changed = data.get_temp::<(Option<pdfcraft_engine::DocId>, usize, bool)>(id) != Some(state);

                    data.insert_temp(id, state);

                    changed

                });

                let mut action = None;

                let mut close = None;


                let mut drag_x = None;

                ui.scope(|ui| {

                    ui.style_mut().always_scroll_the_only_direction = true;

                    let mut area = egui::ScrollArea::horizontal().id_salt("document_tabs").max_width(tabs_width).auto_shrink([true, true]);

                    if let Some(x) = ui.data_mut(|d| d.remove_temp::<f32>(scroll_id.with("to"))) {

                        area = area.horizontal_scroll_offset(x);

                    }

                    let out = area.show(ui, |ui| {

                        ui.horizontal_centered(|ui| {



                            let mut slots = Vec::with_capacity(app.views.len());

                            let mut dragged = None;

                            for i in 0..app.views.len() {

                                let Some(doc) = app.session.get(app.views[i].id) else { continue };

                                let Some(name) = crate::canvas::tab_label(app, i) else { continue };

                                let dirty = doc.dirty;


                                let file = (name != doc.name).then_some(doc.name.as_str());

                                let response = tab(

                                    ui,

                                    ui.id().with(("document-tab", app.views[i].id)),

                                    &t,

                                    "file-text",

                                    &name,

                                    file,

                                    dirty,

                                    app.active == Some(i),

                                    &mut close,

                                    i,

                                    cap,

                                    tab_h,

                                    true,

                                );

                                slots.push((i, response.rect));

                                if response.dragged() || response.drag_stopped() {

                                    dragged = Some((i, response.drag_stopped()));

                                }

                                if changed && app.active == Some(i) && !app.combine_showing() {

                                    response.scroll_to_me(Some(Align::Center));

                                }

                                let id = app.views[i].id;

                                let reveal = cfg!(not(target_arch = "wasm32")) && doc.path.is_some();

                                response.context_menu(|ui| {

                                    if ui.button(tl!("Close Tab")).clicked() {

                                        action = Some(crate::tab_context::Action::Close(id));

                                        ui.close();

                                    }

                                    if ui.add_enabled(!app.closed_tabs.is_empty(), egui::Button::new(tl!("Reopen Closed Tab"))).clicked() {

                                        action = Some(crate::tab_context::Action::Reopen);

                                        ui.close();

                                    }

                                    let label = if cfg!(target_os = "macos") { tl!("Reveal in Finder") } else { tl!("Reveal in File Explorer") };

                                    if ui.add_enabled(reveal, egui::Button::new(label)).clicked() {

                                        action = Some(crate::tab_context::Action::Reveal(id));

                                        ui.close();

                                    }

                                });

                                if response.clicked() {

                                    app.active = Some(i);

                                }



                                if response.middle_clicked() {

                                    close = Some(i);

                                }

                            }



                            if let Some((from, released)) = dragged

                                && let Some(x) = ui.ctx().pointer_latest_pos().map(|p| p.x)

                            {

                                let to = drop_index(&slots, from, x);

                                if released {

                                    if close.is_none() && action.is_none() {

                                        app.move_tab(from, to);

                                    }

                                } else {

                                    drag_x = Some(x);

                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

                                    if let Some(marker) = drop_marker(&slots, from, to, ui.spacing().item_spacing.x) {

                                        let span = slots.first().map_or(ui.max_rect().y_range(), |(_, r)| r.y_range());



                                        let clip = ui.clip_rect();

                                        let marker = marker.clamp(clip.left() + 1.0, (clip.right() - 1.0).max(clip.left() + 1.0));

                                        ui.painter().vline(marker, span.shrink(4.0), Stroke::new(2.0, t.accent));

                                    }

                                }

                            }

                            if let Some(i) = close

                                && let Some(view) = app.views.get(i)

                            {

                                action.get_or_insert(crate::tab_context::Action::Close(view.id));

                            }

                            if let Some(action) = action {

                                app.tab_action(action);

                            }

                            if app.combine_tab.open {


                                let mut close = None;

                                let response = tab(

                                    ui,

                                    ui.id().with("combine-tab"),

                                    &t,

                                    "files",

                                    tl!("Combine files"),

                                    None,

                                    false,

                                    app.combine_showing(),

                                    &mut close,

                                    usize::MAX,

                                    cap,

                                    tab_h,

                                    false,

                                );

                                if changed && app.combine_showing() {

                                    response.scroll_to_me(Some(Align::Center));

                                }

                                if response.clicked() {

                                    app.open_combine_tab();

                                }

                                if close.is_some() || response.middle_clicked() {

                                    app.close_combine_tab();

                                }

                            }

                        });

                    });

                    let max = (out.content_size.x - out.inner_rect.width()).max(0.0);

                    ui.data_mut(|d| d.insert_temp(scroll_id, (out.state.offset.x, max)));


                    if let Some(x) = drag_x

                        && max > 0.0

                    {

                        let step = if x < out.inner_rect.left() + TAB_DRAG_EDGE {

                            -TAB_DRAG_STEP

                        } else if x > out.inner_rect.right() - TAB_DRAG_EDGE {

                            TAB_DRAG_STEP

                        } else {

                            0.0

                        };

                        if step != 0.0 {

                            ui.data_mut(|d| d.insert_temp(scroll_id.with("to"), (out.state.offset.x + step).clamp(0.0, max)));

                            ui.ctx().request_repaint();

                        }

                    }

                });

                if overflow && arrow(ui, "chevron-right", offset < max_offset - 0.5, tl!("Scroll tabs right")) {

                    scroll_to = Some((offset + TAB_MIN_WIDTH + gap).min(max_offset));

                }

                if let Some(x) = scroll_to {

                    ui.data_mut(|d| d.insert_temp(scroll_id.with("to"), x));

                    ui.ctx().request_repaint();

                }

                ui.add_space(4.0);

                let tip = crate::commands::command_tip(ui.ctx(), tl!("Open a PDF ({key})"), "file.open");

                if widgets::ghost_button_sized(ui, "plus", tl!("Open"), tab_h).on_hover_text(tip).clicked() {

                    app.open_dialog();

                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

                    if !controls.right.is_empty() {



                        let size = egui::vec2(window_controls::width(&controls.right) - window_controls::SIDE_GAP, window_controls::BUTTON);

                        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {

                            ui.spacing_mut().item_spacing.x = 0.0;

                            window_controls::buttons(ui, &controls.right);

                        });

                        ui.add_space(window_controls::SIDE_GAP - 4.0);

                    }

                    let (icon, label) = match app.theme_preference {

                        ThemePreference::System => ("monitor", tl!("Use system setting")),

                        ThemePreference::Light => ("sun", tl!("Light gray")),

                        ThemePreference::Dark => ("moon", tl!("Dark gray")),

                    };

                    let tip = format!("{}: {label}", tl!("Display theme"));

                    let response = icons::button(ui, icon, button, false, &tip);


                    if response.clicked() {

                        app.execute(match app.theme_preference {

                            ThemePreference::System => "view.theme.light",

                            ThemePreference::Light => "view.theme.dark",

                            ThemePreference::Dark => "view.theme.system",

                        });

                    }

                    egui::Popup::context_menu(&response).id(response.id.with("theme_menu")).show(|ui| appearance_menu(app, ui));

                    if !compact {

                        if icons::button(ui, "circle-help", button, false, tl!("Keyboard shortcuts")).clicked() {

                            app.dialog = Some(Dialog::Shortcuts);

                        }


                        if widgets::ghost_button_sized(ui, "messages-square", "Discord", tab_h).on_hover_text(pdfcraft_engine::links::DISCORD).clicked() {

                            app.execute("help.discord");

                        }

                    }

                });

            });

        });

}



fn theme_menu(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    for (preference, command, label) in [

        (ThemePreference::System, "view.theme.system", "Use system setting"),

        (ThemePreference::Light, "view.theme.light", "Light gray"),

        (ThemePreference::Dark, "view.theme.dark", "Dark gray"),

    ] {

        if ui.add(widgets::radio(app.theme_preference == preference, tl!(label))).clicked() {

            app.execute(command);

            ui.close();

        }

    }

    ui.separator();

    if ui.button(tl!("Appearance settings…")).clicked() {

        app.dialog = Some(Dialog::Preferences);

        ui.close();

    }

}



fn appearance_menu(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    if ui.button(tl!("Appearance settings…")).clicked() {

        app.dialog = Some(Dialog::Preferences);

        ui.close();

    }

}



const TAB_CHROME: f32 = 64.0;


const TAB_MIN_WIDTH: f32 = 180.0;


const TAB_ARROW: f32 = 24.0;



fn tabs_overflow(natural: &[f32], budget: f32, gap: f32, min: f32) -> bool {

    let narrowest: f32 = natural.iter().map(|w| w.min(min)).sum();

    narrowest + gap * natural.len().saturating_sub(1) as f32 > budget

}



const TAB_DRAG_EDGE: f32 = 24.0;

const TAB_DRAG_STEP: f32 = 8.0;




fn drop_index(slots: &[(usize, Rect)], from: usize, x: f32) -> usize {

    slots.iter().filter(|(i, r)| *i != from && r.center().x < x).count()

}




fn drop_marker(slots: &[(usize, Rect)], from: usize, to: usize, gap: f32) -> Option<f32> {

    if slots.iter().position(|(i, _)| *i == from) == Some(to) {

        return None;

    }

    let mut others = slots.iter().filter(|(i, _)| *i != from).map(|(_, r)| r);

    match to.checked_sub(1) {

        None => others.next().map(|r| r.left() - gap / 2.0),

        Some(before) => others.nth(before).map(|r| r.right() + gap / 2.0),

    }

}



fn arrow(ui: &mut egui::Ui, icon: &str, enabled: bool, tip: &str) -> bool {

    ui.add_enabled_ui(enabled, |ui| icons::button(ui, icon, TAB_ARROW, false, tip)).inner.clicked()

}



fn tab_name(name: &str) -> String {

    if name.chars().count() > 28 { format!("{}…", name.chars().take(27).collect::<String>()) } else { name.to_string() }

}



fn tab_natural_width(ui: &egui::Ui, t: &Tokens, name: &str) -> f32 {

    let label = crate::bidi::visual(&tab_name(name)).into_owned();

    ui.fonts_mut(|f| f.layout_no_wrap(label, theme::regular(13.0), t.text).size().x) + TAB_CHROME

}





fn tab_cap(natural: &[f32], budget: f32, gap: f32, min: f32) -> Option<f32> {

    let n = natural.len();

    if n == 0 {

        return None;

    }

    let mut left = budget - gap * (n - 1) as f32;

    if natural.iter().sum::<f32>() <= left {

        return None;

    }

    let mut sorted = natural.to_vec();

    sorted.sort_by(f32::total_cmp);

    for (k, w) in sorted.iter().enumerate() {

        let share = left / (n - k) as f32;

        if *w > share {

            return Some(share.max(min));

        }

        left -= w;

    }

    None

}


#[allow(clippy::too_many_arguments)]

fn tab(

    ui: &mut egui::Ui,

    id: egui::Id,

    t: &Tokens,

    icon: &str,

    name: &str,

    file: Option<&str>,

    dirty: bool,

    active: bool,

    close: &mut Option<usize>,

    index: usize,

    cap: Option<f32>,

    height: f32,

    draggable: bool,

) -> egui::Response {

    let font = theme::regular(13.0);

    let mut label = tab_name(name);


    if let Some(cap) = cap {

        let room = (cap - TAB_CHROME).max(0.0);

        label = ui.fonts_mut(|f| {

            crate::panels::ellipsized_prefix(&label, |s| f.layout_no_wrap(crate::bidi::visual(s).into_owned(), font.clone(), t.text).size().x <= room)

        });

    }


    let label = crate::bidi::visual(&label).into_owned();

    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), t.text).size().x);

    let width = cap.map_or(text_w + TAB_CHROME, |cap| (text_w + TAB_CHROME).min(cap));


    let height = ui.available_height().max(height);

    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

    let a11y = if dirty { format!("{name} (edited)") } else { name.to_string() };

    let sense = if draggable { Sense::click_and_drag() } else { Sense::click() };

    let resp = craft_ui::tabs::Tab::new(id, &a11y, active).sense(sense).focus_stroke(Stroke::new(1.0, t.accent)).show_at(ui, rect, |ui, resp| {

        let press = theme::Press::track(ui, resp);

        let bg = if active {

            press.fill(t.chrome, t.chrome.lerp_to_gamma(t.hover, 0.65), t.hover)

        } else {

            press.fill(Color32::TRANSPARENT, t.hover, t.pressed)

        };

        if bg.a() > 0 {

            ui.painter().rect_filled(rect, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, bg);

        }

        icons::paint(

            ui,

            Rect::from_min_size(rect.min + vec2(6.0, (height - 16.0) / 2.0), vec2(16.0, 16.0)),

            icon,

            15.0,

            if active { t.accent } else { t.text_muted },

        );

        ui.painter().text(rect.min + vec2(28.0, rect.height() / 2.0), Align2::LEFT_CENTER, label, font, if active { t.text } else { t.text_muted });

    });

    let x_rect = Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));

    let x = ui.interact(x_rect, id.with("close"), Sense::click());

    let x_press = theme::Press::track(ui, &x);

    x_press.wash(ui, x_rect, 4, false);


    if dirty && !resp.hovered() && !x.hovered() {

        ui.painter().circle_filled(x_rect.center(), 4.0, if active { t.text } else { t.text_muted });

    } else if active || resp.hovered() || x.hovered() {

        icons::paint(ui, x_rect.translate(x_press.offset()), "x", 13.0, t.text_muted);

    }

    if theme::hand(x).clicked() {

        *close = Some(index);

    }

    let shown = match file {

        Some(file) => std::borrow::Cow::Owned(format!("{}\n{}", crate::bidi::visual(name), crate::bidi::visual(file))),

        None => crate::bidi::visual(name),

    };

    theme::hand(resp).on_hover_text(if dirty {

        crate::i18n::fmt(tl!("{name} — unsaved changes"), &[("name", shown.as_ref())])

    } else {

        shown.into_owned()

    })

}


pub fn mode_bar(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());

    let compact = crate::layout_traits::is_compact(ui);

    egui::Panel::top("mode_bar")

        .exact_size(if compact { 44.0 } else { 48.0 })

        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(if compact { 6 } else { 10 }, 0)).stroke(Stroke::new(1.0, t.divider)))

        .show(ui, |ui| {

            if compact {

                compact_mode_bar(app, ui, &t);

            } else {

                desktop_mode_bar(app, ui, &t);

            }

        });

}


fn compact_mode_bar(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {

    ui.horizontal_centered(|ui| {

        ui.spacing_mut().item_spacing.x = 4.0;

        main_menu(app, ui);


        let mode_label = match app.mode {

            Mode::AllTools => tl!("Tools"),

            Mode::Read => tl!("Read"),

            Mode::Edit => tl!("Edit"),

            Mode::Convert => tl!("Convert"),

            Mode::Sign => tl!("E-Sign"),

        };

        let mode_btn = egui::Button::new(

            egui::RichText::new(format!("{mode_label} ▾"))

                .font(theme::medium(13.0))

                .color(t.accent),

        )

        .corner_radius(CornerRadius::same(6))

        .fill(t.hover);


        let mode_resp = ui.add(mode_btn);

        egui::Popup::menu(&mode_resp).show(|ui| {

            ui.set_min_width(130.0);

            for (m, lbl) in [

                (Mode::Read, "Read"),

                (Mode::Edit, "Edit"),

                (Mode::AllTools, "All tools"),

                (Mode::Convert, "Convert"),

                (Mode::Sign, "E-Sign"),

            ] {

                if widgets::menu_item(ui, tl!(lbl), "").clicked() {

                    app.switch_mode(m, true);

                    ui.close();

                }

            }

        });


        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

            ui.spacing_mut().item_spacing.x = 4.0;

            let has_doc = app.active.is_some();


            let more_resp = icons::button(ui, "ellipsis-vertical", 32.0, false, tl!("More actions"));

            egui::Popup::menu(&more_resp).show(|ui| {

                ui.set_min_width(180.0);

                if has_doc {

                    if widgets::menu_item(ui, tl!("Save"), "").clicked() {

                        app.run_command("file.save");

                        ui.close();

                    }

                    if widgets::menu_item(ui, tl!("Print"), "").clicked() {

                        app.run_command("print.dialog");

                        ui.close();

                    }

                    if widgets::menu_item(ui, tl!("Document properties"), "").clicked() {

                        app.dialog = Some(Dialog::Properties(PropsTab::Description));

                        ui.close();

                    }

                    ui.separator();

                }

                if widgets::menu_item(ui, tl!("Find tools and commands"), "").clicked() {

                    app.palette_open = true;

                    ui.close();

                }

                if widgets::menu_item(ui, tl!("Preferences"), "").clicked() {

                    app.dialog = Some(Dialog::Preferences);

                    ui.close();

                }

            });


            if has_doc {

                let tip = crate::commands::command_tip(ui.ctx(), tl!("Save ({key})"), "file.save");

                if icons::button(ui, "save", 32.0, false, &tip).clicked() {

                    app.run_command("file.save");

                }

            }


            if icons::button(ui, "search", 32.0, false, tl!("Find tools and commands")).clicked() {

                app.palette_open = true;

            }

        });

    });

}


fn desktop_mode_bar(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {

    ui.horizontal_centered(|ui| {

        ui.spacing_mut().item_spacing.x = 2.0;

        main_menu(app, ui);

        ui.add_space(6.0);

        ui.painter().vline(ui.cursor().left(), ui.max_rect().y_range().shrink(12.0), Stroke::new(1.0, t.divider));

        ui.add_space(10.0);

        for (mode, label) in

            [(Mode::AllTools, "All tools"), (Mode::Read, "Read"), (Mode::Edit, "Edit"), (Mode::Convert, "Convert"), (Mode::Sign, "E-Sign")]

        {

            if widgets::mode_tab(ui, tl!(label), app.mode == mode).clicked() {

                app.switch_mode(mode, true);

            }

        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

            ui.spacing_mut().item_spacing.x = 4.0;

            let has_doc = app.active.is_some();

            ui.add_enabled_ui(has_doc, |ui| {

                let tip = crate::commands::command_tip(ui.ctx(), tl!("Print ({key})"), "print.dialog");

                if icons::button(ui, "printer", 32.0, false, &tip).clicked() {

                    app.run_command("print.dialog");

                }

                let tip = crate::commands::command_tip(ui.ctx(), tl!("Save ({key})"), "file.save");

                if icons::button(ui, "save", 32.0, false, &tip).clicked() {

                    app.run_command("file.save");

                }

                let tip = crate::commands::command_tip(ui.ctx(), tl!("Document properties ({key})"), "file.properties");

                if icons::button(ui, "info", 32.0, false, &tip).clicked() {

                    app.dialog = Some(Dialog::Properties(PropsTab::Description));

                }

            });

            ui.add_space(8.0);

            if widgets::search_box(ui, tl!("Find tools and commands"), 260.0).clicked() {

                app.palette_open = true;

            }

        });

    });

}


fn main_menu(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());

    let resp = widgets::ghost_button(ui, "panel-left", tl!("Menu"));

    egui::Popup::menu(&resp).show(|ui| {

        ui.set_min_width(230.0);

        ui.menu_button(tl!("File"), |ui| crate::commands::registry_menu(app, ui, "File"));

        ui.menu_button(tl!("Edit"), |ui| crate::commands::registry_menu(app, ui, "Edit"));

        ui.menu_button(tl!("Pages"), |ui| crate::commands::registry_menu(app, ui, "Pages"));

        ui.menu_button(tl!("View"), |ui| {

            if let Some(i) = app.active {

                let v = &mut app.views[i];

                let ctx = ui.ctx().clone();

                let key = |s| crate::commands::shortcut_label(&ctx, s);

                use crate::commands::{ACTUAL_SIZE, FIT_WIDTH, NEXT_VIEW, PAGE_LEVEL, PREV_VIEW, ROTATE_CCW, ROTATE_CW, ZOOM_IN, ZOOM_OUT};

                ui.label(egui::RichText::new(tl!("Zoom")).color(t.text_faint).small());

                if widgets::menu_item(ui, tl!("Actual size"), &key(ACTUAL_SIZE)).clicked() {

                    v.set_zoom(1.0);

                }

                if widgets::menu_item(ui, tl!("Zoom to page level"), &key(PAGE_LEVEL)).clicked() {

                    v.fit = Fit::Page;

                }

                if widgets::menu_item(ui, tl!("Fit to width"), &key(FIT_WIDTH)).clicked() {

                    v.fit = Fit::Width;

                }

                if widgets::menu_item(ui, tl!("Fit to height"), "").clicked() {

                    v.fit = Fit::Height;

                    v.goto = Some((v.current, 0.0));

                }

                if widgets::menu_item(ui, tl!("Fit visible"), &crate::commands::command_shortcut_label(&ctx, "view.fit_visible")).clicked() {

                    ui.close();

                    app.execute("view.fit_visible");

                    return;

                }

                if widgets::menu_item(ui, tl!("Zoom in"), &key(ZOOM_IN)).clicked() {

                    v.zoom_step(true);

                }

                if widgets::menu_item(ui, tl!("Zoom out"), &key(ZOOM_OUT)).clicked() {

                    v.zoom_step(false);

                }

                if widgets::menu_item(ui, tl!("Rotate view clockwise"), &key(ROTATE_CW)).clicked() {

                    v.rotate_view(true);

                }

                if widgets::menu_item(ui, tl!("Rotate view counterclockwise"), &key(ROTATE_CCW)).clicked() {

                    v.rotate_view(false);

                }

                ui.separator();

                ui.label(egui::RichText::new(tl!("Page navigation")).color(t.text_faint).small());

                if ui.add_enabled(!v.back.is_empty(), egui::Button::new(tl!("Previous view")).shortcut_text(key(PREV_VIEW))).clicked() {

                    v.view_history(false);

                }

                if ui.add_enabled(!v.forward.is_empty(), egui::Button::new(tl!("Next view")).shortcut_text(key(NEXT_VIEW))).clicked() {

                    v.view_history(true);

                }

                ui.separator();

                if let Some(id) = page_display_menu(ui, &app.views[i]) {

                    app.execute(id);

                }

                ui.separator();

            }

            crate::commands::registry_menu(app, ui, "View");

            ui.menu_button(tl!("Display theme"), |ui| theme_menu(app, ui));

            ui.menu_button(tl!("Side panels"), |ui| {

                for (p, label) in [

                    (RightPanel::Comments, tl!("Comments")),

                    (RightPanel::Bookmarks, tl!("Bookmarks")),

                    (RightPanel::Pages, tl!("Pages")),

                    (RightPanel::Fields, tl!("Fields")),

                    (RightPanel::Layers, tl!("Layers")),

                    (RightPanel::Attachments, tl!("Attachments")),

                    (RightPanel::Signatures, tl!("Signatures")),

                    (RightPanel::Accessibility, tl!("Accessibility Checker")),

                    (RightPanel::Tags, tl!("Tags")),

                    (RightPanel::Search, tl!("Search")),

                    (RightPanel::Compare, tl!("Compare")),

                ] {

                    if ui.add(widgets::radio(app.right == Some(p), tl!(label))).clicked() {

                        app.choose_right_panel(Some(p));

                    }

                }

            });

        });

        ui.menu_button(tl!("Help"), |ui| crate::commands::registry_menu(app, ui, "Help"));

    });

}


pub fn right_rail(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());

    let Some((index, id)) = app.active_ids() else { return };

    let Some(doc) = app.session.get(id) else { return };

    let has_signatures = doc.is_signed();

    let has_check = app.a11y.report.as_ref().is_some_and(|(d, _)| *d == id);

    let (has_comments, has_outline, has_fields, has_layers, has_files) = (

        !doc.info.annotations.is_empty(),

        !doc.info.outline.is_empty(),

        !doc.info.fields.is_empty(),

        !doc.info.layers.is_empty(),

        !doc.info.attachments.is_empty(),

    );

    let page_count = doc.info.pages.len();

    let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();

    egui::Panel::right("rail")

        .resizable(false)

        .exact_size(48.0)

        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(6, 8)).stroke(Stroke::new(1.0, t.divider)))

        .show(ui, |ui| {

            ui.spacing_mut().item_spacing.y = 4.0;

            let mut rail_button = |ui: &mut egui::Ui, panel: RightPanel, icon: &str, tip: &str, has: bool| {

                let selected = app.right == Some(panel) && (app.mode == Mode::Read || app.docking.exposed(crate::docking::Panel::Inspector));

                let r = icons::button(ui, icon, 34.0, selected, tl!(tip));

                if has && !selected {

                    let c = r.rect.right_top() + vec2(-8.0, 8.0);

                    ui.painter().circle_filled(c, 3.0, t.accent);

                }

                if r.clicked() {

                    app.choose_right_panel(if selected { None } else { Some(panel) });

                }

            };

            rail_button(ui, RightPanel::Comments, "message-square-text", "Comments", has_comments);

            rail_button(ui, RightPanel::Bookmarks, "bookmark", "Bookmarks", has_outline);

            rail_button(ui, RightPanel::Pages, "files", "Page thumbnails", false);

            rail_button(ui, RightPanel::Fields, "text-cursor-input", "Form fields", has_fields);

            rail_button(ui, RightPanel::Layers, "layers", "Layers", has_layers);

            rail_button(ui, RightPanel::Attachments, "paperclip", "Attachments", has_files);

            rail_button(ui, RightPanel::Signatures, "signature", "Signatures", has_signatures);

            if has_check {

                rail_button(ui, RightPanel::Accessibility, "accessibility", "Accessibility Checker", false);

            }

            rail_button(ui, RightPanel::Tags, "tag", "Tags", false);



            let view = &mut app.views[index];

            let mut page_display = None;

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {

                ui.spacing_mut().item_spacing.y = 2.0;

                let tip = crate::commands::key_tip(ui.ctx(), tl!("Zoom out ({key})"), crate::commands::ZOOM_OUT);

                if icons::button(ui, "zoom-out", 32.0, false, &tip).clicked() {

                    view.zoom_step(false);

                }

                let tip = crate::commands::key_tip(ui.ctx(), tl!("Zoom in ({key})"), crate::commands::ZOOM_IN);

                if icons::button(ui, "zoom-in", 32.0, false, &tip).clicked() {

                    view.zoom_step(true);

                }


                let tip = crate::i18n::fmt(tl!("Page display: {layout}"), &[("layout", tl!(view.layout.label()))]);

                let resp = icons::button(ui, view.layout.icon(), 32.0, false, &tip);

                page_display = egui::Popup::menu(&resp)

                    .show(|ui| {

                        ui.set_min_width(220.0);

                        rail_view_menu(ui, view)

                    })

                    .and_then(|r| r.inner);

                let tip = crate::commands::key_tip(ui.ctx(), tl!("Rotate view clockwise ({key})"), crate::commands::ROTATE_CW);

                if icons::button(ui, "rotate-cw", 32.0, false, &tip).clicked() {

                    view.rotate_view(true);

                }


                let fit_icon = if view.fit == Fit::Width { "maximize" } else { "maximize-2" };

                if icons::button(ui, fit_icon, 32.0, false, tl!("Toggle fit page / fit width")).clicked() {

                    view.fit = if view.fit == Fit::Width { Fit::Page } else { Fit::Width };

                    view.goto = Some((view.current, 0.0));

                }

                let zoom = format!("{:.0}%", view.zoom * 100.0);

                ui.allocate_ui_with_layout(vec2(40.0, 16.0), Layout::top_down(Align::Center), |ui| {

                    ui.label(egui::RichText::new(zoom).font(theme::medium(11.0)).color(t.text_muted));

                });

                ui.add_space(6.0);

                if icons::button(ui, "chevron-down", 30.0, false, tl!("Next page")).clicked() {

                    view.step_page(true);

                }

                if icons::button(ui, "chevron-up", 30.0, false, tl!("Previous page")).clicked() {

                    view.step_page(false);

                }


                for (forward, icon, label, shortcut, enabled) in [

                    (true, "chevron-right", tl!("Next view"), crate::commands::NEXT_VIEW, !view.forward.is_empty()),

                    (false, "chevron-left", tl!("Previous view"), crate::commands::PREV_VIEW, !view.back.is_empty()),

                ] {

                    let tip = format!("{label} ({})", crate::commands::shortcut_label(ui.ctx(), shortcut));

                    if ui.add_enabled_ui(enabled, |ui| icons::button(ui, icon, 30.0, false, &tip)).inner.clicked() {

                        view.view_history(forward);

                    }

                }

                ui.label(egui::RichText::new(page_count.to_string()).font(theme::regular(11.0)).color(t.text_muted));

                let page_input_id = view.page_input_id();


                let reselect = std::mem::take(&mut view.page_input_reselect);


                if !ui.memory(|m| m.has_focus(page_input_id)) {

                    view.page_input = view.current_page_label(labels.get(view.current).map(String::as_str));

                } else if reselect {


                    let mut state = egui::TextEdit::load_state(ui.ctx(), page_input_id).unwrap_or_default();

                    let end = egui::text::CCursor::new(view.page_input.chars().count());

                    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)));

                    state.store(ui.ctx(), page_input_id);

                }

                let edit = egui::TextEdit::singleline(&mut view.page_input)

                    .id(page_input_id)

                    .desired_width(34.0)

                    .horizontal_align(Align::Center)

                    .font(theme::medium(12.0))

                    .margin(vec2(2.0, 4.0));

                let r = egui::Frame::NONE

                    .fill(t.field)

                    .stroke(Stroke::new(1.0, t.border))

                    .corner_radius(CornerRadius::same(5))

                    .show(ui, |ui| ui.add(edit))

                    .inner

                    .on_hover_text(tl!("Current page — type a page number or label (such as iv) and press Enter"));

                if r.lost_focus() {

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {


                        let typed = view.page_input.clone();

                        view.go_to_typed(&typed, &labels);

                    }

                    view.page_input = view.current_page_label(labels.get(view.current).map(String::as_str));

                }

            });

            if let Some(id) = page_display {

                app.execute(id);

            }

        });

}




fn page_display_menu(ui: &mut egui::Ui, view: &DocView) -> Option<&'static str> {

    let t = Tokens::get(ui.ctx());

    ui.label(egui::RichText::new(tl!("Page display")).color(t.text_faint).small());

    let mut picked = None;

    for l in PageLayout::ORDER {

        if ui.add(widgets::radio(view.layout == l, tl!(l.label()))).clicked() {

            picked = Some(l.command());

        }

    }

    ui.separator();


    let mut continuous = view.continuous();

    if ui.add(widgets::checkbox(&mut continuous, tl!("Continuous scrolling"))).changed() {

        picked = Some("view.continuous");

    }

    let mut cover = view.cover;

    if ui.add_enabled(view.cover_applies(), widgets::checkbox(&mut cover, tl!("Show cover page in two-page view"))).changed() {

        picked = Some("view.layout.cover");

    }

    if picked.is_some() {

        ui.close();

    }

    picked

}




fn rail_view_menu(ui: &mut egui::Ui, view: &mut DocView) -> Option<&'static str> {

    let mut picked = None;

    for (id, label) in [("view.fit_width_scrolling", "Fit to width scrolling"), ("view.fit_one_page", "Fit one full page")] {

        if ui.button(tl!(label)).clicked() {

            picked = Some(id);

        }

    }

    ui.separator();

    if ui.button(tl!("Actual size")).clicked() {

        view.set_zoom(1.0);

        ui.close();

    }

    if ui.button(tl!("Zoom to page level")).clicked() {

        view.set_fit(Fit::Page);

        ui.close();

    }

    if ui.button(tl!("Fit visible")).clicked() {

        picked = Some("view.fit_visible");

    }

    ui.separator();

    picked = page_display_menu(ui, view).or(picked);

    ui.separator();

    for (id, label) in [("view.read_mode", "Read mode"), ("view.full_screen", "Full screen mode")] {

        if ui.button(tl!(label)).clicked() {

            picked = Some(id);

        }

    }

    if picked.is_some() {

        ui.close();

    }

    picked

}


pub fn mobile_top_header(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());

    let insets = crate::layout_traits::safe_area_insets(ui.ctx());

    let header_h = crate::layout_traits::MOBILE_HEADER_HEIGHT + insets.top;


    egui::Panel::top("mobile_top_header")

        .exact_size(header_h)

        .frame(

            egui::Frame::NONE

                .fill(t.chrome)

                .stroke(Stroke::new(1.0, t.divider))

                .inner_margin(egui::Margin {

                    left: 12,

                    right: 12,

                    top: insets.top as i8,

                    bottom: 0,

                }),

        )

        .show(ui, |ui| {

            ui.horizontal_centered(|ui| {

                if app.active.is_some() || app.combine_showing() {

                    if icons::button(ui, "chevron-left", 36.0, false, tl!("Back to Home")).clicked() {

                        app.active = None;

                        app.combine_tab.focused = false;

                    }

                } else if icons::button(ui, "file-plus", 36.0, false, tl!("Open PDF")).clicked() {

                    app.open_dialog();

                }


                let title = if let Some(i) = app.active {

                    crate::canvas::tab_label(app, i).unwrap_or_else(|| "PDF".to_string())

                } else if app.combine_showing() {

                    tl!("Combine files").to_string()

                } else {

                    "PdfCraft".to_string()

                };


                let avail = ui.available_width() - 80.0;

                let title_job = egui::text::LayoutJob::simple_singleline(

                    title.clone(),

                    theme::semibold(15.0),

                    t.text,

                );

                ui.add_sized([avail.max(40.0), 36.0], egui::Label::new(title_job).truncate())

                    .on_hover_text(&title);


                let open_count = app.views.len() + if app.combine_tab.open { 1 } else { 0 };

                if open_count > 0 {

                    let badge_text = format!("{open_count}");

                    let btn_resp = ui.add(

                        egui::Button::new(

                            egui::RichText::new(&badge_text)

                                .font(theme::semibold(12.0))

                                .color(if app.doc_switcher_open { t.accent_text } else { t.text }),

                        )

                        .min_size(vec2(28.0, 28.0))

                        .corner_radius(CornerRadius::same(14))

                        .fill(if app.doc_switcher_open { t.accent } else { t.hover }),

                    ).on_hover_text(tl!("Open documents switcher"));

                    if btn_resp.clicked() {

                        app.doc_switcher_open = !app.doc_switcher_open;

                    }

                }


                if icons::button(ui, "ellipsis-vertical", 32.0, false, tl!("More options")).clicked() {
                    app.more_sheet_open = !app.more_sheet_open;
                }
            });
        });

}


pub fn mobile_bottom_bar(app: &mut PdfCraftApp, ui: &mut egui::Ui) {

    let t = Tokens::get(ui.ctx());

    let insets = crate::layout_traits::safe_area_insets(ui.ctx());

    let bar_h = crate::layout_traits::MOBILE_BOTTOM_BAR_HEIGHT + insets.bottom;


    egui::Panel::bottom("mobile_bottom_bar")

        .exact_size(bar_h)

        .frame(

            egui::Frame::NONE

                .fill(t.chrome)

                .stroke(Stroke::new(1.0, t.divider))

                .shadow(egui::Shadow { offset: [0, -3], blur: 12, spread: 0, color: Color32::from_black_alpha(40) })

                .inner_margin(egui::Margin {

                    left: 8,

                    right: 8,

                    top: 4,

                    bottom: insets.bottom as i8 + 4,

                }),

        )

        .show(ui, |ui| {
            let total_w = ui.available_width();
            let has_doc = app.active.is_some();
            if !has_doc {
                let item_w = (total_w / 4.0).max(48.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if nav_tab(ui, &t, "home", tl!("Home"), true, item_w).clicked() {
                        app.active = None;
                    }
                    if nav_tab(ui, &t, "clock", tl!("Recent"), false, item_w).clicked() {
                        app.active = None;
                    }
                    if nav_tab(ui, &t, "folder", tl!("Files"), false, item_w).clicked() {
                        app.open_dialog();
                    }
                    if nav_tab(ui, &t, "settings", tl!("Settings"), app.dialog == Some(crate::Dialog::Preferences), item_w).clicked() {
                        app.dialog = Some(crate::Dialog::Preferences);
                    }
                });
            } else {
                let item_w = (total_w / 5.0).max(48.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    let is_read = app.mode == Mode::Read && app.right != Some(RightPanel::Pages) && app.right != Some(RightPanel::Search) && !app.more_sheet_open;
                    if nav_tab(ui, &t, "book-open", tl!("Read"), is_read, item_w).clicked() {
                        app.select_mode(Mode::Read);
                        app.choose_right_panel(None);
                        app.more_sheet_open = false;
                    }

                    let is_tools = (app.mode == Mode::Edit || app.mode == Mode::Sign || app.mode == Mode::Convert || app.mode == Mode::AllTools) && !app.more_sheet_open;
                    if nav_tab(ui, &t, "wrench", tl!("Tools"), is_tools, item_w).clicked() {
                        if app.mode == Mode::Read {
                            app.select_mode(Mode::Edit);
                        } else if app.mode == Mode::Edit {
                            app.select_mode(Mode::Sign);
                        } else {
                            app.select_mode(Mode::Edit);
                        }
                        app.choose_right_panel(None);
                        app.more_sheet_open = false;
                    }

                    let is_pages = app.right == Some(RightPanel::Pages) && !app.more_sheet_open;
                    if nav_tab(ui, &t, "layers", tl!("Pages"), is_pages, item_w).clicked() {
                        app.more_sheet_open = false;
                        if is_pages {
                            app.choose_right_panel(None);
                        } else {
                            app.choose_right_panel(Some(RightPanel::Pages));
                        }
                    }

                    let is_search = app.right == Some(RightPanel::Search) && !app.more_sheet_open;
                    if nav_tab(ui, &t, "search", tl!("Search"), is_search, item_w).clicked() {
                        app.more_sheet_open = false;
                        if is_search {
                            app.choose_right_panel(None);
                        } else {
                            app.choose_right_panel(Some(RightPanel::Search));
                        }
                    }

                    let is_more = app.more_sheet_open;
                    if nav_tab(ui, &t, "menu", tl!("More"), is_more, item_w).clicked() {
                        app.more_sheet_open = !app.more_sheet_open;
                        app.choose_right_panel(None);
                    }
                });
            }
        });

}


fn nav_tab(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str, active: bool, width: f32) -> egui::Response {

    let (rect, resp) = ui.allocate_exact_size(vec2(width, 48.0), Sense::click());

    let color = if active { t.accent } else { t.text_muted };

    if active {

        ui.painter().rect_filled(rect.shrink2(vec2(4.0, 4.0)), CornerRadius::same(8), t.hover);

    }

    let icon_rect = Rect::from_center_size(rect.center() - vec2(0.0, 7.0), vec2(20.0, 20.0));

    icons::paint(ui, icon_rect, icon, 20.0, color);

    ui.painter().text(

        rect.center() + vec2(0.0, 11.0),

        Align2::CENTER_CENTER,

        label,

        theme::medium(10.0),

        color,

    );

    resp

}


pub fn show_doc_switcher(app: &mut PdfCraftApp, ctx: &egui::Context) {

    if !app.doc_switcher_open {

        return;

    }

    let screen = ctx.content_rect();

    let t = Tokens::get(ctx);

    let mut close = false;

    let mut switch_to: Option<usize> = None;

    let mut close_tab: Option<usize> = None;

    let mut open_new = false;


    let insets = crate::layout_traits::safe_area_insets(ctx);

    let sheet_h = ((screen.height() * 0.65).clamp(260.0, 520.0) + insets.bottom).min(screen.height() - insets.top);

    let sheet_rect = Rect::from_min_size(

        egui::pos2(screen.left(), screen.bottom() - sheet_h),

        vec2(screen.width(), sheet_h),

    );

    let above_rect = Rect::from_min_max(screen.min, egui::pos2(screen.right(), sheet_rect.top()));

    egui::Area::new(egui::Id::new("doc_switcher_scrim"))

        .order(egui::Order::Foreground)

        .fixed_pos(screen.min)

        .show(ctx, |ui| {

            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(130));

            let resp = ui.allocate_rect(above_rect, Sense::click());

            if resp.clicked() {

                close = true;

            }

        });


    egui::Area::new(egui::Id::new("doc_switcher_sheet"))

        .order(egui::Order::Foreground)

        .fixed_pos(sheet_rect.min)

        .show(ctx, |ui| {

            egui::Frame::NONE

                .fill(t.chrome)

                .corner_radius(CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 })

                .stroke(Stroke::new(1.0, t.divider))

                .shadow(egui::Shadow { offset: [0, -4], blur: 20, spread: 0, color: Color32::from_black_alpha(100) })

                .inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: (insets.bottom as i8 + 16).max(16) })

                .show(ui, |ui| {

                    ui.set_width(sheet_rect.width() - 32.0);

                    ui.set_height(sheet_rect.height() - 28.0 - insets.bottom);


                    ui.vertical_centered(|ui| {

                        let handle_rect = Rect::from_center_size(ui.cursor().center() + vec2(0.0, 4.0), vec2(40.0, 4.0));

                        ui.painter().rect_filled(handle_rect, CornerRadius::same(2), t.divider);

                        ui.add_space(10.0);

                    });


                    ui.horizontal(|ui| {

                        ui.label(egui::RichText::new(tl!("Open Documents")).font(theme::semibold(16.0)).color(t.text));

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

                            if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {

                                close = true;

                            }

                        });

                    });

                    ui.separator();

                    ui.add_space(4.0);


                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {

                        for i in 0..app.views.len() {

                            let is_active = app.active == Some(i);

                            let title = crate::canvas::tab_label(app, i).unwrap_or_else(|| format!("Doc {}", i + 1));

                            let (row_rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());

                            if is_active {

                                ui.painter().rect_filled(row_rect, CornerRadius::same(8), t.hover);

                                ui.painter().rect_stroke(row_rect, CornerRadius::same(8), Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);

                            } else if resp.hovered() {

                                ui.painter().rect_filled(row_rect, CornerRadius::same(8), t.hover);

                            }

                            let icon_rect = Rect::from_center_size(row_rect.left_center() + vec2(20.0, 0.0), vec2(22.0, 22.0));

                            icons::paint(ui, icon_rect, "file-text", 22.0, if is_active { t.accent } else { t.text_muted });


                            let label_rect = Rect::from_min_max(

                                egui::pos2(row_rect.left() + 40.0, row_rect.top() + 8.0),

                                egui::pos2(row_rect.right() - 44.0, row_rect.bottom() - 8.0),

                            );

                            let mut job = egui::text::LayoutJob::simple_singleline(

                                title.clone(),

                                theme::semibold(14.0),

                                if is_active { t.text } else { t.text_muted },

                            );

                            job.wrap = egui::text::TextWrapping { max_width: label_rect.width(), max_rows: 1, break_anywhere: false, ..Default::default() };

                            let galley = ui.fonts_mut(|f| f.layout_job(job));

                            ui.painter().galley(label_rect.left_center() - vec2(0.0, galley.size().y / 2.0), galley, t.text);


                            let close_rect = Rect::from_center_size(row_rect.right_center() - vec2(20.0, 0.0), vec2(28.0, 28.0));

                            let close_id = ui.id().with(("doc_close", i));

                            let close_resp = ui.interact(close_rect, close_id, Sense::click());

                            let close_color = if close_resp.hovered() { t.text } else { t.text_faint };

                            icons::paint(ui, close_rect.shrink(4.0), "x", 16.0, close_color);

                            if close_resp.clicked() {

                                close_tab = Some(i);

                            } else if resp.clicked() {

                                switch_to = Some(i);

                            }

                            ui.add_space(4.0);

                        }


                        if app.combine_tab.open {

                            let is_active = app.combine_showing();

                            let (row_rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());

                            if is_active {

                                ui.painter().rect_filled(row_rect, CornerRadius::same(8), t.hover);

                                ui.painter().rect_stroke(row_rect, CornerRadius::same(8), Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);

                            }

                            let icon_rect = Rect::from_center_size(row_rect.left_center() + vec2(20.0, 0.0), vec2(22.0, 22.0));

                            icons::paint(ui, icon_rect, "layers", 22.0, t.accent);

                            ui.painter().text(row_rect.left_center() + vec2(40.0, 0.0), Align2::LEFT_CENTER, tl!("Combine files"), theme::semibold(14.0), t.text);

                            if resp.clicked() {

                                app.active = None;

                                app.combine_tab.focused = true;

                                close = true;

                            }

                        }


                        ui.add_space(8.0);

                        if widgets::ghost_button(ui, "plus", tl!("Open another file…")).clicked() {

                            open_new = true;

                        }

                    });

                });

        });


    if let Some(i) = switch_to {

        app.active = Some(i);

        app.combine_tab.focused = false;

        close = true;

    }

    if let Some(i) = close_tab {

        app.close_tab(i);

        if app.views.is_empty() && !app.combine_tab.open {

            close = true;

        }

    }

    if open_new {

        app.open_dialog();

        close = true;

    }

    if close {

        app.doc_switcher_open = false;

    }

}

pub fn show_more_sheet(app: &mut PdfCraftApp, ctx: &egui::Context) {
    if !app.more_sheet_open {
        return;
    }
    let screen = ctx.content_rect();
    let insets = crate::layout_traits::safe_area_insets(ctx);
    let t = Tokens::get(ctx);
    let mut close = false;
    let has_doc = app.active.is_some();
    let sheet_h = if has_doc {
        (390.0f32 + insets.bottom).min(screen.height() * 0.75)
    } else {
        (240.0f32 + insets.bottom).min(screen.height() * 0.5)
    };
    let sheet_rect = Rect::from_min_size(
        egui::pos2(screen.left(), screen.bottom() - sheet_h),
        vec2(screen.width(), sheet_h),
    );
    let above_rect = Rect::from_min_max(screen.min, egui::pos2(screen.right(), sheet_rect.top()));

    egui::Area::new(egui::Id::new("more_sheet_scrim"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(130));
            let resp = ui.allocate_rect(above_rect, Sense::click());
            if resp.clicked() {
                close = true;
            }
        });

    egui::Area::new(egui::Id::new("more_sheet_panel"))
        .order(egui::Order::Foreground)
        .fixed_pos(sheet_rect.min)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(t.chrome)
                .corner_radius(CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 })
                .stroke(Stroke::new(1.0, t.divider))
                .shadow(egui::Shadow { offset: [0, -4], blur: 20, spread: 0, color: Color32::from_black_alpha(100) })
                .inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: (insets.bottom as i8 + 16).max(16) })
                .show(ui, |ui| {
                    ui.set_width(sheet_rect.width() - 32.0);
                    ui.set_height(sheet_rect.height() - 28.0 - insets.bottom);

                    ui.vertical_centered(|ui| {
                        let handle_rect = Rect::from_center_size(ui.cursor().center() + vec2(0.0, 4.0), vec2(40.0, 4.0));
                        ui.painter().rect_filled(handle_rect, CornerRadius::same(2), t.divider);
                        ui.add_space(10.0);
                    });

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(tl!("More Actions")).font(theme::semibold(16.0)).color(t.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icons::button(ui, "x", 28.0, false, tl!("Close")).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.separator();
                    ui.add_space(4.0);

                    let action_row = |ui: &mut egui::Ui, icon: &str, label: &str| -> bool {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                        let hover = resp.hovered();
                        if hover {
                            ui.painter().rect_filled(rect, CornerRadius::same(8), t.hover);
                        }
                        let icon_rect = Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(20.0, 20.0));
                        icons::paint(ui, icon_rect, icon, 20.0, if hover { t.accent } else { t.text_muted });
                        ui.painter().text(
                            rect.left_center() + vec2(42.0, 0.0),
                            Align2::LEFT_CENTER,
                            label,
                            theme::medium(14.0),
                            t.text,
                        );
                        resp.clicked()
                    };

                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if has_doc {
                            if action_row(ui, "bookmark", tl!("Bookmarks")) {
                                app.choose_right_panel(Some(RightPanel::Bookmarks));
                                close = true;
                            }
                            if action_row(ui, "message-square", tl!("Comments & Annotations")) {
                                app.choose_right_panel(Some(RightPanel::Comments));
                                close = true;
                            }
                            if action_row(ui, "paperclip", tl!("Attachments")) {
                                app.choose_right_panel(Some(RightPanel::Attachments));
                                close = true;
                            }
                            if action_row(ui, "info", tl!("Document Properties")) {
                                app.dialog = Some(crate::Dialog::Properties(crate::PropsTab::Description));
                                close = true;
                            }
                            if action_row(ui, "share-2", tl!("Export / Convert…")) {
                                app.run_command("file.export");
                                close = true;
                            }
                            if action_row(ui, "printer", tl!("Print…")) {
                                app.open_print();
                                close = true;
                            }
                        } else {
                            if action_row(ui, "folder", tl!("Open PDF…")) {
                                app.open_dialog();
                                close = true;
                            }
                            if action_row(ui, "settings", tl!("Preferences")) {
                                app.dialog = Some(crate::Dialog::Preferences);
                                close = true;
                            }
                            if action_row(ui, "info", tl!("About PdfCraft")) {
                                app.dialog = Some(crate::Dialog::About);
                                close = true;
                            }
                        }
                    });
                });
        });

    if close {
        app.more_sheet_open = false;
    }
}


#[cfg(test)]

mod tab_widths {

    use super::{MAC_CONTENT_LEFT, MAC_LIGHT, MAC_LIGHT_PITCH, MAC_LIGHTS_LEFT, MAC_TITLE_BAR, tab_cap};



    #[test]

    fn tabs_that_fit_do_not_shrink() {

        assert_eq!(tab_cap(&[200.0, 150.0], 400.0, 4.0, 110.0), None);

        assert_eq!(tab_cap(&[], 0.0, 4.0, 110.0), None);

    }




    #[test]

    fn wide_tabs_share_what_narrow_ones_leave() {

        let natural = [120.0, 300.0, 300.0];

        let cap = tab_cap(&natural, 500.0, 4.0, 110.0).expect("must shrink");

        let used: f32 = natural.iter().map(|w| w.min(cap)).sum::<f32>() + 2.0 * 4.0;

        assert!((used - 500.0).abs() < 0.01, "{cap} uses {used}");

        assert!((cap - 186.0).abs() < 0.01, "{cap}");

    }



    #[test]

    fn arrows_only_when_the_narrowest_tabs_overflow() {

        use super::tabs_overflow;

        assert!(!tabs_overflow(&[250.0; 3], 600.0, 4.0, 180.0), "three shrunk tabs fit");

        assert!(tabs_overflow(&[250.0; 4], 600.0, 4.0, 180.0), "four don't, even at 180");

        assert!(!tabs_overflow(&[100.0; 5], 600.0, 4.0, 180.0), "narrow tabs fit as they are");

        assert!(!tabs_overflow(&[], 0.0, 4.0, 180.0));

    }



    #[test]

    fn tabs_stop_at_their_minimum() {

        assert_eq!(tab_cap(&[250.0; 12], 600.0, 4.0, 110.0), Some(110.0));

        assert_eq!(tab_cap(&[250.0; 3], 0.0, 4.0, 110.0), Some(110.0));

    }



    fn three_slots() -> Vec<(usize, egui::Rect)> {

        (0..3).map(|i| (i, egui::Rect::from_min_size(egui::pos2(i as f32 * 104.0, 0.0), egui::vec2(100.0, 30.0)))).collect()

    }



    #[test]

    fn a_dragged_tab_drops_past_the_centres_it_crossed() {

        use super::drop_index;

        let slots = three_slots();


        assert_eq!(drop_index(&slots, 0, 10.0), 0, "still over itself");

        assert_eq!(drop_index(&slots, 0, 160.0), 1, "past the middle tab's centre");

        assert_eq!(drop_index(&slots, 0, 900.0), 2, "past the end");

        assert_eq!(drop_index(&slots, 2, -50.0), 0, "before the start");

        assert_eq!(drop_index(&slots, 2, 100.0), 1, "between the first and middle centres");

        assert_eq!(drop_index(&[], 0, 100.0), 0);

    }



    #[test]

    fn the_drop_line_marks_the_gap_and_hides_when_nothing_moves() {

        use super::drop_marker;

        let slots = three_slots();

        assert_eq!(drop_marker(&slots, 0, 0, 4.0), None);

        assert_eq!(drop_marker(&slots, 1, 1, 4.0), None);


        assert_eq!(drop_marker(&slots, 0, 2, 4.0), Some(310.0));


        assert_eq!(drop_marker(&slots, 0, 1, 4.0), Some(206.0));


        assert_eq!(drop_marker(&slots, 2, 0, 4.0), Some(-2.0));

        assert_eq!(drop_marker(&[], 0, 1, 4.0), None);

    }


    #[test]

    fn the_mac_title_bar_centres_and_clears_the_traffic_lights() {


        let margin = (MAC_TITLE_BAR - MAC_LIGHT) / 2.0;

        let lights_end = MAC_LIGHTS_LEFT + 2.0 * MAC_LIGHT_PITCH + MAC_LIGHT;

        assert_eq!(MAC_CONTENT_LEFT - lights_end, margin);


        assert!(MAC_CONTENT_LEFT <= f32::from(i8::MAX));

    }

}

