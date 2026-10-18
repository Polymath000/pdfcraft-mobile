
use std::path::PathBuf;

use egui::{Align, Color32, CornerRadius, Layout, Pos2, Rect, Stroke, vec2};
use pdfcraft_engine::sign::{self, Appearance, AppearanceLabels, Certificate, DigitalId, Name, PrivateKey};
use pdfcraft_engine::{SignOptions, SignatureImage, SignatureInfo, SignatureStatus};

use crate::canvas::{DocView, PageXform, Snapshot, SnapshotKind};
use crate::fill_sign::SavedSig;
use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, icons, widgets};
use crate::rfd;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DigitalIdEntry {
    pub path: String,
    pub name: String,
    pub issuer: String,
    pub email: String,
    pub expires: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unusable: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignStep {
    Choose,
    Configure,
    SignAs,
}

pub const KEY_ALGORITHMS: [(&str, &str); 4] =
    [("2048-bit RSA", "rsa2048"), ("3072-bit RSA", "rsa3072"), ("4096-bit RSA", "rsa4096"), ("256-bit ECDSA (P-256)", "p256")];

#[derive(Clone, Debug, PartialEq)]
pub struct NewIdDraft {
    pub create: bool,
    pub name: String,
    pub unit: String,
    pub organization: String,
    pub email: String,
    pub country: String,
    pub key: usize,
    pub password: String,
    pub confirm: String,
    pub file: String,
    pub file_password: String,
}

impl Default for NewIdDraft {
    fn default() -> Self {
        Self {
            create: true,
            name: String::new(),
            unit: String::new(),
            organization: String::new(),
            email: String::new(),
            country: String::new(),
            key: 0,
            password: String::new(),
            confirm: String::new(),
            file: String::new(),
            file_password: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SignGraphic {
    #[default]
    Name,
    FillSign,
    Image(#[serde(with = "crate::fill_sign::image_data")] SignatureImage),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SignDraft {
    pub page: usize,
    pub rect: Option<[f64; 4]>,
    pub field: Option<String>,
    pub certify: Option<u8>,
    pub step: SignStep,
    pub selected: Option<usize>,
    pub password: String,
    pub reason: String,
    pub location: String,
    pub appearance: Appearance,
    pub graphic: SignGraphic,
    pub image: Option<SignatureImage>,
    pub new_id: NewIdDraft,
    pub error: Option<String>,
    pub confirm_remove: Option<usize>,
}

impl SignDraft {
    fn new(page: usize, rect: Option<[f64; 4]>, field: Option<String>, certify: Option<u8>, ids: &[DigitalIdEntry]) -> Self {
        Self {
            page,
            rect,
            field,
            certify,
            step: if ids.is_empty() { SignStep::Configure } else { SignStep::Choose },
            selected: ids.iter().position(|e| e.unusable.is_none()),
            password: String::new(),
            reason: String::new(),
            location: String::new(),
            appearance: Appearance::default(),
            graphic: SignGraphic::Name,
            image: None,
            new_id: NewIdDraft::default(),
            error: None,
            confirm_remove: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignView {
    pub drag: Option<(usize, Pos2)>,
    pub drawn: Option<(usize, [f64; 4])>,
    pub field: Option<String>,
}

pub(crate) fn page_input(ui: &egui::Ui, resp: &egui::Response, xf: &PageXform, page: usize, info: &pdfcraft_render::DocInfo, view: &mut DocView) {
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let origin = ui.input(|i| i.pointer.press_origin());
    if pointer.is_some_and(|p| xf.rect.contains(p)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if resp.drag_started()
        && let Some(o) = origin.filter(|o| xf.rect.contains(*o))
    {
        view.sign.drag = Some((page, o));
    }
    if let Some((p, start)) = view.sign.drag
        && p == page
        && let Some(end) = pointer
    {
        let end = Pos2::new(end.x.clamp(xf.rect.left(), xf.rect.right()), end.y.clamp(xf.rect.top(), xf.rect.bottom()));
        let r = Rect::from_two_pos(start, end);
        ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(0x14, 0x73, 0xE6, 28));
        ui.painter().rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(0x14, 0x73, 0xE6)), egui::StrokeKind::Middle);
        if resp.drag_stopped() {
            view.sign.drag = None;
            if r.width() >= 8.0 && r.height() >= 8.0 {
                let user = |q: Pos2| {
                    let (vx, vy) = xf.screen_to_view(q);
                    info.pages[page].view_to_user(vx, vy)
                };
                let (a, b) = (user(r.left_top()), user(r.right_bottom()));
                let rect = [a[0].min(b[0]) as f64, a[1].min(b[1]) as f64, a[0].max(b[0]) as f64, a[1].max(b[1]) as f64];
                view.sign.drawn = Some((page, rect));
            }
        }
    }
}

fn id_dir() -> Option<PathBuf> {
    crate::recovery::RecoveryStore::default_dir().and_then(|d| d.parent().map(|p| p.join("Digital IDs")))
}

fn save_new_id_file(dir: &std::path::Path, stem: &str, p12: &[u8]) -> std::io::Result<PathBuf> {
    use std::io::{ErrorKind, Write};
    for i in 1..=10_000u32 {
        let path = dir.join(if i == 1 { format!("{stem}.p12") } else { format!("{stem} {i}.p12") });
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let file = match opts.open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) if e.kind() == ErrorKind::PermissionDenied && path.symlink_metadata().is_ok() => continue,
            Err(e) => return Err(e),
        };
        let written = {
            let mut file = file;
            file.write_all(p12).and_then(|()| file.sync_all())
        };
        if let Err(e) = written {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        return Ok(path);
    }
    Err(std::io::Error::new(ErrorKind::AlreadyExists, "no free file name for the digital ID"))
}

fn keychain_id(reference: &str) -> Result<DigitalId, String> {
    #[cfg(target_os = "macos")]
    return sign::keychain::find(reference).map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    Err(format!("{reference}: Keychain identities are only available on macOS"))
}

fn windows_id(reference: &str) -> Result<DigitalId, String> {
    #[cfg(target_os = "windows")]
    return sign::windows::find(reference).map_err(|e| e.to_string());
    #[cfg(not(target_os = "windows"))]
    Err(format!("{reference}: Windows certificate store identities are only available on Windows"))
}

pub fn entry_for(path: &str, c: &Certificate) -> DigitalIdEntry {
    DigitalIdEntry {
        path: path.to_string(),
        name: c.display_name(),
        issuer: c.issuer.common_name().map(str::to_string).unwrap_or_else(|| c.issuer.display()),
        email: c.subject.email().unwrap_or("").to_string(),
        expires: format!("{:04}.{:02}.{:02}", c.not_after.year, c.not_after.month, c.not_after.day),
        unusable: None,
    }
}

#[cfg(target_os = "windows")]
fn entry_unusable(u: &sign::windows::Unusable) -> DigitalIdEntry {
    DigitalIdEntry {
        path: format!("windows:{}", u.fingerprint.split(' ').collect::<String>().to_ascii_lowercase()),
        name: u.subject.clone(),
        issuer: String::new(),
        email: String::new(),
        expires: String::new(),
        unusable: Some(u.reason.clone()),
    }
}

impl PdfCraftApp {
    pub(crate) fn export_certificate(&mut self, c: &Certificate) {
        let name = format!("{}.cer", crate::files::attachment_file_name(&c.display_name()));
        self.write_files(&[(name, std::sync::Arc::new(certificate_pem(c).into_bytes()))], "Export certificate");
    }

    pub fn start_signing(&mut self, page: usize, rect: Option<[f64; 4]>, field: Option<String>, certify: Option<u8>) {
        self.refresh_os_key_store_ids();
        let mut draft = SignDraft::new(page, rect, field, certify, &self.digital_ids);
        draft.graphic = match &self.sign_graphic {
            SignGraphic::FillSign if self.signature.is_none() => SignGraphic::Name,
            g => g.clone(),
        };
        if let SignGraphic::Image(image) = &self.sign_graphic {
            draft.image = Some(image.clone());
        }
        self.sign_draft = Some(draft);
        self.dialog = Some(crate::Dialog::Sign);
    }

    fn refresh_os_key_store_ids(&mut self) {
        self.digital_ids.retain(|e| !e.path.starts_with("keychain:") && !e.path.starts_with("windows:"));
        #[cfg(target_os = "macos")]
        if self.os_key_store_ids {
            match sign::keychain::identities(None) {
                Ok(ids) => {
                    for id in ids {
                        let mut entry = entry_for(&sign::keychain::reference(&id.certificate), &id.certificate);
                        if let Some(name) = id.friendly_name {
                            entry.name = name;
                        }
                        self.digital_ids.push(entry);
                    }
                }
                Err(e) => self.notify_fmt("The Keychain's digital IDs couldn't be listed: {e}", &[("e", &e.to_string())]),
            }
        }
        #[cfg(target_os = "windows")]
        if self.os_key_store_ids {
            match sign::windows::list() {
                Ok(listing) => {
                    for id in &listing.ids {
                        self.digital_ids.push(entry_for(&sign::windows::reference(&id.certificate), &id.certificate));
                    }
                    for u in listing.unusable.iter().filter(|u| !u.no_private_key) {
                        self.digital_ids.push(entry_unusable(u));
                    }
                }
                Err(e) => self.notify_fmt("The Windows store's digital IDs couldn't be listed: {e}", &[("e", &e.to_string())]),
            }
        }
    }

    pub fn add_digital_id(&mut self, path: &str, cert: &Certificate) -> usize {
        if let Some(i) = self.digital_ids.iter().position(|e| e.path == path) {
            self.digital_ids[i] = entry_for(path, cert);
            return i;
        }
        self.digital_ids.push(entry_for(path, cert));
        self.digital_ids.len() - 1
    }

    pub fn remove_digital_id(&mut self, index: usize) -> bool {
        if self.digital_ids.get(index).is_none_or(|e| e.path.starts_with("keychain:") || e.path.starts_with("windows:")) {
            return false;
        }
        self.digital_ids.remove(index);
        if let Some(d) = self.sign_draft.as_mut() {
            d.confirm_remove = None;
            d.selected = match d.selected {
                Some(s) if s > index => Some(s - 1),
                Some(s) if s == index => self.digital_ids.iter().position(|e| e.unusable.is_none()),
                s => s,
            };
            if self.digital_ids.is_empty() {
                d.step = SignStep::Configure;
            }
        }
        true
    }

    fn create_digital_id(&mut self) -> Result<usize, String> {
        let Some(d) = self.sign_draft.as_ref().map(|d| d.new_id.clone()) else { return Err("nothing to create".into()) };
        if d.name.trim().is_empty() {
            return Err("Enter a name.".into());
        }
        const _: () = assert!(sign::pkcs12::MIN_PASSWORD_CHARS == 8);
        if d.password.chars().count() < sign::pkcs12::MIN_PASSWORD_CHARS {
            return Err("The password must have at least 8 characters.".into());
        }
        if d.password != d.confirm {
            return Err("The passwords do not match.".into());
        }
        let key = match KEY_ALGORITHMS[d.key.min(KEY_ALGORITHMS.len() - 1)].1 {
            "rsa3072" => PrivateKey::generate_rsa(3072),
            "rsa4096" => PrivateKey::generate_rsa(4096),
            "p256" => PrivateKey::generate_p256(),
            _ => PrivateKey::generate_rsa(2048),
        }
        .map_err(|e| e.to_string())?;
        let now = self.session.now_secs();
        let dn = Name::build(&d.name, &d.unit, &d.organization, &d.email, &d.country);
        let serial = sign::keys::DigestAlg::Sha256.digest(&[d.name.as_bytes(), &now.to_be_bytes()])[..8].to_vec();
        let cert = Certificate::self_signed(&dn, &key, sign::Time::from_unix(now), 5, &serial).map_err(|e| e.to_string())?;
        let id = DigitalId { key, certificate: cert.clone(), chain: Vec::new(), friendly_name: Some(d.name.trim().to_string()) };
        let p12 = sign::pkcs12::write(&id, &d.password).map_err(|e| e.to_string())?;
        let dir = match &self.export_dir_override {
            Some(p) => Some(PathBuf::from(p)),
            None => id_dir(),
        }
        .ok_or("There is no folder to save the digital ID in.")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stem: String = d.name.trim().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
        let path = save_new_id_file(&dir, &stem, &p12).map_err(|e| e.to_string())?;
        Ok(self.add_digital_id(&path.to_string_lossy(), &cert))
    }

    fn import_digital_id(&mut self) -> Result<usize, String> {
        let Some(d) = self.sign_draft.as_ref().map(|d| d.new_id.clone()) else { return Err("nothing to import".into()) };
        let bytes = std::fs::read(&d.file).map_err(|e| format!("{}: {e}", d.file))?;
        let id = sign::pkcs12::open(&bytes, &d.file_password).map_err(|e| match e {
            sign::SignError::WrongPassword => "The password is incorrect.".to_string(),
            e => e.to_string(),
        })?;
        Ok(self.add_digital_id(&d.file, &id.certificate))
    }

    fn finish_signing(&mut self) -> Result<(), String> {
        if !self.commit_form_typing() {
            return Err(String::new());
        }
        let Some((_, doc_id)) = self.active_ids() else { return Err("no document".into()) };
        let d = self.sign_draft.clone().ok_or("nothing to sign")?;
        let entry = d.selected.and_then(|i| self.digital_ids.get(i)).filter(|e| e.unusable.is_none()).cloned().ok_or("Choose a digital ID.")?;
        let id = if entry.path.starts_with("keychain:") {
            keychain_id(&entry.path)?
        } else if entry.path.starts_with("windows:") {
            windows_id(&entry.path)?
        } else {
            let bytes = std::fs::read(&entry.path).map_err(|e| format!("{}: {e}", entry.path))?;
            sign::pkcs12::open(&bytes, &d.password).map_err(|e| match e {
                sign::SignError::WrongPassword => "The password is incorrect.".to_string(),
                e => e.to_string(),
            })?
        };
        let some = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
        let graphic = match self.graphic_sig(&d.graphic) {
            Some(saved) if d.rect.is_some() || d.field.is_some() => Some(saved.graphic().ok_or("The saved signature has nothing to draw.")?),
            _ => None,
        };
        let choice = d.graphic.clone();
        let opts = SignOptions {
            field: d.field.clone(),
            page: d.page,
            rect: d.rect,
            reason: some(&d.reason),
            location: some(&d.location),
            certify: d.certify,
            appearance: d.appearance.clone(),
            appearance_labels: appearance_labels(crate::i18n::current()).0,
            graphic,
            ..SignOptions::default()
        };
        let name = self.session.get(doc_id).map(|d| d.name.clone()).unwrap_or_default();
        let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string();
        let sign_and_save = move |app: &mut Self, path: PathBuf| -> Result<(), String> {
            if !app.commit_typing_in(doc_id) {
                return Err(String::new());
            }
            let signed = app.session.sign(doc_id, &id, opts).map_err(|e| e.to_string())?;
            crate::editing::write_atomically(&path.to_string_lossy(), signed.as_slice()).map_err(|e| format!("Could not save: {e}"))?;
            #[cfg(not(target_arch = "wasm32"))]
            app.watch.known_now(doc_id, &path.to_string_lossy());
            app.session.mark_signed(doc_id, signed, Some(path.to_string_lossy().into_owned())).map_err(|e| e.to_string())?;
            app.sign_graphic = choice;
            if let Some(view) = app.views.iter_mut().find(|v| v.id == doc_id) {
                view.invalidate_content();
            }
            app.right = Some(crate::RightPanel::Signatures);
            app.notify_fmt("Signed and saved to {path}", &[("path", &path.display().to_string())]);
            Ok(())
        };
        match self.save_override.clone() {
            Some(p) => sign_and_save(self, PathBuf::from(p)),
            #[cfg(not(target_arch = "wasm32"))]
            None => {
                let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem}_signed.pdf"));
                let asked = self.ask_one(crate::pickers::Ask::Save(dialog), Some(doc_id), move |app, path| {
                    if let Err(e) = sign_and_save(app, path)
                        && !e.is_empty()
                    {
                        app.notify(e);
                    }
                });
                if asked { Ok(()) } else { Err(String::new()) }
            }
            #[cfg(target_arch = "wasm32")]
            None => {
                let _ = (stem, sign_and_save);
                Err(String::new())
            }
        }
    }

    fn graphic_sig(&self, g: &SignGraphic) -> Option<SavedSig> {
        graphic_of(self.signature.as_ref(), g)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pick_sign_graphic_image(&mut self) {
        let epoch = self.dialog_epoch();
        let dialog = rfd::AsyncFileDialog::new().add_filter(tl!("Image"), &["png", "jpg", "jpeg"]);
        self.ask_one(crate::pickers::Ask::File(dialog), None, move |app, path| {
            if app.dialog_epoch() != epoch || app.dialog != Some(crate::Dialog::Sign) {
                return;
            }
            let image = std::fs::File::open(path)
                .map_err(pdfcraft_engine::signature_image::SignatureImageError::from)
                .and_then(SignatureImage::read)
                .and_then(|image| image.remove_white_background(245, 35));
            match (image, app.sign_draft.as_mut()) {
                (Ok(image), Some(d)) => {
                    d.graphic = SignGraphic::Image(image.clone());
                    d.image = Some(image);
                    d.error = None;
                }
                (Ok(_), None) => {}
                (Err(e), _) => app.notify_fmt("Couldn't import the signature image: {e}", &[("e", &e.to_string())]),
            }
        });
    }

    pub fn trust_certificate(&mut self, cert: Certificate) {
        let mut certs = self.session.trusted_certificates().to_vec();
        if !certs.iter().any(|c| c.raw == cert.raw) {
            certs.push(cert);
        }
        self.apply_trusted_certificates(certs);
    }

    pub fn apply_trusted_certificates(&mut self, certs: Vec<Certificate>) {
        self.session.set_trusted_certificates(certs);
        for view in &mut self.views {
            view.invalidate_content();
        }
    }

    fn file_stem(name: &str) -> String {
        name.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string()
    }

    pub fn open_revision(&mut self, n: usize) {
        let Some((_, id)) = self.active_ids() else { return };
        let Some(doc) = self.session.get(id) else { return };
        let ends = doc.revision_ends();
        if let Some(&end) = n.checked_sub(1).and_then(|k| ends.get(k)) {
            let end = end.min(doc.bytes.len());
            if let Some(index) = self.find_snapshot(SnapshotKind::Revision, &doc.name, &doc.bytes[..end]) {
                self.active = Some(index);
                return;
            }
        }
        let from = doc.name.clone();
        match self.session.open_revision(id, n) {
            Ok(new) => {
                self.session.set_reading_colors(new, self.reading_colors);
                let Some(doc) = self.session.get(new) else { return };
                let mut view = DocView::new(new, &doc.info, self.view_defaults);
                view.snapshot = Some(Snapshot { kind: SnapshotKind::Revision, revision: n, from });
                self.views.push(view);
                self.active = Some(self.views.len() - 1);
            }
            Err(e) => self.notify_fmt("Couldn't open revision {n}: {e}", &[("n", &n.to_string()), ("e", &e.to_string())]),
        }
    }

    fn find_snapshot(&self, kind: SnapshotKind, from: &str, bytes: &[u8]) -> Option<usize> {
        let mut fallback = None;
        for (i, v) in self.views.iter().enumerate() {
            let same_kind = v.snapshot.as_ref().is_some_and(|s| s.kind == kind);
            let same_bytes = same_kind && self.session.get(v.id).is_some_and(|d| d.bytes.as_slice() == bytes);
            if !same_bytes {
                continue;
            }
            if v.snapshot.as_ref().is_some_and(|s| s.from == from) {
                return Some(i);
            }
            fallback.get_or_insert(i);
        }
        fallback
    }

    pub fn view_signed_version(&mut self, len: usize, revision: usize) {
        let Some((_, id)) = self.active_ids() else { return };
        let Some(doc) = self.session.get(id) else { return };
        let end = len.min(doc.bytes.len());
        if let Some(index) = self.find_snapshot(SnapshotKind::SignedVersion, &doc.name, &doc.bytes[..end]) {
            self.active = Some(index);
            return;
        }
        let from = doc.name.clone();
        let bytes = doc.bytes[..end].to_vec();
        let name = format!("{} (signed version)", Self::file_stem(&from));
        match self.session.open(format!("{name}.pdf"), None, std::sync::Arc::new(bytes), doc.password.clone().as_deref()) {
            Ok(new) => {
                self.session.set_reading_colors(new, self.reading_colors);
                let Some(doc) = self.session.get(new) else { return };
                let mut view = DocView::new(new, &doc.info, self.view_defaults);
                view.snapshot = Some(Snapshot { kind: SnapshotKind::SignedVersion, revision, from });
                self.views.push(view);
                self.active = Some(self.views.len() - 1);
            }
            Err(e) => self.notify_fmt("Couldn't open the signed version: {e}", &[("e", &e.to_string())]),
        }
    }
}

pub(crate) fn dialog(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    let Some(step) = app.sign_draft.as_ref().map(|d| d.step) else { return true };
    match step {
        SignStep::Choose => choose(ui, app, t),
        SignStep::Configure => configure(ui, app, t),
        SignStep::SignAs => sign_as(ui, app, t),
    }
}

fn title(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(tl!(text)).font(theme::semibold(18.0)));
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);
}

fn error(ui: &mut egui::Ui, err: &Option<String>) {
    if let Some(e) = err.as_ref().filter(|e| !e.is_empty()) {
        ui.colored_label(Color32::from_rgb(0xD7, 0x37, 0x3F), tl!(e));
        ui.add_space(4.0);
    }
}

fn choose(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    title(ui, "Sign with a Digital ID");
    ui.label(tl!("Choose the digital ID that you want to use for signing:"));
    ui.add_space(6.0);
    let ids = app.digital_ids.clone();
    let Some(d) = app.sign_draft.as_mut() else { return true };
    let mut close = false;
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        for (i, e) in ids.iter().enumerate() {
            let selected = d.selected == Some(i);
            let usable = e.unusable.is_none();
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), egui::Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, usable, selected, &e.name));
            let fill = if selected {
                t.accent_soft
            } else if resp.hovered() && usable {
                t.hover
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
            let (icon, name_color) = if usable { (t.accent, t.text) } else { (t.text_muted, t.text_muted) };
            icons::paint(ui, Rect::from_min_size(rect.min + vec2(10.0, 15.0), vec2(20.0, 20.0)), "badge-check", 18.0, icon);
            let line = |text: &str, font: egui::FontId, color: Color32| {
                let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
                job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - 50.0).max(0.0));
                ui.painter().layout_job(job)
            };
            let name = line(&e.name, theme::semibold(13.0), name_color);
            let store = if e.path.starts_with("keychain:") {
                tl!("Keychain  ·  ").to_string()
            } else if e.path.starts_with("windows:") {
                tl!("Windows store  ·  ").to_string()
            } else {
                String::new()
            };
            let sub = match &e.unusable {
                Some(reason) => format!("{store}{reason}"),
                None => format!(
                    "{store}{email}{issued}{issuer}{expires}{date}",
                    email = if e.email.is_empty() { String::new() } else { format!("{}  ·  ", e.email) },
                    issued = tl!("Issued by: "),
                    issuer = e.issuer,
                    expires = tl!(", Expires: "),
                    date = e.expires,
                ),
            };
            let details = line(&sub, theme::regular(11.5), t.text_muted);
            let elided = name.elided || details.elided;
            ui.painter().galley(rect.min + vec2(40.0, 9.0), name, name_color);
            ui.painter().galley(rect.min + vec2(40.0, 28.0), details, t.text_muted);
            let resp = if elided { resp.on_hover_text(format!("{}\n{sub}", e.name)) } else { resp };
            if !usable {
                continue;
            }
            if resp.clicked() {
                d.selected = Some(i);
                d.confirm_remove = None;
            }
            if resp.double_clicked() {
                d.step = SignStep::SignAs;
            }
        }
        if ids.is_empty() {
            ui.label(egui::RichText::new(tl!("No digital IDs yet.")).color(t.text_muted));
        }
    });
    ui.add_space(10.0);
    let mut remove = None;
    if let Some(e) = d.confirm_remove.and_then(|i| ids.get(i)) {
        ui.label(crate::i18n::fmt(tl!("Remove \"{name}\" from this list? The file itself is not deleted."), &[("name", &e.name)]));
        ui.horizontal(|ui| {
            if widgets::pill_button(ui, tl!("Remove"), true).clicked() {
                remove = d.confirm_remove;
            }
            if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
                d.confirm_remove = None;
            }
        });
        ui.add_space(10.0);
    }
    let removable = d.selected.filter(|&i| ids.get(i).is_some_and(|e| !e.path.starts_with("keychain:") && !e.path.starts_with("windows:")));
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, tl!("Configure New Digital ID"), false).clicked() {
            d.step = SignStep::Configure;
            d.error = None;
        }
        if d.confirm_remove.is_none() && ui.add_enabled_ui(removable.is_some(), |ui| widgets::pill_button(ui, tl!("Remove"), false)).inner.clicked() {
            d.confirm_remove = removable;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled_ui(d.selected.is_some(), |ui| widgets::pill_button(ui, tl!("Continue"), true)).inner.clicked() {
                d.step = SignStep::SignAs;
                d.error = None;
            }
            if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
                close = true;
            }
        });
    });
    if let Some(i) = remove {
        app.remove_digital_id(i);
    }
    close
}

fn configure(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    title(ui, "Configure a Digital ID for Signing");
    let Some(d) = app.sign_draft.as_mut() else { return true };
    let mut close = false;
    let mut go = false;
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut browse = false;
    widgets::radio_value(ui, &mut d.new_id.create, false, tl!("Use a Digital ID from a file"));
    widgets::radio_value(ui, &mut d.new_id.create, true, tl!("Create a new Digital ID (self-signed, saved to a password-protected file)"));
    ui.add_space(8.0);
    let n = &mut d.new_id;
    if n.create {
        egui::Grid::new("new-id").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            for (label, value) in [
                (tl!("Name"), &mut n.name),
                (tl!("Organizational Unit"), &mut n.unit),
                (tl!("Organization Name"), &mut n.organization),
                (tl!("Email Address"), &mut n.email),
            ] {
                let l = ui.label(label);
                ui.add(crate::widgets::line(value).desired_width(260.0)).labelled_by(l.id);
                ui.end_row();
            }
            let l = ui.label(tl!("Country/Region"));
            ui.add(crate::widgets::line(&mut n.country).hint_text("US").char_limit(2).desired_width(60.0)).labelled_by(l.id);
            ui.end_row();
            ui.label(tl!("Key Algorithm"));
            egui::ComboBox::from_id_salt("key-alg").selected_text(KEY_ALGORITHMS[n.key].0).show_ui(ui, |ui| {
                for (i, (label, _)) in KEY_ALGORITHMS.iter().enumerate() {
                    ui.selectable_value(&mut n.key, i, *label);
                }
            });
            ui.end_row();
            let l = ui.label(tl!("Password"));
            ui.add(crate::widgets::line(&mut n.password).password(true).desired_width(200.0)).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Confirm Password"));
            ui.add(crate::widgets::line(&mut n.confirm).password(true).desired_width(200.0)).labelled_by(l.id);
            ui.end_row();
        });
        ui.label(
            egui::RichText::new(tl!("Valid for 5 years, for digital signatures. Self-signed IDs are usually not trusted by others."))
                .small()
                .color(t.text_muted),
        );
    } else {
        egui::Grid::new("id-file").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            let l = ui.label(tl!("File (.p12, .pfx)"));
            ui.horizontal(|ui| {
                ui.add(crate::widgets::line(&mut n.file).desired_width(220.0)).labelled_by(l.id);
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button(tl!("Browse…")).clicked() {
                    browse = true;
                }
            });
            ui.end_row();
            let l = ui.label(tl!("Password"));
            ui.add(crate::widgets::line(&mut n.file_password).password(true).desired_width(200.0)).labelled_by(l.id);
            ui.end_row();
        });
    }
    ui.add_space(6.0);
    error(ui, &d.error);
    ui.horizontal(|ui| {
        if !app.digital_ids.is_empty() && widgets::pill_button(ui, tl!("Back"), false).clicked() {
            d.step = SignStep::Choose;
            d.error = None;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, if d.new_id.create { tl!("Save") } else { tl!("Continue") }, true).clicked() {
                go = true;
            }
            if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
                close = true;
            }
        });
    });
    if go {
        let create = app.sign_draft.as_ref().is_some_and(|d| d.new_id.create);
        let result = if create { app.create_digital_id() } else { app.import_digital_id() };
        let Some(d) = app.sign_draft.as_mut() else { return true };
        match result {
            Ok(i) => {
                d.selected = Some(i);
                d.step = SignStep::Choose;
                d.error = None;
                d.new_id = NewIdDraft::default();
            }
            Err(e) => d.error = Some(e),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if browse {
        let dialog = rfd::AsyncFileDialog::new().add_filter(tl!("Digital ID"), &["p12", "pfx"]);
        let epoch = app.dialog_epoch();
        app.ask_one(crate::pickers::Ask::File(dialog), None, move |app, p| {
            if app.dialog_epoch() == epoch
                && let Some(d) = app.sign_draft.as_mut()
            {
                d.new_id.file = p.to_string_lossy().into_owned();
            }
        });
    }
    #[cfg(target_arch = "wasm32")]
    let _ = browse;
    close
}

fn graphic_of(signature: Option<&SavedSig>, g: &SignGraphic) -> Option<SavedSig> {
    match g {
        SignGraphic::Name => None,
        SignGraphic::FillSign => signature.cloned(),
        SignGraphic::Image(image) => Some(SavedSig::Image(image.clone())),
    }
}

pub(crate) fn appearance_labels(lang: crate::i18n::Lang) -> (AppearanceLabels, bool) {
    let english = AppearanceLabels::default();
    if lang == crate::i18n::Lang::EN {
        return (english, false);
    }
    let said = |s: &str| (!lang.rtl() && crate::i18n::has(lang, s)).then(|| crate::i18n::tr(lang, s).to_string());
    let translated = (|| {
        Some(AppearanceLabels {
            signed_by: said(&english.signed_by)?,
            distinguished_name: said(&english.distinguished_name)?,
            reason: said(&english.reason)?,
            location: said(&english.location)?,
            date: said(&english.date)?,
        })
    })();
    match translated.filter(|l| l.first_undrawable().is_none()) {
        Some(labels) => (labels, false),
        None => (english, true),
    }
}

fn detail_lines(a: &Appearance, labels: &AppearanceLabels, name: &str, reason: &str, location: &str, date: &str) -> Vec<String> {
    let label = |l: &str, v: &str| if a.labels { format!("{l}{v}") } else { v.to_string() };
    let mut lines = Vec::new();
    if a.name {
        if a.labels {
            lines.push(labels.signed_by.clone());
        }
        lines.push(name.to_string());
    }
    if a.reason && !reason.trim().is_empty() {
        lines.push(label(&labels.reason, reason.trim()));
    }
    if a.location && !location.trim().is_empty() {
        lines.push(label(&labels.location, location.trim()));
    }
    if a.date {
        lines.push(label(&labels.date, date));
    }
    lines.retain(|l| !l.is_empty());
    lines
}

fn preview(
    ui: &mut egui::Ui,
    t: &Tokens,
    name: &str,
    a: &Appearance,
    lines: &[String],
    graphic: Option<&SavedSig>,
    cache: &mut Option<(SavedSig, egui::TextureHandle)>,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), egui::Sense::hover());
    let bg = if t.dark() { Color32::from_gray(0xF4) } else { Color32::WHITE };
    ui.painter().rect(rect, CornerRadius::same(4), bg, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let inner = rect.shrink(10.0);
    let alone = graphic.is_some() && lines.is_empty();
    let half = Rect::from_min_size(inner.min, vec2(inner.width() * 0.5 - 6.0, inner.height()));
    if let Some(g) = graphic {
        crate::fill_sign::paint_saved(ui, if alone { inner } else { half }, g, cache, "sign-graphic");
    } else if a.name {
        let galley = ui.painter().layout(name.to_string(), theme::regular(22.0), Color32::BLACK, half.width());
        ui.painter().galley(half.left_center() - vec2(0.0, galley.size().y / 2.0), galley, Color32::BLACK);
    }
    let x = if (a.name || graphic.is_some()) && !alone { inner.center().x + 6.0 } else { inner.left() };
    let galley = ui.painter().layout(lines.join("\n"), theme::regular(11.0), Color32::from_gray(0x20), inner.right() - x);
    ui.painter().galley(egui::pos2(x, inner.center().y - galley.size().y / 2.0), galley, Color32::BLACK);
}

fn sign_as(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    let ids = app.digital_ids.clone();
    let date = sign::pdf::display_date(&app.session.signing_date());
    let Some(d) = app.sign_draft.as_mut() else { return true };
    let Some(entry) = d.selected.and_then(|i| ids.get(i)).cloned() else {
        d.step = SignStep::Choose;
        return false;
    };
    title(
        ui,
        &crate::i18n::fmt(
            tl!("{verb} as \"{name}\""),
            &[("verb", if d.certify.is_some() { tl!("Certify") } else { tl!("Sign") }), ("name", &entry.name)],
        ),
    );
    let in_os_key_store = entry.path.starts_with("keychain:") || entry.path.starts_with("windows:");
    let mut close = false;
    let mut go = false;
    #[cfg(not(target_arch = "wasm32"))]
    let mut pick = false;
    if d.rect.is_some() || d.field.is_some() {
        ui.label(egui::RichText::new(tl!("Appearance")).font(theme::semibold(12.5)));
        let graphic = graphic_of(app.signature.as_ref(), &d.graphic);
        let (labels, in_english) = appearance_labels(crate::i18n::current());
        let lines = detail_lines(&d.appearance, &labels, &entry.name, &d.reason, &d.location, &date);
        preview(ui, t, &entry.name, &d.appearance, &lines, graphic.as_ref(), &mut app.sign_graphic_preview);
        if in_english && d.appearance.labels {
            ui.label(
                egui::RichText::new(tl!("The labels are in English: the signature's font can't draw them in this language."))
                    .small()
                    .color(t.text_muted),
            );
        }
        ui.horizontal_wrapped(|ui| {
            let a = &mut d.appearance;
            ui.add(widgets::checkbox(&mut a.name, tl!("Name")));
            ui.add(widgets::checkbox(&mut a.date, tl!("Date")));
            ui.add(widgets::checkbox(&mut a.reason, tl!("Reason")));
            ui.add(widgets::checkbox(&mut a.location, tl!("Location")));
            ui.add(widgets::checkbox(&mut a.distinguished_name, tl!("Distinguished name")));
            ui.add(widgets::checkbox(&mut a.labels, tl!("Labels")));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tl!("Graphic"));
            widgets::radio_value(ui, &mut d.graphic, SignGraphic::Name, tl!("Signer's name"));
            ui.add_enabled_ui(app.signature.is_some(), |ui| {
                if ui.add(widgets::radio(d.graphic == SignGraphic::FillSign, tl!("Fill & Sign signature"))).clicked() {
                    d.graphic = SignGraphic::FillSign;
                }
            });
            ui.add_enabled_ui(d.image.is_some(), |ui| {
                if ui.add(widgets::radio(matches!(d.graphic, SignGraphic::Image(_)), tl!("Image"))).clicked()
                    && let Some(image) = &d.image
                {
                    d.graphic = SignGraphic::Image(image.clone());
                }
            });
            #[cfg(not(target_arch = "wasm32"))]
            if widgets::pill_button(ui, tl!("Choose image…"), false).clicked() {
                pick = true;
            }
        });
        ui.add_space(6.0);
    } else {
        ui.label(egui::RichText::new(tl!("The signature will be invisible.")).color(t.text_muted));
    }
    egui::Grid::new("sign-as").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        if d.certify.is_some() {
            ui.label(tl!("Permitted actions after certifying"));
            let mut p = d.certify.unwrap_or(2);
            let label = |p: u8| match p {
                1 => tl!("No changes allowed"),
                3 => tl!("Annotations, form fill-in, and digital signatures"),
                _ => tl!("Form fill-in and digital signatures"),
            };
            egui::ComboBox::from_id_salt("certify-p").selected_text(label(p)).width(300.0).show_ui(ui, |ui| {
                for v in [1u8, 2, 3] {
                    ui.selectable_value(&mut p, v, label(v));
                }
            });
            d.certify = Some(p);
            ui.end_row();
        }
        let l = ui.label(tl!("Reason"));
        ui.add(crate::widgets::line(&mut d.reason).hint_text(tl!("Optional")).desired_width(260.0)).labelled_by(l.id);
        ui.end_row();
        let l = ui.label(tl!("Location"));
        ui.add(crate::widgets::line(&mut d.location).hint_text(tl!("Optional")).desired_width(260.0)).labelled_by(l.id);
        ui.end_row();
        if in_os_key_store {
            ui.label("");
            ui.label(
                egui::RichText::new(if entry.path.starts_with("windows:") {
                    tl!("The key is in the Windows certificate store, which may ask to allow PdfCraft to use it.")
                } else {
                    tl!("The key is in the macOS Keychain, which may ask to allow PdfCraft to use it.")
                })
                .small()
                .color(t.text_muted),
            );
        } else {
            let l = ui.label(tl!("Digital ID password"));
            let r = ui.add(crate::widgets::line(&mut d.password).password(true).desired_width(200.0)).labelled_by(l.id);
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                go = true;
            }
        }
        ui.end_row();
    });
    ui.add_space(6.0);
    error(ui, &d.error);
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, tl!("Back"), false).clicked() {
            d.step = SignStep::Choose;
            d.error = None;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Sign"), true).clicked() {
                go = true;
            }
            if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
                close = true;
            }
        });
    });
    #[cfg(not(target_arch = "wasm32"))]
    if pick {
        app.pick_sign_graphic_image();
    }
    if go {
        match app.finish_signing() {
            Ok(()) => {
                app.sign_draft = None;
                return true;
            }
            Err(e) => {
                if let Some(d) = app.sign_draft.as_mut() {
                    d.error = Some(e);
                }
            }
        }
    }
    close
}

pub(crate) fn banner(sigs: &[SignatureInfo]) -> Option<(&'static str, Color32, &'static str, String)> {
    let signed: Vec<&SignatureInfo> = sigs.iter().filter(|s| s.signed).collect();
    if signed.is_empty() {
        return None;
    }
    if let Some(c) = signed.iter().find(|s| s.certify.is_some())
        && c.status == SignatureStatus::Valid
    {
        let by = c.certificate.as_ref().map(|x| {
            let org = x.subject.organization().map(|o| format!(", {o}")).unwrap_or_default();
            let who = format!("{}{org}", x.display_name());
            let (template, issuer) = match x.issuer.common_name() {
                Some(issuer) => ("{who}, certificate issued by {issuer}.", issuer),
                None => ("{who}, certificate issued by an unknown issuer.", ""),
            };
            crate::i18n::fmt(tl!(template), &[("who", &who), ("issuer", issuer)])
        });
        return Some(("badge-check", Color32::from_rgb(0x14, 0x73, 0xE6), "Certified by {by}", by.unwrap_or_default()));
    }
    Some(if signed.iter().any(|s| s.status == SignatureStatus::Invalid) {
        ("circle-x", Color32::from_rgb(0xD7, 0x37, 0x3F), "At least one signature is invalid.", String::new())
    } else if signed.iter().all(|s| s.status == SignatureStatus::Valid) {
        ("circle-check", Color32::from_rgb(0x2D, 0x9D, 0x78), "Signed and all signatures are valid.", String::new())
    } else {
        ("triangle-alert", Color32::from_rgb(0xE6, 0x86, 0x19), "At least one signature has problems.", String::new())
    })
}

fn status_icon(s: &SignatureInfo) -> (&'static str, Color32) {
    if !s.signed {
        return ("pen-line", Color32::from_gray(0x80));
    }
    signature_status_style(s.status)
}

pub(crate) fn signature_status_style(status: SignatureStatus) -> (&'static str, Color32) {
    match status {
        SignatureStatus::Valid => ("circle-check", Color32::from_rgb(0x2D, 0x9D, 0x78)),
        SignatureStatus::Unknown => ("triangle-alert", Color32::from_rgb(0xE6, 0x86, 0x19)),
        SignatureStatus::Invalid => ("circle-x", Color32::from_rgb(0xD7, 0x37, 0x3F)),
    }
}

#[derive(Clone, Debug)]
pub enum PanelAction {
    Validate,
    GoTo(usize),
    Trust(Box<Certificate>),
    ViewSigned {
        len: usize,
        revision: usize,
    },
    Sign(String),
    ExportCertificate(Box<Certificate>),
    ViewCertificate(Vec<Certificate>),
}

pub(crate) fn panel(ui: &mut egui::Ui, t: &Tokens, sigs: &[SignatureInfo], expanded: &mut Vec<String>) -> Option<PanelAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, tl!("Validate all"), false).clicked() {
            action = Some(PanelAction::Validate);
        }
    });
    ui.add_space(6.0);
    if sigs.is_empty() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.add(icons::image("signature", 32.0, t.text_faint));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("This document has no signatures.")).color(t.text_muted));
        });
        return action;
    }
    let mut ordered: Vec<&SignatureInfo> = sigs.iter().collect();
    ordered.sort_by_key(|s| (!s.signed, s.revision));
    for s in ordered {
        let open = expanded.contains(&s.field);
        let (icon, color) = status_icon(s);
        let (template, name) = match (s.signed, s.certify.is_some(), s.signer.as_deref()) {
            (false, ..) => ("Unsigned signature field: {name}", s.field.as_str()),
            (true, true, Some(signer)) => ("Rev. {rev}: Certified by {name}", signer),
            (true, true, None) => ("Rev. {rev}: Certified by an unknown signer", ""),
            (true, false, Some(signer)) => ("Rev. {rev}: Signed by {name}", signer),
            (true, false, None) => ("Rev. {rev}: Signed by an unknown signer", ""),
        };
        let head = crate::i18n::fmt(tl!(template), &[("rev", &s.revision.to_string()), ("name", name)]);
        let resp = ui
            .horizontal(|ui| {
                let chevron = if open { "chevron-down" } else { "chevron-right" };
                let toggle = icons::button(ui, chevron, 20.0, false, if open { "Collapse" } else { "Expand" }).clicked();
                ui.add(icons::image(icon, 16.0, color));
                let l = ui.add(egui::Label::new(egui::RichText::new(&head).font(theme::semibold(12.5)).color(t.text)).sense(egui::Sense::click()));
                toggle || l.clicked()
            })
            .inner;
        if resp {
            if open {
                expanded.retain(|f| f != &s.field);
            } else {
                expanded.push(s.field.clone());
            }
        }
        if !open {
            continue;
        }
        egui::Frame::NONE.inner_margin(egui::Margin { left: 44, right: 4, top: 2, bottom: 8 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            if !s.signed {
                if widgets::pill_button(ui, tl!("Sign this field…"), true).clicked() {
                    action = Some(PanelAction::Sign(s.field.clone()));
                }
                return;
            }
            ui.label(
                egui::RichText::new(match s.status {
                    SignatureStatus::Valid => tl!("Signature is valid:"),
                    SignatureStatus::Unknown => tl!("Signature validity is UNKNOWN:"),
                    SignatureStatus::Invalid => tl!("Signature is INVALID:"),
                })
                .font(theme::semibold(12.0)),
            );
            for detail in &s.details {
                let line = detail.said(&crate::i18n::words);
                ui.add(egui::Label::new(egui::RichText::new(format!("• {line}")).font(theme::regular(11.5)).color(t.text_muted)).wrap());
            }
            ui.add_space(4.0);
            ui.label(egui::RichText::new(tl!("Signature Details")).font(theme::semibold(12.0)));
            let row = |ui: &mut egui::Ui, k: &str, v: &str| {
                ui.add(egui::Label::new(egui::RichText::new(format!("{}: {v}", tl!(k))).font(theme::regular(11.5))).wrap());
            };
            if let Some(r) = &s.reason {
                row(ui, "Reason", r);
            }
            if let Some(l) = &s.location {
                row(ui, "Location", l);
            }
            if let Some(d) = &s.date {
                row(ui, "Signing time", &sign::pdf::display_date(d));
            }
            if let Some(a) = &s.algorithm {
                row(ui, "Algorithm", a);
            }
            if let Some(c) = &s.certificate {
                row(ui, "Signer", &c.subject.display());
                row(ui, "Issued by", &c.issuer.display());
                row(ui, "Valid", &format!("{} to {}", c.not_before, c.not_after));
            }
            match s.page {
                Some(p) => {
                    let link = ui.add(
                        egui::Label::new(
                            egui::RichText::new(crate::i18n::fmt(
                                tl!("Field: {field} on page {p}"),
                                &[("field", &s.field), ("p", &(p + 1).to_string())],
                            ))
                            .font(theme::regular(11.5))
                            .color(t.accent_text)
                            .underline(),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if link.clicked() {
                        action = Some(PanelAction::GoTo(p));
                    }
                }
                None => row(ui, "Field", &s.field),
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if let Some(c) = s.certificate.as_ref()
                    && s.status == SignatureStatus::Unknown
                    && ui.button(tl!("Add to trusted certificates")).clicked()
                {
                    action = Some(PanelAction::Trust(Box::new(c.clone())));
                }
                if ui.button(tl!("View signed version")).clicked() {
                    action = Some(PanelAction::ViewSigned { len: s.signed_len, revision: s.revision });
                }
                if let Some(c) = &s.certificate
                    && ui.button(tl!("Show certificate…")).clicked()
                {
                    let mut chain = vec![c.clone()];
                    chain.extend(s.chain.iter().filter(|x| x.raw != c.raw).cloned());
                    action = Some(PanelAction::ViewCertificate(chain));
                }
                if let Some(c) = &s.certificate
                    && ui.button(tl!("Export certificate…")).clicked()
                {
                    action = Some(PanelAction::ExportCertificate(Box::new(c.clone())));
                }
            });
        });
    }
    action
}

#[derive(Clone, Debug)]
pub struct CertViewer {
    pub chain: Vec<Certificate>,
    pub selected: usize,
    pub tab: CertTab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertTab {
    Summary,
    Details,
    Trust,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn key_usage(bits: u16) -> String {
    const NAMES: [&str; 9] = [
        "Digital Signature",
        "Non-Repudiation",
        "Key Encipherment",
        "Data Encipherment",
        "Key Agreement",
        "Certificate Signing",
        "CRL Signing",
        "Encipher Only",
        "Decipher Only",
    ];
    let used: Vec<&str> = NAMES.iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0).map(|(_, n)| tl!(*n)).collect();
    if used.is_empty() { tl!("None").into() } else { used.join(", ") }
}

pub(crate) enum CertAction {
    Trust(Box<Certificate>),
    Export(Box<Certificate>),
}

fn show_cert_chain(ui: &mut egui::Ui, v: &mut CertViewer) {
    for (depth, i) in (0..v.chain.len()).rev().enumerate() {
        let c = &v.chain[i];
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            if ui.selectable_label(v.selected == i, c.display_name()).clicked() {
                v.selected = i;
            }
        });
    }
}

fn show_cert_details(ui: &mut egui::Ui, v: &mut CertViewer, trusted: &[Certificate], t: &Tokens, action: &mut Option<CertAction>) {
    let Some(c) = v.chain.get(v.selected).cloned() else { return };
    ui.horizontal(|ui| {
        for (tab, label) in [(CertTab::Summary, tl!("Summary")), (CertTab::Details, tl!("Details")), (CertTab::Trust, tl!("Trust"))] {
            if widgets::pill_button(ui, label, v.tab == tab).clicked() {
                v.tab = tab;
            }
        }
    });
    ui.add_space(8.0);
    let grid = |ui: &mut egui::Ui, rows: Vec<(&str, String)>| {
        egui::Grid::new(("cert-rows", v.tab as u8)).num_columns(2).spacing([12.0, 5.0]).show(ui, |ui| {
            for (k, val) in rows {
                ui.label(egui::RichText::new(tl_ctx!("certificate", k)).color(t.text_muted));
                ui.add(egui::Label::new(val).wrap());
                ui.end_row();
            }
        });
    };
    match v.tab {
        CertTab::Summary => grid(
            ui,
            vec![
                ("Issued to", c.subject.display()),
                ("Issued by", c.issuer.display()),
                ("Valid from", c.not_before.to_string()),
                ("Valid to", c.not_after.to_string()),
                ("Intended usage", c.key_usage.map(key_usage).unwrap_or_else(|| tl!("Any").to_string())),
            ],
        ),
        CertTab::Details => grid(
            ui,
            vec![
                ("Version", "3".into()),
                ("Serial number", c.serial_hex()),
                ("Issuer", c.issuer.display()),
                ("Subject", c.subject.display()),
                ("Validity starts", c.not_before.to_string()),
                ("Validity ends", c.not_after.to_string()),
                ("Public key", c.public_key.describe()),
                ("Basic constraints", tl!(if c.is_ca { "Certificate authority" } else { "End entity" }).to_string()),
                ("Key usage", c.key_usage.map(key_usage).unwrap_or_else(|| tl!("Not present").to_string())),
                ("Self-signed", if c.is_self_signed() { tl!("Yes").to_string() } else { tl!("No").to_string() }),
                ("SHA-1 digest", hex(&sign::keys::DigestAlg::Sha1.digest(&[&c.raw]))),
                ("SHA-256 digest", hex(&sign::keys::DigestAlg::Sha256.digest(&[&c.raw]))),
            ],
        ),
        CertTab::Trust => {
            let is_trusted = trusted.iter().any(|x| x.raw == c.raw);
            let anchored = v.chain.iter().any(|x| trusted.iter().any(|y| y.raw == x.raw));
            ui.label(if is_trusted {
                tl!("This certificate is in your list of trusted certificates.")
            } else if anchored {
                tl!("This certificate is trusted through a certificate above it in the chain.")
            } else {
                tl!("This certificate is not trusted. Signatures made with it show an unknown identity.")
            });
            ui.add_space(8.0);
            if !is_trusted && widgets::pill_button(ui, tl!("Add to Trusted Certificates"), false).clicked() {
                *action = Some(CertAction::Trust(Box::new(c.clone())));
            }
        }
    }
    ui.add_space(8.0);
    if widgets::pill_button(ui, tl!("Export…"), false).clicked() {
        *action = Some(CertAction::Export(Box::new(c.clone())));
    }
}

pub(crate) fn cert_viewer(ui: &mut egui::Ui, v: &mut CertViewer, trusted: &[Certificate], t: &Tokens) -> (bool, Option<CertAction>) {
    let compact = crate::layout_traits::is_compact(ui);
    title(ui, "Certificate Viewer");
    ui.label(egui::RichText::new(tl!("This dialog shows the details of a certificate and its chain.")).color(t.text_muted));
    ui.add_space(8.0);
    let mut action = None;
    v.selected = v.selected.min(v.chain.len().saturating_sub(1));

    if compact {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            show_cert_chain(ui, v);
        });
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            show_cert_details(ui, v, trusted, t, &mut action);
        });
    } else {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(200.0);
                show_cert_chain(ui, v);
            });
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.set_width(440.0);
                show_cert_details(ui, v, trusted, t, &mut action);
            });
        });
    }

    ui.add_space(10.0);
    let mut close = false;
    ui.horizontal(|ui| ui.with_layout(Layout::right_to_left(Align::Center), |ui| close = widgets::pill_button(ui, tl!("OK"), true).clicked()));
    (close, action)
}

pub fn certificate_pem(c: &Certificate) -> String {
    sign::x509::to_pem(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("pdfcraft-sign-ui-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn create_in(dir: &std::path::Path) -> Result<PathBuf, String> {
        let mut app = PdfCraftApp::new();
        app.export_dir_override = Some(dir.to_string_lossy().into_owned());
        let mut draft = SignDraft::new(0, None, None, None, &[]);
        let key = KEY_ALGORITHMS.iter().position(|k| k.1 == "p256").unwrap();
        draft.new_id =
            NewIdDraft { name: "Grace Hopper".into(), key, password: "secret12".into(), confirm: "secret12".into(), ..NewIdDraft::default() };
        app.sign_draft = Some(draft);
        let i = app.create_digital_id()?;
        Ok(PathBuf::from(&app.digital_ids[i].path))
    }

    #[test]
    fn an_exported_certificate_is_written_inside_the_chosen_folder() {
        let dir = scratch();
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let key = PrivateKey::generate_p256().unwrap();
        let cert = Certificate::self_signed(&Name::build("../x", "", "", "", ""), &key, sign::Time::from_unix(0), 1, &[1]).unwrap();
        let mut app = PdfCraftApp::new();
        app.export_dir_override = Some(out.to_string_lossy().into_owned());
        app.export_certificate(&cert);
        let written: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(written, vec![std::ffi::OsString::from("out")]);
        let pem = std::fs::read_to_string(out.join(".._x.cer")).unwrap();
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"), "{pem}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signature_status_style_gives_each_verdict_its_panel_icon() {
        assert_eq!(signature_status_style(SignatureStatus::Valid).0, "circle-check");
        assert_eq!(signature_status_style(SignatureStatus::Unknown).0, "triangle-alert");
        assert_eq!(signature_status_style(SignatureStatus::Invalid).0, "circle-x");
    }

    #[test]
    fn a_new_digital_id_skips_names_already_taken_without_writing_through_them() {
        let dir = scratch();
        std::fs::write(dir.join("victim.txt"), "keep me").unwrap();
        std::fs::hard_link(dir.join("victim.txt"), dir.join("Grace Hopper.p12")).unwrap();
        std::fs::create_dir(dir.join("Grace Hopper 2.p12")).unwrap();
        let saved = create_in(&dir).unwrap();
        assert_eq!(saved, dir.join("Grace Hopper 3.p12"));
        assert_eq!(std::fs::read_to_string(dir.join("victim.txt")).unwrap(), "keep me");
        assert!(sign::pkcs12::open(&std::fs::read(&saved).unwrap(), "secret12").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_digital_id_is_not_written_through_a_dangling_link() {
        let dir = scratch();
        let (link, target) = (dir.join("Grace Hopper.p12"), dir.join("elsewhere.p12"));
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&target, &link);
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&target, &link);
        #[cfg(not(any(unix, windows)))]
        let made: std::io::Result<()> = {
            let _ = (&target, &link);
            Err(std::io::ErrorKind::Unsupported.into())
        };
        if let Err(e) = made {
            eprintln!("skipped: can't create a symlink here: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let saved = create_in(&dir).unwrap();
        assert!(!target.exists(), "nothing written through the link");
        assert_eq!(saved, dir.join("Grace Hopper 2.p12"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_new_digital_id_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch();
        let saved = create_in(&dir).unwrap();
        let mode = std::fs::metadata(&saved).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "the private key is not readable by others: {mode:o}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn listed(path: &str, name: &str) -> DigitalIdEntry {
        DigitalIdEntry { path: path.into(), name: name.into(), issuer: String::new(), email: String::new(), expires: String::new(), unusable: None }
    }

    #[test]
    fn a_digital_id_file_is_removed_from_the_list_and_stays_removed() {
        let dir = scratch();
        let file = dir.join("old.p12");
        std::fs::write(&file, b"not opened").unwrap();
        let mut app = PdfCraftApp::new();
        app.digital_ids = vec![listed("windows:abc", "Store"), listed(&file.to_string_lossy(), "Old"), listed("/ids/new.p12", "New")];
        let mut draft = SignDraft::new(0, None, None, None, &app.digital_ids);
        draft.selected = Some(2);
        draft.confirm_remove = Some(1);
        app.sign_draft = Some(draft);

        assert!(!app.remove_digital_id(0), "OS key store IDs stay");
        assert!(!app.remove_digital_id(9), "an index past the list is refused");
        assert!(app.remove_digital_id(1));
        assert_eq!(app.digital_ids.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Store", "New"]);
        assert!(file.exists(), "the file itself is not deleted");
        let d = app.sign_draft.as_ref().unwrap();
        assert_eq!((d.selected, d.confirm_remove, d.step), (Some(1), None, SignStep::Choose), "still New, no pending question");

        assert!(app.remove_digital_id(1));
        assert_eq!(app.sign_draft.as_ref().unwrap().selected, Some(0));
        let mut fresh = PdfCraftApp::new();
        fresh.restore(&app.persist());
        assert!(fresh.digital_ids.is_empty(), "the removal survives a restart: {:?}", fresh.digital_ids);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn appearance_labels_are_translated_or_all_english() {
        use crate::i18n::{Lang, tr};
        let english = AppearanceLabels::default();
        assert_eq!(appearance_labels(Lang::EN), (english.clone(), false));
        for code in ["de", "es", "fr", "it", "pt-br", "pl", "tr", "sl", "hu"] {
            let lang = Lang::from_code(code).unwrap();
            let (labels, in_english) = appearance_labels(lang);
            assert!(!in_english, "{code}: {labels:?}");
            for ((field, ours), (_, source)) in labels.fields().into_iter().zip(english.fields()) {
                assert_eq!(ours, tr(lang, source), "{code}: {field}");
            }
        }
        let (de, _) = appearance_labels(Lang::from_code("de").unwrap());
        assert_eq!((de.signed_by.as_str(), de.reason.as_str()), ("Digital signiert von", "Grund: "));
        for code in ["zh-hans", "zh-hant", "ja", "ru", "uk", "bg", "kk", "el", "te", "cs", "lt", "ar"] {
            assert_eq!(appearance_labels(Lang::from_code(code).unwrap()), (english.clone(), true), "{code}");
        }
        for lang in Lang::all() {
            let (labels, in_english) = appearance_labels(lang);
            assert_eq!(labels.first_undrawable(), None, "{}: signing would refuse {labels:?}", lang.code());
            assert_eq!(in_english, lang != Lang::EN && labels == english, "{}", lang.code());
        }
    }

    #[test]
    fn preview_lines_use_the_labels_signing_stamps() {
        let (de, _) = appearance_labels(crate::i18n::Lang::from_code("de").unwrap());
        let all = Appearance { reason: true, location: true, ..Appearance::default() };
        let lines = detail_lines(&all, &de, "Ada", " Genehmigt ", "Berlin", "2026.10.14 09:30:00 Z");
        assert_eq!(lines, ["Digital signiert von", "Ada", "Grund: Genehmigt", "Ort: Berlin", "Datum: 2026.10.14 09:30:00 Z"]);
        let bare = Appearance { labels: false, ..all.clone() };
        assert_eq!(detail_lines(&bare, &de, "Ada", "Genehmigt", "", "D"), ["Ada", "Genehmigt", "D"]);
        let nothing = Appearance { name: false, date: false, ..all };
        assert!(detail_lines(&nothing, &de, "Ada", "  ", "", "D").is_empty());
    }
}
