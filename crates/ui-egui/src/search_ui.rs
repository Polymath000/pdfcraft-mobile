use egui::text::LayoutJob;

use egui::{Color32, FontId, TextFormat};


use crate::canvas::DocView;

use crate::icons;

use crate::theme::{self, Tokens};

use crate::widgets;


const CONTEXT: usize = 36;


pub(crate) fn panel(ui: &mut egui::Ui, t: &Tokens, view: &mut DocView, pages: usize) {

    crate::widgets::begin_dialog(ui);

    let searched = view.texts.len() + view.text_failed.len();

    let rerun_needed = {

        let Some(find) = view.find.as_mut() else { return };

        find.in_panel = true;

        let l = ui.label(egui::RichText::new(tl!("What word or phrase would you like to search for?")).color(t.text_muted));

        let mut clear_clicked = false;

        ui.horizontal(|ui| {

            let input_w = if find.query.is_empty() { ui.available_width() } else { ui.available_width() - 36.0 };

            let r = ui

                .add_sized([input_w, 32.0], crate::widgets::line(&mut find.query).id(egui::Id::new("search-panel-input")).hint_text(tl!("Search")))

                .labelled_by(l.id);

            if find.focus {

                r.request_focus();

                r.scroll_to_me(Some(egui::Align::Center));

                find.focus = false;

            }

            if !find.query.is_empty() && icons::button(ui, "x", 28.0, false, tl!("Clear search")).clicked() {

                clear_clicked = true;

            }

        });

        if clear_clicked {

            find.query.clear();

            find.case_query.clear();

        }

        ui.horizontal(|ui| {

            let w = &mut ui.visuals_mut().widgets;

            w.inactive.bg_stroke = egui::Stroke::new(1.0, t.border);

            w.inactive.bg_fill = t.hover;

            let a = ui.add(widgets::checkbox(&mut find.whole_words, tl!("Whole words only"))).changed();

            let b = ui.add(widgets::checkbox(&mut find.case_sensitive, tl!("Case-sensitive"))).changed();

            if a || b {

                find.case_query.clear();

            }

        });

        clear_clicked || find.query != find.case_query

    };

    if rerun_needed {

        view.rerun_find();

    }

    let Some(find) = view.find.as_ref() else { return };

    ui.add_space(6.0);

    let status = if find.query.trim().is_empty() {

        String::new()

    } else if find.matches.is_empty() && searched < pages {

        crate::i18n::fmt(tl!("Searching… {s} of {p} pages"), &[("s", &searched.to_string()), ("p", &pages.to_string())])

    } else {

        let n = find.matches.len();

        let more = if searched < pages {

            crate::i18n::fmt(tl!(" (searching… {s} of {p} pages)"), &[("s", &searched.to_string()), ("p", &pages.to_string())])

        } else {

            String::new()

        };


        let count = if n == 1 { crate::i18n::fmt(tl!("1 instance"), &[]) } else { crate::i18n::fmt(tl!("{n} instances"), &[("n", &n.to_string())]) };

        format!("{count}{more}")

    };

    ui.label(egui::RichText::new(status).font(theme::semibold(12.5)));

    ui.add_space(4.0);

    let mut go = None;

    let mut last_page = None;

    for (i, (p, range)) in find.matches.iter().enumerate().take(2000) {

        if last_page != Some(*p) {

            ui.add_space(4.0);

            ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Page {label}"), &[("label", &(p + 1).to_string())])).small().color(t.text_faint));

            last_page = Some(*p);

        }

        let Some(text) = view.texts.get(p) else { continue };

        let n = text.glyphs.len();

        let before = text.text_of(range.start.saturating_sub(CONTEXT)..range.start.min(n));

        let hit = text.text_of(range.start.min(n)..range.end.min(n));

        let after = text.text_of(range.end.min(n)..(range.end + CONTEXT).min(n));

        let flat = |s: String| s.split_whitespace().collect::<Vec<_>>().join(" ");

        let mut job = LayoutJob::default();

        let font = FontId::new(12.5, egui::FontFamily::Proportional);

        let plain = TextFormat { font_id: font.clone(), color: t.text_muted, ..Default::default() };

        job.append(&format!("…{} ", flat(before)), 0.0, plain.clone());

        job.append(

            &flat(hit),

            0.0,

            TextFormat {

                font_id: theme::semibold(12.5),

                color: t.text,

                background: Color32::from_rgba_unmultiplied(255, 214, 0, 90),

                ..Default::default()

            },

        );

        job.append(&format!(" {}…", flat(after)), 0.0, plain);

        job.wrap.max_width = ui.available_width();

        let selected = find.current == Some(i);

        let resp = ui.add(egui::Button::selectable(selected, job).wrap()).on_hover_text(tl!("Go to this result"));

        if resp.clicked() {

            go = Some(i);

        }

    }

    if let Some(i) = go {

        view.go_to_match(i);

    }

}

