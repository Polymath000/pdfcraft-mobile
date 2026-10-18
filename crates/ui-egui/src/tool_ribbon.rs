use egui::{vec2, Color32, Margin, Rect, ScrollArea, Shadow, Stroke, Ui};
use crate::theme::Tokens;
use crate::{icons, widgets, Mode, PdfCraftApp, QuickTool};

pub fn render(app: &mut PdfCraftApp, ui: &mut Ui, t: &Tokens) {
    if app.mode == Mode::Read {
        return;
    }
    let rect = ui.max_rect();
    let insets = crate::layout_traits::safe_area_insets(ui.ctx());
    let bar_h = crate::layout_traits::MOBILE_BOTTOM_BAR_HEIGHT + insets.bottom;
    let ribbon_h = 56.0;
    let ribbon_rect = Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - bar_h - ribbon_h),
        vec2(rect.width(), ribbon_h),
    );

    egui::Area::new(egui::Id::new("mobile_tool_ribbon"))
        .order(egui::Order::Foreground)
        .fixed_pos(ribbon_rect.min)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(t.chrome)
                .stroke(Stroke::new(1.0, t.divider))
                .shadow(Shadow { offset: [0, -2], blur: 8, spread: 0, color: Color32::from_black_alpha(60) })
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ribbon_rect.width() - 16.0);
                    ui.set_height(ribbon_h - 12.0);
                    ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            match app.mode {
                                Mode::AllTools | Mode::Edit => {
                                    if widgets::pill_button(ui, tl!("Add Text"), app.quick_tool == QuickTool::AddText).clicked() {
                                        app.quick_tool = QuickTool::AddText;
                                    }
                                    if widgets::pill_button(ui, tl!("Edit Text"), app.quick_tool == QuickTool::EditText).clicked() {
                                        app.quick_tool = QuickTool::EditText;
                                    }
                                    if widgets::pill_button(ui, tl!("Sign"), app.mode == Mode::Sign).clicked() {
                                        app.select_mode(Mode::Sign);
                                    }
                                    if icons::button(ui, "rotate-ccw", 44.0, false, tl!("Rotate Left")).clicked() {
                                        app.rotate_active_page(false);
                                    }
                                    if icons::button(ui, "rotate-cw", 44.0, false, tl!("Rotate Right")).clicked() {
                                        app.rotate_active_page(true);
                                    }
                                    if icons::button(ui, "trash-2", 44.0, false, tl!("Delete Page")).clicked() {
                                        app.delete_active_page();
                                    }
                                }
                                Mode::Sign => {
                                    if widgets::pill_button(ui, tl!("Text"), app.quick_tool == QuickTool::Fill(crate::fill_sign::FillTool::Text)).clicked() {
                                        app.quick_tool = QuickTool::Fill(crate::fill_sign::FillTool::Text);
                                    }
                                    if widgets::pill_button(ui, tl!("Sign"), app.quick_tool == QuickTool::Fill(crate::fill_sign::FillTool::Signature)).clicked() {
                                        app.quick_tool = QuickTool::Fill(crate::fill_sign::FillTool::Signature);
                                    }
                                    if widgets::pill_button(ui, tl!("Date"), app.quick_tool == QuickTool::Fill(crate::fill_sign::FillTool::Date)).clicked() {
                                        app.quick_tool = QuickTool::Fill(crate::fill_sign::FillTool::Date);
                                    }
                                    if widgets::pill_button(ui, tl!("Check"), app.quick_tool == QuickTool::Fill(crate::fill_sign::FillTool::Check)).clicked() {
                                        app.quick_tool = QuickTool::Fill(crate::fill_sign::FillTool::Check);
                                    }
                                    if widgets::pill_button(ui, tl!("Cross"), app.quick_tool == QuickTool::Fill(crate::fill_sign::FillTool::Cross)).clicked() {
                                        app.quick_tool = QuickTool::Fill(crate::fill_sign::FillTool::Cross);
                                    }
                                }
                                Mode::Convert => {
                                    if widgets::pill_button(ui, tl!("Export Image"), false).clicked() {
                                        app.start_export(crate::export_ui::ExportKind::Image);
                                    }
                                    if widgets::pill_button(ui, tl!("Export Text"), false).clicked() {
                                        app.start_export(crate::export_ui::ExportKind::Text);
                                    }
                                }
                                _ => {}
                            }
                        });
                    });
                });
        });
}
