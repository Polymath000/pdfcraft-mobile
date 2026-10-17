//! Responsive and adaptive layout helpers for mobile and compact viewports.

use egui::Ui;

/// Breakpoint threshold in points below which the interface switches to compact mobile layout.
pub const COMPACT_BREAKPOINT: f32 = 640.0;

/// Touch-friendly minimum target size according to Android Material guidelines (44pt).
pub const TOUCH_TARGET_SIZE: f32 = 44.0;

/// Standard desktop button/control size.
pub const DESKTOP_CONTROL_SIZE: f32 = 28.0;

/// Whether the current available area is compact (e.g. phone portrait or narrow window).
#[inline]
pub fn is_compact(ui: &Ui) -> bool {
    ui.available_width() < COMPACT_BREAKPOINT
}

/// Whether the target is a mobile platform (Android or iOS).
#[inline]
pub fn is_mobile_os() -> bool {
    cfg!(target_os = "android") || cfg!(target_os = "ios")
}

/// Whether touch-first ergonomics should be prioritized.
#[inline]
pub fn is_touch(ui: &Ui) -> bool {
    is_mobile_os() || is_compact(ui)
}

/// Dynamic control size adapting to screen density and touch capability.
#[inline]
pub fn control_size(ui: &Ui) -> f32 {
    if is_touch(ui) { TOUCH_TARGET_SIZE } else { DESKTOP_CONTROL_SIZE }
}
