


use pdfcraft_engine::Edit;

use pdfcraft_platform::staging::{StagingName, staging_suffixes};


use crate::PdfCraftApp;

use crate::rfd;



#[derive(Clone, Copy, Debug, PartialEq, Eq)]

pub enum CloseRequest {



    Tab(pdfcraft_engine::DocId),


    Quit,


    All,

}




#[cfg(not(target_arch = "wasm32"))]

pub struct PendingOverwrite {

    pub id: pdfcraft_engine::DocId,

    pub dest: String,


    after: Box<dyn FnOnce(&mut PdfCraftApp) + Send>,

}



#[derive(Clone, Copy, Debug, PartialEq, Eq)]

pub enum SaveTarget {


    InPlace,


    As,

}


impl PdfCraftApp {


    pub fn apply_edit(&mut self, edit: Edit) -> bool {

        let Some((i, id)) = self.active_ids() else { return false };

        let label = edit.label();

        let (form_before, page_count_before, had_xfa) = if let Some(d) = self.session.get(id) {

            (Some(std::sync::Arc::clone(&d.form)), d.info.pages.len(), d.xfa.is_some())

        } else {

            (None, 0, false)

        };

        match self.session.apply(id, edit.clone()) {

            Ok(()) => {

                let Some(doc) = self.session.get(id) else { return true };

                let info = &doc.info;

                let view = &mut self.views[i];

                if !matches!(edit, Edit::AddAnnotation(_)) {


                    view.comments.settling.clear();

                }

                view.signature_drag.committed(&edit, doc.edit_generation());

                view.move_background.committed(&edit, doc.edit_generation());

                if let Some(page) = comment_page(&edit).or_else(|| content_page(&edit)) {


                    view.page_changed(page);

                } else if let Some(form_before) = form_before

                    && doc.info.pages.len() == page_count_before

                    && doc.xfa.is_some() == had_xfa

                    && doc.xfa.is_none()

                    && is_form_edit(&edit)

                {

                    let mut changed_pages = std::collections::BTreeSet::new();

                    for f in doc.form.iter() {

                        match form_before.iter().find(|old| old.name == f.name) {

                            Some(old) => {

                                if old != f {

                                    for w in &f.widgets {

                                        if let Some(p) = w.page {

                                            changed_pages.insert(p);

                                        }

                                    }

                                    for w in &old.widgets {

                                        if let Some(p) = w.page {

                                            changed_pages.insert(p);

                                        }

                                    }

                                    if old.value != f.value {

                                        for w in &f.widgets {

                                            if let Some(p) = w.page {

                                                view.forms.unbaked.insert((f.name.clone(), p));

                                            }

                                        }

                                    }

                                }

                            }

                            None => {

                                for w in &f.widgets {

                                    if let Some(p) = w.page {

                                        changed_pages.insert(p);

                                        view.forms.unbaked.insert((f.name.clone(), p));

                                    }

                                }

                            }

                        }

                    }

                    for old in form_before.iter() {

                        if !doc.form.iter().any(|f| f.name == old.name) {

                            for w in &old.widgets {

                                if let Some(p) = w.page {

                                    changed_pages.insert(p);

                                }

                            }

                        }

                    }

                    for page in changed_pages {

                        view.page_changed(page);

                    }

                } else {

                    view.document_changed(info);

                }


                match edit {

                    Edit::MovePages { pages, to } => {

                        let n = pages.iter().collect::<std::collections::BTreeSet<_>>().len();

                        let to = to.min(info.pages.len() - n);

                        view.select_pages(&(to..to + n).collect::<Vec<_>>());

                    }

                    Edit::InsertBlankPage { at, .. } => view.select_pages(&[at.min(info.pages.len() - 1)]),

                    Edit::DeletePages { .. } => view.select_pages(&[]),



                    Edit::AddAnnotation(a)

                        if matches!(a.shape, pdfcraft_engine::Shape::Ink { .. })

                            && self.quick_tool == crate::QuickTool::Comment(crate::comments::CommentTool::Ink) =>

                    {

                        view.comments.selected = None;

                    }

                    Edit::AddAnnotation(a) => {


                        let newest = info.annotations.iter().filter(|x| x.page == a.page && x.in_reply_to.is_none()).map(|x| x.index).max();

                        view.comments.selected = newest.map(|n| (a.page, n));

                        view.comments.reveal = true;



                        if let (pdfcraft_engine::Shape::TextMarkup { kind: pdfcraft_engine::Markup::Highlight, .. }, Some(n)) = (&a.shape, newest)

                            && a.contents.is_empty()

                            && view.comments.editing.as_ref().is_none_or(|(_, _, text)| text.is_empty())

                        {

                            view.comments.editing = Some((a.page, n, String::new()));

                        }

                    }

                    Edit::DeleteAnnotation { .. } => view.comments.selected = None,

                    _ => {}

                }

                if std::mem::take(&mut view.comments.tool_done) && !self.comment_prefs.pinned {

                    self.quick_tool = crate::QuickTool::Select;

                }



                if let Some(n) = self.session.get(id).and_then(|d| d.edit_notices.first()).cloned() {

                    self.notify(crate::content_ui::undrawn_message(&n));

                }

                let out = self.session.take_js_output(id);

                self.handle_js(id, out);

                true

            }

            Err(e) => {

                let message = if matches!(edit, Edit::MoveObjects { .. }) { crate::object_ui::move_error(&e) } else { e.to_string() };

                self.notify_fmt("{label} failed: {e}", &[("label", &crate::i18n::edit_label(&label)), ("e", &message)]);

                false

            }

        }

    }



    pub fn undo(&mut self) {



        if let Some((i, _)) = self.active_ids()

            && self.views[i].comments.undo_point()

        {

            return;

        }

        if self.combine_showing() {

            self.combine_history_step(true);

        } else {

            self.history_step(true);

        }

    }


    pub fn redo(&mut self) {

        if self.combine_showing() {

            self.combine_history_step(false);

        } else {

            self.history_step(false);

        }

    }


    fn history_step(&mut self, undo: bool) {

        let Some((i, id)) = self.active_ids() else { return };

        let result = if undo { self.session.undo(id) } else { self.session.redo(id) };

        match result {

            Ok(label) => {

                if let Some(doc) = self.session.get(id) {

                    self.views[i].document_changed(&doc.info);

                }

                self.views[i].comments.selected = None;

                let label = crate::i18n::edit_label(&label);

                if undo {

                    self.notify_fmt("Undid {label}", &[("label", &label)]);

                } else {

                    self.notify_fmt("Redid {label}", &[("label", &label)]);

                }

            }

            Err(e) => self.notify_error(e),

        }

    }


    pub fn rotate_active_page(&mut self, clockwise: bool) {

        if let Some((i, _)) = self.active_ids() {

            let current = self.views[i].current;

            let degrees = if clockwise { 90 } else { -90 };

            self.apply_edit(Edit::RotatePages { pages: vec![current], degrees });

        }

    }


    pub fn delete_active_page(&mut self) {

        if let Some((i, id)) = self.active_ids() {

            if let Some(doc) = self.session.get(id) {

                if doc.info.pages.len() > 1 {

                    let current = self.views[i].current;

                    self.apply_edit(Edit::DeletePages { pages: vec![current] });

                } else {

                    self.notify_tr("Cannot delete the only page in a document");

                }

            }

        }

    }


    pub(crate) fn process_pending_edits(&mut self) -> bool {

        let Some(i) = self.active else { return true };

        let applied = self.apply_queued_edit(i);

        if !applied {


            self.deferred_commands.clear();

        }

        match self.views.get_mut(i).and_then(|v| v.pending_action.take()) {

            Some(crate::canvas::ViewAction::InsertFromFile) => self.insert_from_file_dialog(),

            Some(crate::canvas::ViewAction::InsertFromFileAt(at)) => self.insert_from_file_at(Some(at)),

            Some(crate::canvas::ViewAction::ReplacePages) => self.replace_pages_dialog(),

            Some(crate::canvas::ViewAction::Save) => {

                self.save_active(SaveTarget::InPlace);

            }

            Some(crate::canvas::ViewAction::Extract) => self.open_extract_dialog(),

            Some(crate::canvas::ViewAction::Split) => self.dialog = Some(crate::Dialog::Split),

            Some(crate::canvas::ViewAction::CopyPages { cut }) => self.copy_pages(cut),

            Some(crate::canvas::ViewAction::PastePages) => self.paste_pages(),

            Some(crate::canvas::ViewAction::EnhanceCamera) => self.enhance_camera(i),

            Some(crate::canvas::ViewAction::CancelCamera) => self.leave_camera(i),

            Some(crate::canvas::ViewAction::CameraRotate { clockwise }) => {

                if let Some(done) = self.views.get(i).and_then(|v| v.camera_done) {

                    let degrees = if clockwise { 90 } else { -90 };

                    if self.apply_edit(Edit::RotatePages { pages: vec![done.page], degrees })

                        && let Some(d) = self.views.get_mut(i).and_then(|v| v.camera_done.as_mut())

                    {

                        d.turns += degrees;

                        d.steps += 1;

                    }

                }

            }

            Some(crate::canvas::ViewAction::CameraLevel) => self.camera_level(i),

            Some(crate::canvas::ViewAction::CameraNext(command)) => {

                self.leave_camera(i);

                self.execute(command);

            }

            None => {}

        }

        applied

    }





    fn camera_level(&mut self, i: usize) {

        let Some(view) = self.views.get(i) else { return };

        let (id, Some(done)) = (view.id, view.camera_done) else { return };

        let ours = |s: &pdfcraft_engine::Session, k: usize| {

            let label = s.get(id).and_then(|d| d.can_undo().map(str::to_owned));

            let want = if k + 1 == done.steps { "Enhance camera image" } else { "Rotate page" };

            label.as_deref() == Some(want)

        };




        let restore = |app: &mut Self| {

            if let Some(d) = app.views.get_mut(i).and_then(|v| v.camera_done.as_mut()) {

                (d.look, d.level) = d.applied;

            }

        };

        let mut undone = 0;

        while undone < done.steps && ours(&self.session, undone) && self.session.undo(id).is_ok() {

            undone += 1;

        }

        if undone < done.steps {

            for _ in 0..undone {

                if self.session.redo(id).is_err() {

                    break;

                }

            }

            restore(self);

            return;

        }

        let result = self.session.enhance_camera_page_at(id, done.page, done.corners, done.look, done.level).and_then(|()| {

            let turn = done.turns.rem_euclid(360);

            if turn == 0 { Ok(1) } else { self.session.apply(id, Edit::RotatePages { pages: vec![done.page], degrees: turn }).map(|()| 2) }

        });

        if let (Some(doc), Some(view)) = (self.session.get(id), self.views.get_mut(i)) {

            view.document_changed(&doc.info);

            if let Some(d) = view.camera_done.as_mut() {

                match result {

                    Ok(steps) => {

                        d.applied = (d.look, d.level);

                        d.steps = steps;

                    }

                    Err(_) => (d.look, d.level) = d.applied,

                }

            }

        }

        if let Err(e) = result {

            let label = crate::i18n::action_label("Enhance camera image");

            self.notify_fmt("{label} failed: {e}", &[("label", &label), ("e", &e.to_string())]);

        }

    }



    fn leave_camera(&mut self, i: usize) {

        if let Some(view) = self.views.get_mut(i) {

            view.camera = None;

            view.camera_done = None;

        }

        if self.quick_tool == crate::QuickTool::EnhanceCamera {

            self.quick_tool = crate::QuickTool::Select;

        }

    }





    fn enhance_camera(&mut self, i: usize) {

        let Some(view) = self.views.get(i) else { return };

        let (id, Some(q)) = (view.id, view.camera) else { return };

        let Some(page) = self.session.get(id).and_then(|d| d.info.pages.get(q.page).cloned()) else { return };

        let corners = q.corners.map(|[x, y]| {

            let [u, v] = page.view_to_user(x, y);

            [u as f64, v as f64]

        });

        let (look, level) = (crate::camera_ui::Look::Colour, crate::camera_ui::DEFAULT_LEVEL);

        match self.session.enhance_camera_page_at(id, q.page, corners, look, level) {

            Ok(()) => {


                if let (Some(doc), Some(view)) = (self.session.get(id), self.views.get_mut(i)) {

                    view.document_changed(&doc.info);

                    view.camera = None;

                    view.camera_done =

                        Some(crate::camera_ui::CameraDone { page: q.page, corners, look, level, applied: (look, level), turns: 0, steps: 1 });

                }

            }

            Err(e) => {

                let label = crate::i18n::action_label("Enhance camera image");

                self.notify_fmt("{label} failed: {e}", &[("label", &label), ("e", &e.to_string())]);

            }

        }

    }




    pub fn copy_pages(&mut self, cut: bool) {


        if !self.commit_form_typing() {

            return;

        }

        let Some((i, id)) = self.active_ids() else { return };

        let Some(doc) = self.session.get(id) else { return };

        let pages = self.views[i].target_pages();

        let n = doc.info.pages.len();

        if cut && pages.len() >= n {

            self.notify_tr("A document needs at least one page: copy instead");

            return;

        }

        self.page_clipboard = Some(crate::PageClip { from: id, name: doc.name.clone(), bytes: doc.bytes.clone(), pages: pages.clone() });

        if cut {

            self.apply_edit(Edit::DeletePages { pages: pages.clone() });

        }

        let what = if pages.len() == 1 { tl!("1 page").to_string() } else { crate::i18n::fmt(tl!("{n} pages"), &[("n", &pages.len().to_string())]) };

        if cut {

            self.notify_fmt("Cut {what}", &[("what", &what)]);

        } else {

            self.notify_fmt("Copied {what}", &[("what", &what)]);

        }

    }



    pub fn paste_pages(&mut self) {

        let Some(clip) = self.page_clipboard.clone() else {

            self.notify_tr("Copy or cut pages first");

            return;

        };

        let Some(i) = self.active else { return };

        let at = self.views[i].target_pages().into_iter().max().map_or(0, |p| p + 1);

        let count = clip.pages.len();



        let field_conflicts =

            if self.views[i].id == clip.from { pdfcraft_engine::FieldConflicts::Merge } else { pdfcraft_engine::FieldConflicts::Rename };

        if self.apply_edit(Edit::InsertPagesFrom {

            name: clip.name.clone(),

            bytes: clip.bytes.clone(),

            pages: Some(clip.pages.clone()),

            at,

            field_conflicts,

        }) {

            self.views[i].select_pages(&(at..at + count).collect::<Vec<_>>());

        }

    }











    pub(crate) fn commit_form_typing(&mut self) -> bool {

        let Some((i, id)) = self.active_ids() else { return true };

        if !self.apply_queued_edit(i) {

            return false;

        }

        let Some(draft) = self.views.get(i).and_then(|v| v.forms.focus.clone()) else { return true };

        let Some(edit) = self.session.get(id).and_then(|d| crate::forms_ui::draft_edit(&draft, &d.form)) else {

            return true; 

        };

        if let Some(view) = self.views.get_mut(i) {

            view.forms.focus = None;

        }

        if self.apply_edit(edit) {

            return true;

        }

        if let Some(view) = self.views.get_mut(i) {

            let mut draft = draft;

            draft.request_focus = true;

            view.forms.focus = Some(draft);

        }

        false

    }





    fn apply_queued_edit(&mut self, i: usize) -> bool {

        let Some(edit) = self.views.get_mut(i).and_then(|v| v.pending_edit.take()) else { return true };

        let signature_page = self.views.get_mut(i).and_then(|v| v.fill_signature_page.take());

        let committed = self.views.get_mut(i).and_then(|v| v.forms.committed.take());

        let typed = match (&edit, &committed) {

            (Edit::SetFieldValue { name, .. }, Some(draft)) => *name == draft.name,

            _ => false,

        };

        if self.apply_edit(edit) {




            if typed

                && let Some(view) = self.views.get_mut(i)

                && let Some(focus) = view.forms.focus.as_mut()

                && focus.request_focus

                && committed.as_ref().is_some_and(|draft| draft.name == focus.name)

                && let Some(field) = self.session.get(view.id).and_then(|doc| doc.form.iter().find(|field| field.name == focus.name))

            {

                focus.text = field.value.first().cloned().unwrap_or_default();

                focus.picked = field.value.clone();

            }

            if let Some(page) = signature_page

                && let Some(view) = self.views.get_mut(i)

            {



                let newest = self

                    .session

                    .get(view.id)

                    .and_then(|d| d.info.annotations.iter().filter(|a| a.page == page && a.in_reply_to.is_none()).map(|a| a.index).max());

                view.comments.selected = newest.map(|index| (page, index));

                view.comments.reveal = true;

                self.quick_tool = crate::QuickTool::Select;

            }

            return true;

        }

        if !typed {

            return true;

        }

        if let (Some(mut draft), Some(view)) = (committed, self.views.get_mut(i)) {

            draft.request_focus = true;

            view.forms.focus = Some(draft);

        }

        false

    }





    pub(crate) fn commit_typing_in(&mut self, id: pdfcraft_engine::DocId) -> bool {

        let Some(i) = self.views.iter().position(|v| v.id == id) else { return true };



        let blocked = self.views.get_mut(i).and_then(|v| {

            let c = v.content.blocked()?;

            v.content.hold_blocked();

            if let Some(page) = v.content.draft.as_ref().map(|d| d.page) {

                v.go_to_page(page);

            }

            Some(c)

        });

        if let Some(c) = blocked {

            self.active = Some(i);

            self.notify(crate::content_ui::undrawable_message(c));

            return false;

        }

        let typing = self

            .views

            .get(i)

            .is_some_and(|v| v.pending_edit.is_some() || v.forms.focus.is_some() || v.fill_text.as_ref().is_some_and(|t| !t.text.trim().is_empty()));

        if self.active != Some(i) {

            if !typing {

                return true;

            }

            self.active = Some(i);

        }

        self.commit_form_typing() && self.commit_open_fill_text(id)

    }




    fn commit_open_fill_text(&mut self, id: pdfcraft_engine::DocId) -> bool {

        let Some(i) = self.views.iter().position(|v| v.id == id) else { return true };

        let Some(tb) = self.views[i].fill_text.clone() else { return true };

        let text = tb.text.trim().to_string();

        if text.is_empty() {

            if let Some(view) = self.views.get_mut(i) {

                view.fill_text = None;

            }

            return true;

        }

        let author = self.comment_prefs.author.clone();

        let rotation = self.session.get(id).map_or(0, |doc| crate::comments::page_rotation(&doc.info, tb.page));

        let edit = crate::fill_sign::typed(tb.page, tb.at, &text, tb.size, rotation, &author);

        if self.apply_edit(edit) {

            if let Some(view) = self.views.get_mut(i) {

                view.fill_text = None;

            }

            true

        } else {

            false

        }

    }




    fn flatten_fill_sign_for_save(&mut self, id: pdfcraft_engine::DocId) -> bool {

        if !self.flatten_fill_sign_on_save {

            return true;

        }

        let before = self.session.get(id).map(|d| d.edit_generation());

        if let Err(e) = self.session.apply(id, Edit::FlattenFillSign) {

            self.notify_error(e);

            return false;

        }

        let after = self.session.get(id).map(|d| d.edit_generation());

        if before != after

            && let Some(doc) = self.session.get(id)

            && let Some(view) = self.views.iter_mut().find(|v| v.id == id)

        {

            view.document_changed(&doc.info);

        }

        true

    }



    pub(crate) fn has_unsaved_work(&self, index: usize) -> bool {

        let Some(v) = self.views.get(index) else { return false };

        let Some(doc) = self.session.get(v.id) else { return false };

        doc.dirty

            || v.pending_edit.is_some()

            || v.content.blocked().is_some()

            || v.forms.focus.as_ref().is_some_and(|f| crate::forms_ui::draft_edit(f, &doc.form).is_some())

            || v.fill_text.as_ref().is_some_and(|t| !t.text.trim().is_empty())

            || v.line_editor.as_ref().map_or_else(|| false, |ed| ed.has_unsaved_text())

    }



    pub fn save_active(&mut self, target: SaveTarget) -> bool {

        match self.active {

            Some(i) => self.save_view(i, target),

            None => false,

        }

    }





    pub fn save_view(&mut self, index: usize, target: SaveTarget) -> bool {

        self.save_then(index, target, |_| {})

    }




    pub(crate) fn save_then(&mut self, index: usize, target: SaveTarget, after: impl FnOnce(&mut Self) + Send + 'static) -> bool {


        if let Some(id) = self.views.get(index).map(|v| v.id)

            && !self.commit_typing_in(id)

        {

            return false;

        }

        let Some(id) = self.views.get(index).map(|v| v.id) else { return false };

        let Some(doc) = self.session.get(id) else { return false };

        let (name, path) = (doc.name.clone(), doc.path.clone());

        #[cfg(not(target_arch = "wasm32"))]

        {

            let destination = match (target, path, &self.save_override) {

                (_, _, Some(p)) => p.clone(),



                (SaveTarget::InPlace, Some(p), _) if !is_mail_temp_copy(&p) => p,

                _ => {

                    let name = if name.to_ascii_lowercase().ends_with(".pdf") { name } else { format!("{name}.pdf") };

                    let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(name);


                    self.ask_one(crate::pickers::Ask::Save(dialog), None, move |app, dest| {

                        app.save_doc_to_then(id, &dest.to_string_lossy(), Box::new(after));

                    });

                    return false;

                }

            };

            self.save_doc_to_then(id, &destination, Box::new(after))

        }

        #[cfg(target_arch = "wasm32")]

        {

            let _ = path;

            if !self.flatten_fill_sign_for_save(id) {

                return false;

            }

            let bytes = match self.session.save_bytes(id) {

                Ok(b) => b,

                Err(e) => {

                    self.notify_fmt("Couldn't save {name}: {e}", &[("name", &name), ("e", &e.to_string())]);

                    return false;

                }

            };



            let to_host = self.host_save.is_some();

            let delivered = match &self.host_save {

                Some(save) => save(&name, &bytes, matches!(target, SaveTarget::As)),

                None => download(&name, &bytes),

            };

            match delivered {

                Ok(()) => {

                    let _ = self.session.mark_saved(id, bytes, None);

                    if let Some(doc) = self.session.get(id)

                        && let Some(view) = self.views.get_mut(index)

                    {

                        view.document_changed(&doc.info);

                    }


                    if !to_host {

                        self.notify_fmt("Downloaded {name}", &[("name", &name)]);

                    }

                    after(self);

                    true

                }

                Err(e) if to_host => {

                    self.notify_fmt("Couldn't save {name}: {e}", &[("name", &name), ("e", &e)]);

                    false

                }

                Err(e) => {

                    self.notify_fmt("Couldn't download {name}: {e}", &[("name", &name), ("e", &e.to_string())]);

                    false

                }

            }

        }

    }







    #[cfg(not(target_arch = "wasm32"))]

    fn save_doc_to_then(&mut self, id: pdfcraft_engine::DocId, dest: &str, after: Box<dyn FnOnce(&mut Self) + Send>) -> bool {

        let own = self.session.get(id).and_then(|d| d.path.as_deref()).is_some_and(|p| crate::same_file(p, dest));

        if own && self.watch.disk_differs(id, dest) {

            self.pending_overwrite = Some(PendingOverwrite { id, dest: dest.to_string(), after });

            self.dialog = Some(crate::Dialog::OverwriteChanged);

            return false;

        }

        let saved = self.save_doc_to(id, dest);

        if saved {

            after(self);

        }

        saved

    }



    #[cfg(not(target_arch = "wasm32"))]

    pub(crate) fn overwrite_changed_file(&mut self) {

        let Some(p) = self.pending_overwrite.take() else { return };

        if self.save_doc_to(p.id, &p.dest) {

            (p.after)(self);

        }

    }



    #[cfg(not(target_arch = "wasm32"))]

    pub(crate) fn save_changed_file_elsewhere(&mut self) {

        let Some(p) = self.pending_overwrite.take() else { return };

        if let Some(index) = self.views.iter().position(|v| v.id == p.id) {

            self.save_then(index, SaveTarget::As, p.after);

        }

    }



    #[cfg(not(target_arch = "wasm32"))]

    fn save_doc_to(&mut self, id: pdfcraft_engine::DocId, dest: &str) -> bool {



        if !self.commit_typing_in(id) || !self.flatten_fill_sign_for_save(id) {

            return false;

        }

        let Some(name) = self.session.get(id).map(|d| d.name.clone()) else {

            self.notify_tr("The document was closed before it could be saved.");

            return false;

        };

        let bytes = match self.session.save_bytes(id).and_then(|b| self.session.check_save(id, &b).map(|()| b)) {

            Ok(b) => b,

            Err(e) => {

                self.notify_fmt("Couldn't save {name}: {e}", &[("name", &name), ("e", &e.to_string())]);

                return false;

            }

        };

        if let Err(e) = write_atomically(dest, &bytes) {

            self.notify_fmt("Couldn't save {name}: {e}", &[("name", dest), ("e", &e.to_string())]);

            return false;

        }


        self.watch.known_now(id, dest);

        match self.session.mark_saved(id, bytes, Some(dest.to_string())) {

            Ok(()) => {

                self.forget_recovery(id);

                if let Some(doc) = self.session.get(id)

                    && let Some(view) = self.views.iter_mut().find(|v| v.id == id)

                {

                    view.document_changed(&doc.info);


                    view.disk_changed = false;

                }

                self.notify_fmt("Saved {name}", &[("name", &short_name(dest))]);

                true

            }

            Err(e) => {

                self.notify_fmt("Saved, but reopening failed: {e}", &[("e", &e.to_string())]);

                false

            }

        }

    }



    pub fn request_close_tab(&mut self, index: usize) {


        if self.has_unsaved_work(index)

            && let Some(id) = self.views.get(index).map(|v| v.id)

        {

            self.close_request = Some(CloseRequest::Tab(id));

        } else {

            self.close_tab(index);

        }

    }



    pub fn close_all(&mut self) {

        for i in (0..self.views.len()).rev() {

            if !self.has_unsaved_work(i) {

                self.close_tab(i);

            }

        }

        if self.first_dirty().is_some() {

            self.close_request = Some(CloseRequest::All);

        }

    }



    pub fn revert_active(&mut self) {

        let Some((_, id)) = self.active_ids() else { return };

        match self.session.revert(id) {

            Ok(()) => {

                if let Some(i) = self.active

                    && let Some(d) = self.session.get(id)

                {

                    let view = &mut self.views[i];

                    view.document_changed(&d.info);



                    view.forms.focus = None;

                    view.forms.committed = None;

                    view.forms.unbaked.clear();

                    view.pending_edit = None;

                }

                self.notify_tr("Reverted to the last saved version");

            }

            Err(e) => self.notify_error(e),

        }

    }



    pub fn first_dirty(&self) -> Option<usize> {

        (0..self.views.len()).find(|&i| self.has_unsaved_work(i))

    }



    pub fn resolve_close(&mut self, ctx: &egui::Context, choice: Option<bool>) {

        let Some(req) = self.close_request.take() else { return };

        let index = match req {

            CloseRequest::Tab(id) => match self.views.iter().position(|v| v.id == id) {

                Some(i) => i,

                None => return, 

            },

            CloseRequest::Quit | CloseRequest::All => match self.first_dirty() {

                Some(i) => i,

                None => {

                    if req == CloseRequest::Quit {

                        self.quit(ctx);

                    }

                    return;

                }

            },

        };



        if req == CloseRequest::Quit {

            match choice {

                Some(_) => self.note_quit_session(),

                None => self.forget_quit_session(),

            }

        }

        match choice {

            None => {} 

            Some(false) => self.close_and_continue(ctx, index, req),

            Some(true) => {

                let Some(id) = self.views.get(index).map(|v| v.id) else { return };



                self.active = Some(index);




                let ctx = ctx.clone();

                self.save_then(index, SaveTarget::InPlace, move |app| {

                    if let Some(i) = app.views.iter().position(|v| v.id == id) {

                        app.close_and_continue(&ctx, i, req);

                    }

                });

            }

        }

    }



    fn close_and_continue(&mut self, ctx: &egui::Context, index: usize, req: CloseRequest) {

        self.close_tab(index);


        if (req == CloseRequest::Quit || req == CloseRequest::All) && self.close_request.is_none() {

            match self.first_dirty() {

                Some(_) => self.close_request = Some(req),

                None if req == CloseRequest::Quit => self.quit(ctx),

                None => {}

            }

        }

    }




    pub(crate) fn request_quit(&mut self, ctx: &egui::Context) -> bool {

        if self.first_dirty().is_some() {

            self.close_request = Some(CloseRequest::Quit);

            ctx.request_repaint();

            false

        } else {

            self.shutdown_recovery();

            self.quit(ctx);

            true

        }

    }


    fn quit(&mut self, ctx: &egui::Context) {

        self.allow_quit = true;

        ctx.send_viewport_cmd(egui::ViewportCommand::Close);

    }



    pub(crate) fn sync_unsaved_flag(&self) {

        self.unsaved_flag.store(self.first_dirty().is_some(), std::sync::atomic::Ordering::Relaxed);

    }



    pub(crate) fn guard_quit(&mut self, ctx: &egui::Context) {


        ctx.options_mut(|o| o.quit_shortcuts.clear());

        let shortcut = ctx.input(|i| {

            i.events.iter().any(|e| {

                matches!(e, egui::Event::Key { key: egui::Key::Q, pressed: true, modifiers, .. }

                    if modifiers.matches_exact(egui::Modifiers::COMMAND))

            })

        });

        if shortcut {

            self.request_quit(ctx);

        }

        self.sync_unsaved_flag();

        if !ctx.input(|i| i.viewport().close_requested()) {

            return;

        }


        if !self.allow_quit && self.first_dirty().is_some() {

            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);

            self.close_request = Some(CloseRequest::Quit);

        } else {


            self.shutdown_recovery();

        }

    }

}




fn content_page(edit: &Edit) -> Option<usize> {

    match edit {

        Edit::MoveObjects { page, .. }

        | Edit::EditTextLine { page, .. }

        | Edit::EditTextBlock { page, .. }

        | Edit::EditPageImage { page, .. }

        | Edit::AddText { page, .. }

        | Edit::AddImage { page, .. }

        | Edit::UpdateContent { page, .. }

        | Edit::DeleteContent { page, .. }

        | Edit::ReplaceImage { page, .. } => Some(*page),

        _ => None,

    }

}


fn comment_page(edit: &Edit) -> Option<usize> {

    match edit {

        Edit::AddAnnotation(a) => Some(a.page),

        Edit::DeleteAnnotation { page, .. }

        | Edit::SetAnnotationContents { page, .. }

        | Edit::ReplyToAnnotation { page, .. }

        | Edit::SetAnnotationStatus { page, .. }

        | Edit::MoveAnnotation { page, .. }

        | Edit::ResizeAnnotation { page, .. }

        | Edit::StyleAnnotation { page, .. }

        | Edit::FillAnnotation { page, .. }

        | Edit::FontSizeAnnotation { page, .. }

        | Edit::SetAnnotationInfo { page, .. } => Some(*page),

        _ => None,

    }

}



fn is_form_edit(edit: &Edit) -> bool {

    matches!(

        edit,

        Edit::SetFieldValue { .. }

            | Edit::ResetForm { .. }

            | Edit::AddField { .. }

            | Edit::DeleteField { .. }

            | Edit::DuplicateField { .. }

            | Edit::SetFieldProps { .. }

            | Edit::SetFieldImage { .. }

            | Edit::SetFieldScript { .. }

            | Edit::ApplyScriptChanges { .. }

    )

}











pub(crate) fn is_mail_temp_copy(path: &str) -> bool {

    let p = path.replace('\\', "/").to_ascii_lowercase();

    p.contains("/inetcache/content.outlook/")

        || p.contains("/temporary internet files/content.outlook/")

        || p.contains("/microsoft.outlookforwindows")

}




pub fn write_atomically(path: &str, bytes: &[u8]) -> std::io::Result<()> {

    write_atomically_with(path, bytes, staging_suffixes())

}



fn write_atomically_with(path: &str, bytes: &[u8], suffixes: impl IntoIterator<Item = u64>) -> std::io::Result<()> {

    write_atomically_syncing(path, bytes, suffixes, std::fs::File::sync_all)

}





fn write_atomically_syncing(

    path: &str,

    bytes: &[u8],

    suffixes: impl IntoIterator<Item = u64>,

    sync_all: impl FnOnce(&std::fs::File) -> std::io::Result<()>,

) -> std::io::Result<()> {

    pdfcraft_platform::staging::write_atomically_with(std::path::Path::new(path), bytes, StagingName::TagThenSuffix, suffixes, sync_all)

}


#[cfg(not(target_arch = "wasm32"))]

fn short_name(path: &str) -> String {

    std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())

}



#[cfg(target_arch = "wasm32")]

pub(crate) fn download(name: &str, bytes: &[u8]) -> Result<(), String> {

    use wasm_bindgen::JsCast;

    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");

    let array = js_sys::Uint8Array::from(bytes);

    let parts = js_sys::Array::of1(&array);

    let opts = web_sys::BlobPropertyBag::new();

    opts.set_type("application/pdf");

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;

    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;

    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;

    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(err)?.dyn_into().map_err(|_| "anchor")?;

    a.set_href(&url);

    a.set_download(name);

    a.click();

    let _ = web_sys::Url::revoke_object_url(&url);

    Ok(())

}


impl PdfCraftApp {


    pub fn bookmark_action(&mut self, action: crate::panels::BmAction) {

        use crate::panels::BmAction as A;

        let Some((i, id)) = self.active_ids() else { return };

        let current = self.views[i].current;

        let parent_of = |p: &[usize]| p[..p.len() - 1].to_vec();

        let edit = match action {

            A::New => {

                let n = self.session.get(id).map_or(0, |d| d.info.outline.len());

                self.apply_edit(Edit::AddBookmark { parent: vec![], index: n, title: "Untitled".into(), page: current });


                self.bookmark_rename = Some((vec![n], "Untitled".into()));

                self.right = Some(crate::RightPanel::Bookmarks);

                return;

            }

            A::StartRename(path) => {

                let title = self.session.get(id).and_then(|d| bookmark_at(&d.info.outline, &path)).map(|b| b.title.clone()).unwrap_or_default();

                self.bookmark_rename = Some((path, title));

                return;

            }

            A::Rename(path, title) => Edit::RenameBookmark { path, title },

            A::SetToCurrentPage(path) => Edit::SetBookmarkPage { path, page: current },

            A::Delete(path) => Edit::DeleteBookmark { path },

            A::MoveUp(path) => {

                let at = path[path.len() - 1].saturating_sub(1);

                Edit::MoveBookmark { to_parent: parent_of(&path), from: path, index: at }

            }

            A::MoveDown(path) => {

                let at = path[path.len() - 1] + 1;

                Edit::MoveBookmark { to_parent: parent_of(&path), from: path, index: at }

            }

            A::Indent(path) => {

                let mut to_parent = parent_of(&path);

                to_parent.push(path[path.len() - 1].saturating_sub(1));

                Edit::MoveBookmark { from: path, to_parent, index: usize::MAX }

            }

            A::Outdent(path) => {

                let parent = parent_of(&path);

                let grand = parent_of(&parent);

                let at = parent[parent.len() - 1] + 1;

                Edit::MoveBookmark { from: path, to_parent: grand, index: at }

            }

            A::FromStructure => {

                self.right = Some(crate::RightPanel::Bookmarks);

                Edit::BookmarksFromStructure

            }

        };

        self.apply_edit(edit);

    }

}


fn bookmark_at<'a>(items: &'a [pdfcraft_render::OutlineItem], path: &[usize]) -> Option<&'a pdfcraft_render::OutlineItem> {

    let (first, rest) = path.split_first()?;

    let item = items.get(*first)?;

    if rest.is_empty() { Some(item) } else { bookmark_at(&item.children, rest) }

}


#[cfg(test)]

mod tests {

    use super::{SaveTarget, is_mail_temp_copy, write_atomically, write_atomically_syncing, write_atomically_with};

    use pdfcraft_platform::staging::{STAGING_ATTEMPTS, staging_suffixes};

    use std::path::{Path, PathBuf};



    fn staging_dir(test: &str) -> PathBuf {

        let dir = std::env::temp_dir().join(format!("pdfcraft-ui-staging-{}-{test}", std::process::id()));

        let _ = std::fs::remove_dir_all(&dir);

        std::fs::create_dir_all(&dir).unwrap();

        dir

    }


    fn staged(dir: &Path, suffix: u64) -> PathBuf {

        dir.join(format!(".out.pdf.pdfcraft-{suffix:016x}.tmp"))

    }


    fn read(p: &Path) -> String {

        std::fs::read_to_string(p).unwrap()

    }




    fn file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {

        #[cfg(unix)]

        {

            std::os::unix::fs::symlink(target, link)

        }

        #[cfg(windows)]

        {

            std::os::windows::fs::symlink_file(target, link)

        }

        #[cfg(not(any(unix, windows)))]

        {

            Err(std::io::Error::other(format!("no symbolic links here: {} {}", target.display(), link.display())))

        }

    }


    #[test]

    fn saving_never_writes_through_a_file_planted_at_the_staging_name() {

        let dir = staging_dir("planted");

        let outside = dir.join("outside.txt");

        std::fs::write(&outside, "PRECIOUS").unwrap();

        let target = dir.join("out.pdf");

        let path = target.to_string_lossy().into_owned();


        std::fs::hard_link(&outside, staged(&dir, 1)).unwrap();

        std::fs::write(staged(&dir, 2), "PLANTED").unwrap();

        write_atomically_with(&path, b"NEW", [1, 2, 3]).unwrap();

        assert_eq!(read(&target), "NEW");

        assert_eq!(read(&outside), "PRECIOUS");

        assert_eq!(read(&staged(&dir, 1)), "PRECIOUS");

        assert_eq!(read(&staged(&dir, 2)), "PLANTED");

        assert!(!staged(&dir, 3).exists(), "the staging file was renamed into place");

        let _ = std::fs::remove_dir_all(&dir);

    }


    #[test]

    fn saving_never_writes_through_a_symbolic_link_at_the_staging_name() {

        let dir = staging_dir("symlink");

        let outside = dir.join("outside.txt");

        std::fs::write(&outside, "PRECIOUS").unwrap();

        let target = dir.join("out.pdf");

        if let Err(e) = file_symlink(&outside, &staged(&dir, 1)) {

            eprintln!("symbolic links not checked: {e}");

            return;

        }

        write_atomically_with(&target.to_string_lossy(), b"NEW", [1, 2]).unwrap();

        assert_eq!(read(&target), "NEW");

        assert_eq!(read(&outside), "PRECIOUS");

        let _ = std::fs::remove_dir_all(&dir);

    }


    #[test]

    fn saving_never_creates_a_file_through_a_dangling_link_at_the_staging_name() {

        let dir = staging_dir("dangling");

        let unborn = dir.join("created-through-a-link.txt");

        let target = dir.join("out.pdf");

        if let Err(e) = file_symlink(&unborn, &staged(&dir, 1)) {

            eprintln!("symbolic links not checked: {e}");

            return;

        }

        write_atomically_with(&target.to_string_lossy(), b"NEW", [1, 2]).unwrap();

        assert_eq!(read(&target), "NEW");

        assert!(!unborn.exists(), "nothing was created through the dangling link");

        let _ = std::fs::remove_dir_all(&dir);

    }


    #[test]

    fn a_folder_at_the_staging_name_is_left_alone() {

        let dir = staging_dir("folder");

        std::fs::create_dir(staged(&dir, 1)).unwrap();

        std::fs::write(staged(&dir, 1).join("inside.txt"), "PLANTED").unwrap();

        let target = dir.join("out.pdf");

        write_atomically_with(&target.to_string_lossy(), b"NEW", [1, 2]).unwrap();

        assert_eq!(read(&target), "NEW");

        assert_eq!(read(&staged(&dir, 1).join("inside.txt")), "PLANTED");

        let _ = std::fs::remove_dir_all(&dir);

    }


    #[test]

    fn saving_gives_up_rather_than_reuse_a_taken_name() {

        let dir = staging_dir("taken");

        let target = dir.join("out.pdf");

        std::fs::write(&target, "OLD").unwrap();

        for s in 1..=STAGING_ATTEMPTS as u64 {

            std::fs::write(staged(&dir, s), "PLANTED").unwrap();

        }

        assert!(write_atomically_with(&target.to_string_lossy(), b"NEW", 1..).is_err());

        assert_eq!(read(&target), "OLD");

        for s in 1..=STAGING_ATTEMPTS as u64 {

            assert_eq!(read(&staged(&dir, s)), "PLANTED");

        }

        let _ = std::fs::remove_dir_all(&dir);

    }



    fn listing(dir: &Path) -> Vec<String> {

        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();

        names.sort();

        names

    }


    #[test]

    fn saving_a_long_name_leaves_only_the_saved_file() {

        assert_ne!(staging_suffixes().next(), staging_suffixes().next(), "each save draws new names");



        let dir = staging_dir("clean");

        let name = format!("{}.pdf", "\u{1F600}".repeat(60));

        let path = dir.join(&name).to_string_lossy().into_owned();

        write_atomically(&path, b"NEW").unwrap();

        write_atomically(&path, b"NEWER").unwrap();

        assert_eq!(read(&dir.join(&name)), "NEWER");

        assert_eq!(listing(&dir), [name], "no staging file is left behind");

        let _ = std::fs::remove_dir_all(&dir);

    }




    #[cfg(windows)]

    #[test]

    fn a_failed_save_removes_the_staging_file() {

        let dir = staging_dir("readonly");

        let target = dir.join("out.pdf");

        std::fs::write(&target, "OLD").unwrap();

        let mut perms = std::fs::metadata(&target).unwrap().permissions();

        perms.set_readonly(true);

        std::fs::set_permissions(&target, perms.clone()).unwrap();

        let e = write_atomically_with(&target.to_string_lossy(), b"NEW", [7]).unwrap_err();

        assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied, "the rename failed: {e}");

        assert_eq!(read(&target), "OLD");

        assert_eq!(listing(&dir), ["out.pdf"], "the staging file was removed");

        #[allow(clippy::permissions_set_readonly_false)]

        perms.set_readonly(false);

        std::fs::set_permissions(&target, perms).unwrap();

        let _ = std::fs::remove_dir_all(&dir);

    }




    #[cfg(target_os = "macos")]

    #[test]

    fn a_volume_without_full_flush_still_saves() {

        for errno in [45, 25] {

            let dir = staging_dir(&format!("nofullsync-{errno}"));

            let target = dir.join("out.pdf");

            std::fs::write(&target, "OLD").unwrap();

            let asked = std::cell::Cell::new(false);

            let refuse = |_: &std::fs::File| {

                asked.set(true);

                Err(std::io::Error::from_raw_os_error(errno))

            };

            write_atomically_syncing(&target.to_string_lossy(), b"NEW", [7], refuse).unwrap();

            assert!(asked.get(), "the full flush was tried");

            assert_eq!(read(&target), "NEW");

            assert_eq!(listing(&dir), ["out.pdf"], "no staging file is left behind");

            let _ = std::fs::remove_dir_all(&dir);

        }

    }




    #[test]

    fn a_failed_flush_keeps_the_original() {

        let dir = staging_dir("syncfails");

        let target = dir.join("out.pdf");

        std::fs::write(&target, "OLD").unwrap();

        let e = write_atomically_syncing(&target.to_string_lossy(), b"NEW", [7], |_| Err(std::io::Error::other("disk gone"))).unwrap_err();

        assert_eq!(e.to_string(), "disk gone");

        assert_eq!(read(&target), "OLD");

        assert_eq!(listing(&dir), ["out.pdf"], "the staging file was removed");

        let _ = std::fs::remove_dir_all(&dir);

    }


    #[test]

    fn outlook_attachment_copies_are_recognised_on_either_separator_and_in_any_case() {

        for p in [

            r"C:\Users\me\AppData\Local\Microsoft\Windows\INetCache\Content.Outlook\AB12CD34\Quote.pdf",

            r"c:\users\me\appdata\local\microsoft\windows\inetcache\content.outlook\ab12cd34\quote (2).pdf",

            r"C:\Users\me\AppData\Local\Microsoft\Windows\Temporary Internet Files\Content.Outlook\AB12CD34\Quote.pdf",

            r"C:\Users\me\AppData\Local\Packages\Microsoft.OutlookForWindows_8wekyb3d8bbwe\LocalCache\Quote.pdf",

            "C:/Users/me/AppData/Local/Microsoft/Windows/INetCache/Content.Outlook/AB12CD34/Quote.pdf",

        ] {

            assert!(is_mail_temp_copy(p), "{p}");

        }

    }


    #[test]

    fn ordinary_paths_are_not_mail_copies() {

        for p in [

            r"C:\Users\me\Documents\Quote.pdf",

            r"C:\Users\me\OneDrive - Contoso\Content.Outlook notes\Quote.pdf",

            r"C:\Users\me\AppData\Local\Microsoft\Windows\INetCache\IE\Quote.pdf",

            "/home/me/Documents/Quote.pdf",

            "Quote.pdf",

        ] {

            assert!(!is_mail_temp_copy(p), "{p}");

        }

    }



    fn one_page_pdf() -> Vec<u8> {

        let objs = [

            "<< /Type /Catalog /Pages 2 0 R >>",

            "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 300] >>",

            "<< /Type /Page /Parent 2 0 R >>",

        ];

        let mut out = b"%PDF-1.7\n".to_vec();

        let mut offsets = Vec::new();

        for (i, o) in objs.iter().enumerate() {

            offsets.push(out.len());

            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());

        }

        let xref = out.len();

        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());

        for o in offsets {

            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());

        }

        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());

        out

    }


    #[test]

    fn save_asks_where_to_keep_an_email_attachment_and_leaves_the_temporary_copy_alone() {

        let dir = staging_dir("mail-temp");

        let cache = dir.join("INetCache").join("Content.Outlook").join("AB12CD34");

        std::fs::create_dir_all(&cache).unwrap();

        let attachment = cache.join("Quote.pdf");

        let original = one_page_pdf();

        std::fs::write(&attachment, &original).unwrap();

        let kept = dir.join("Quote.pdf");


        let mut app = crate::PdfCraftApp::new();

        app.set_option("language", "en").unwrap();

        app.open_bytes("Quote.pdf", Some(attachment.to_string_lossy().into_owned()), original.clone()).unwrap();

        assert!(app.apply_edit(pdfcraft_engine::Edit::RotatePages { pages: vec![0], degrees: 90 }));

        app.pick_override = Some(vec![kept.to_string_lossy().into_owned()]);


        assert!(!app.save_active(SaveTarget::InPlace), "Save asks instead of writing the temporary copy");

        app.process_picked();

        assert_eq!(std::fs::read(&attachment).unwrap(), original, "the attachment's copy is untouched");

        assert!(std::fs::read(&kept).unwrap().starts_with(b"%PDF"), "the document went where the user chose");



        app.pick_override = None;

        assert!(app.apply_edit(pdfcraft_engine::Edit::RotatePages { pages: vec![0], degrees: 90 }));

        assert!(app.save_active(SaveTarget::InPlace));

        let _ = std::fs::remove_dir_all(&dir);

    }

}


#[cfg(test)]

mod revert_draft_tests {

    use super::*;

    use crate::forms_ui::Focus;

    use pdfcraft_engine::FieldValue;


    fn focus(text: &str) -> Focus {

        Focus {

            name: "name".into(),

            widget: 0,

            text: text.into(),

            picked: Vec::new(),

            request_focus: false,

            select_all: false,

            press: None,

            calendar: None,

            calendar_rect: None,

        }

    }


    fn app() -> PdfCraftApp {

        let mut app = PdfCraftApp::new();

        app.open_bytes("source.pdf", None, include_bytes!("../tests/data/form.pdf").to_vec()).unwrap();

        app

    }


    fn queue_draft(app: &mut PdfCraftApp, index: usize, text: &str) {

        app.views[index].forms.focus = Some(focus(text));

        let form = app.session.get(app.views[index].id).unwrap().form.clone();

        crate::forms_ui::commit(&mut app.views[index], &form);

        assert!(app.views[index].forms.committed.is_some() && app.views[index].pending_edit.is_some());

    }


    #[test]

    fn control_quit_prompts_for_uncommitted_typing_and_cancel_retains_it() {

        for queued in [false, true] {

            let mut app = app();

            let id = app.views[0].id;

            app.open_bytes("other.pdf", None, include_bytes!("../tests/data/form.pdf").to_vec()).unwrap();

            if queued {

                queue_draft(&mut app, 0, "Unsaved typing");

            } else {

                app.views[0].forms.focus = Some(focus("Unsaved typing"));

            }

            assert!(!app.session.get(id).unwrap().dirty);

            assert_eq!(app.active, Some(1));

            let ctx = egui::Context::default();

            assert!(!app.request_quit(&ctx));

            assert_eq!(app.close_request, Some(CloseRequest::Quit));

            assert!(!app.allow_quit);

            app.resolve_close(&ctx, None);

            assert!(app.close_request.is_none());

            assert_eq!(app.views.len(), 2);

            assert!(app.has_unsaved_work(0));

            assert_eq!(app.views[0].id, id);

            assert!(!app.session.get(id).unwrap().dirty);

            if queued {

                assert!(

                    matches!(&app.views[0].pending_edit, Some(Edit::SetFieldValue { value: FieldValue::Text(text), .. }) if text == "Unsaved typing")

                );

            } else {

                assert_eq!(app.views[0].forms.focus.as_ref().unwrap().text, "Unsaved typing");

            }

        }

    }




    #[test]

    fn a_value_its_font_cannot_draw_is_kept_and_explained() {

        let mut app = app();

        let fill =

            |app: &mut PdfCraftApp, text: &str| app.apply_edit(Edit::SetFieldValue { name: "name".into(), value: FieldValue::Text(text.into()) });

        assert!(fill(&mut app, "张三"));

        let toast = app.toast.as_ref().map(|t| t.0.clone()).expect("a notice");

        assert!(toast.contains("“张” (U+5F20)") && toast.contains("other PDF viewers redraw the field"), "{toast}");

        assert_eq!(app.session.get(app.views[0].id).unwrap().form.iter().find(|f| f.name == "name").unwrap().value, ["张三"]);


        let previous = crate::i18n::current();

        crate::i18n::set_current(crate::i18n::Lang::from_code("zh-hans").expect("registered"));

        assert!(fill(&mut app, "李四"));

        crate::i18n::set_current(previous);

        let toast = app.toast.as_ref().map(|t| t.0.clone()).expect("a notice");

        assert!(toast.contains("此字段的字体无法显示“李”（U+674E）"), "{toast}");

        app.toast = None;

        assert!(fill(&mut app, "Ada"));

        assert!(app.toast.is_none(), "{:?}", app.toast);

    }


    #[test]

    fn unsaved_flag_follows_dirty_state() {

        use std::sync::atomic::Ordering;


        let ctx = egui::Context::default();

        let mut app = app();

        app.guard_quit(&ctx);

        assert!(!app.unsaved_flag.load(Ordering::Relaxed));

        assert!(app.apply_edit(Edit::SetFieldValue { name: "name".into(), value: FieldValue::Text("Changed".into()) }));

        app.guard_quit(&ctx);

        assert!(app.unsaved_flag.load(Ordering::Relaxed));

    }


    #[test]

    fn successful_revert_clears_focused_or_queued_drafts_only_on_its_document() {

        for queued in [false, true] {

            let mut app = app();

            let source = app.views[0].id;

            assert!(app.apply_edit(Edit::SetFieldValue { name: "name".into(), value: FieldValue::Text("Committed change".into()) }));

            if queued {

                queue_draft(&mut app, 0, "Queued source draft");

            } else {

                app.views[0].forms.focus = Some(focus("Focused source draft"));

            }

            app.open_bytes("other.pdf", None, include_bytes!("../tests/data/form.pdf").to_vec()).unwrap();

            let other = app.views[1].id;

            assert!(app.apply_edit(Edit::SetFieldValue { name: "name".into(), value: FieldValue::Text("Other committed value".into()) }));

            queue_draft(&mut app, 1, "Other queued draft");

            app.views[1].forms.focus = Some(focus("Other focused draft"));

            let other_bytes = app.session.get(other).unwrap().bytes.clone();

            let other_generation = app.session.get(other).unwrap().edit_generation();

            let source_generation = app.session.get(source).unwrap().edit_generation();

            app.active = Some(0);

            app.revert_active();

            let reverted = app.session.get(source).unwrap();

            assert!(!reverted.dirty);

            assert_eq!(reverted.edit_generation(), source_generation + 1);

            assert!(reverted.form.iter().find(|field| field.name == "name").unwrap().value.is_empty());

            assert!(app.views[0].forms.focus.is_none() && app.views[0].forms.committed.is_none() && app.views[0].pending_edit.is_none());

            assert!(!app.has_unsaved_work(0));

            assert_eq!(app.views[1].forms.focus.as_ref().unwrap().text, "Other focused draft");

            assert_eq!(app.views[1].forms.committed.as_ref().unwrap().text, "Other queued draft");

            assert!(

                matches!(&app.views[1].pending_edit, Some(Edit::SetFieldValue { name, value: FieldValue::Text(text) }) if name == "name" && text == "Other queued draft")

            );

            let untouched = app.session.get(other).unwrap();

            assert_eq!(untouched.bytes, other_bytes);

            assert_eq!(untouched.edit_generation(), other_generation);

            assert!(untouched.dirty && app.has_unsaved_work(1));

            assert_eq!(app.active_ids(), Some((0, source)));

        }

    }


    #[test]

    fn failed_revert_preserves_focused_and_queued_drafts() {

        let mut app = app();

        let source = app.views[0].id;

        queue_draft(&mut app, 0, "Queued draft");

        app.views[0].forms.focus = Some(focus("Focused draft"));



        app.session.close(source);

        app.revert_active();

        assert_eq!(app.views[0].forms.focus.as_ref().unwrap().text, "Focused draft");

        assert_eq!(app.views[0].forms.committed.as_ref().unwrap().text, "Queued draft");

        assert!(

            matches!(&app.views[0].pending_edit, Some(Edit::SetFieldValue { name, value: FieldValue::Text(text) }) if name == "name" && text == "Queued draft")

        );

        assert_eq!(app.active_ids(), Some((0, source)));

        assert_eq!(app.toast.as_ref().unwrap().0, pdfcraft_engine::EditError::NoDocument.to_string());

    }

}


#[cfg(test)]

mod content_page_tests {

    use super::content_page;

    use pdfcraft_engine::{Edit, ObjectKind, ObjectTarget};




    #[test]

    fn single_page_content_edits_map_to_their_page() {

        let moved = Edit::MoveObjects { page: 3, objects: vec![ObjectTarget { kind: ObjectKind::Text, index: 0 }], offset: [1.0, 0.0] };

        assert_eq!(content_page(&moved), Some(3));

        assert_eq!(content_page(&Edit::RotatePages { pages: vec![0], degrees: 90 }), None);

        assert_eq!(content_page(&Edit::SetInfo { key: "Title".into(), value: "x".into() }), None);

    }

}

