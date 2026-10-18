use egui::{Context, Margin, Rect, Ui, Vec2, pos2, vec2};


pub const COMPACT_BREAKPOINT: f32 = 640.0;

pub const TOUCH_TARGET_SIZE: f32 = 44.0;

pub const TOUCH_TARGET_MATERIAL: f32 = 48.0;

pub const DESKTOP_CONTROL_SIZE: f32 = 28.0;

pub const STATUS_BAR_HEIGHT: f32 = 36.0;

pub const NAV_BAR_HEIGHT: f32 = 32.0;

pub const LANDSCAPE_CUTOUT_WIDTH: f32 = 44.0;

pub const MOBILE_BOTTOM_BAR_HEIGHT: f32 = 56.0;

pub const MOBILE_HEADER_HEIGHT: f32 = 48.0;


#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]

pub struct SafeAreaInsets {

    pub top: f32,

    pub bottom: f32,

    pub left: f32,

    pub right: f32,

}


impl SafeAreaInsets {

    pub const ZERO: Self = Self {

        top: 0.0,

        bottom: 0.0,

        left: 0.0,

        right: 0.0,

    };


    pub fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {

        Self { top, bottom, left, right }

    }


    pub fn is_empty(&self) -> bool {

        self.top <= 0.0 && self.bottom <= 0.0 && self.left <= 0.0 && self.right <= 0.0

    }


    pub fn to_margin(&self) -> Margin {

        Margin {

            left: self.left.round() as i8,

            right: self.right.round() as i8,

            top: self.top.round() as i8,

            bottom: self.bottom.round() as i8,

        }

    }


    pub fn inset_rect(&self, rect: Rect) -> Rect {

        Rect::from_min_max(

            pos2(rect.min.x + self.left, rect.min.y + self.top),

            pos2(rect.max.x - self.right, rect.max.y - self.bottom),

        )

    }

}


pub fn safe_area_insets(ctx: &Context) -> SafeAreaInsets {

    if let Some(insets) = ctx.data(|d| d.get_temp::<SafeAreaInsets>(egui::Id::new("safe_area_insets"))) {

        return insets;

    }

    if is_mobile_os() {

        let screen = ctx.content_rect();

        if screen.width() < screen.height() {

            SafeAreaInsets {

                top: STATUS_BAR_HEIGHT,

                bottom: NAV_BAR_HEIGHT,

                left: 0.0,

                right: 0.0,

            }

        } else {

            SafeAreaInsets {

                top: 0.0,

                bottom: NAV_BAR_HEIGHT,

                left: LANDSCAPE_CUTOUT_WIDTH,

                right: 0.0,

            }

        }

    } else {

        SafeAreaInsets::ZERO

    }

}


pub fn set_safe_area_insets(ctx: &Context, insets: SafeAreaInsets) {

    ctx.data_mut(|d| d.insert_temp(egui::Id::new("safe_area_insets"), insets));

}


pub fn init_android_safe_area(ctx: &Context) {

    let insets = safe_area_insets(ctx);

    set_safe_area_insets(ctx, insets);

}


#[inline]

pub fn top_chrome_inset(ctx: &Context) -> f32 {

    safe_area_insets(ctx).top

}


#[inline]

pub fn bottom_nav_inset(ctx: &Context) -> f32 {

    safe_area_insets(ctx).bottom

}


pub fn safe_inner_margin(ctx: &Context, base: Margin) -> Margin {

    let insets = safe_area_insets(ctx);

    Margin {

        left: base.left.saturating_add(insets.left.round() as i8),

        right: base.right.saturating_add(insets.right.round() as i8),

        top: base.top.saturating_add(insets.top.round() as i8),

        bottom: base.bottom.saturating_add(insets.bottom.round() as i8),

    }

}


#[inline]

pub fn top_panel_height(base_height: f32, ctx: &Context) -> f32 {

    base_height + top_chrome_inset(ctx)

}


pub fn top_panel_margin(base_margin: Margin, ctx: &Context) -> Margin {

    let mut m = base_margin;

    m.top = m.top.saturating_add(top_chrome_inset(ctx).round() as i8);

    m

}


#[inline]

pub fn bottom_panel_height(base_height: f32, ctx: &Context) -> f32 {

    base_height + bottom_nav_inset(ctx)

}


pub fn bottom_panel_margin(base_margin: Margin, ctx: &Context) -> Margin {

    let mut m = base_margin;

    m.bottom = m.bottom.saturating_add(bottom_nav_inset(ctx).round() as i8);

    m

}


#[inline]
pub fn is_compact(ui: &Ui) -> bool {
    ui.available_width() < COMPACT_BREAKPOINT || is_mobile_os()
}


#[inline]

pub fn is_compact_ctx(ctx: &Context) -> bool {

    ctx.content_rect().width() < COMPACT_BREAKPOINT || is_mobile_os()

}


#[inline]

pub fn is_mobile_os() -> bool {

    cfg!(target_os = "android") || cfg!(target_os = "ios")

}


#[inline]

pub fn is_touch(ui: &Ui) -> bool {

    is_mobile_os() || is_compact(ui)

}


#[inline]

pub fn is_touch_device(ui: &Ui) -> bool {

    is_touch(ui)

}


#[inline]

pub fn is_mobile(ui: &Ui) -> bool {

    is_mobile_os() || is_compact(ui)

}


#[inline]

pub fn control_size(ui: &Ui) -> f32 {

    if is_touch(ui) {

        TOUCH_TARGET_SIZE

    } else {

        DESKTOP_CONTROL_SIZE

    }

}


#[inline]

pub fn touch_size(ui: &Ui, desktop_size: f32) -> f32 {

    if is_touch(ui) {

        desktop_size.max(TOUCH_TARGET_SIZE)

    } else {

        desktop_size

    }

}


#[inline]

pub fn touch_material_size(ui: &Ui, desktop_size: f32) -> f32 {

    if is_touch(ui) {

        desktop_size.max(TOUCH_TARGET_MATERIAL)

    } else {

        desktop_size

    }

}


#[inline]

pub fn touch_size_custom(ui: &Ui, desktop_size: f32, touch_target: f32) -> f32 {

    if is_touch(ui) {

        touch_target

    } else {

        desktop_size

    }

}


#[inline]

pub fn touch_vec(ui: &Ui, desktop_size: Vec2) -> Vec2 {

    if is_touch(ui) {

        vec2(desktop_size.x.max(TOUCH_TARGET_SIZE), desktop_size.y.max(TOUCH_TARGET_SIZE))

    } else {

        desktop_size

    }

}


#[inline]

pub fn touch_margin(ui: &Ui, desktop_margin: Margin, touch_margin: Margin) -> Margin {

    if is_touch(ui) {

        touch_margin

    } else {

        desktop_margin

    }

}


#[inline]

pub fn touch_pad(ui: &Ui, desktop_pad: f32, touch_pad: f32) -> f32 {

    if is_touch(ui) {

        touch_pad

    } else {

        desktop_pad

    }

}


#[inline]

pub fn expand_hitbox(ui: &Ui, rect: Rect, min_size: f32) -> Rect {

    if !is_touch(ui) {

        return rect;

    }

    let target = min_size.max(TOUCH_TARGET_SIZE);

    let dw = (target - rect.width()).max(0.0);

    let dh = (target - rect.height()).max(0.0);

    rect.expand2(vec2(dw * 0.5, dh * 0.5))

}

