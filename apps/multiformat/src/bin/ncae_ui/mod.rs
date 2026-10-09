//! Presentation-only egui UI. Audio processing and replacement stay in the parent module.
use super::{ncae_jobs::Job, NcaeGuiApp};
use egui::{Color32, FontId, RichText, Stroke, Vec2};
use ncae_tool::{
    wav::{ChannelMode, LengthMode},
    EffectInfo, PayloadKind,
};
use std::path::Path;

#[derive(Clone, Copy)]
struct Palette {
    ink: Color32,
    muted: Color32,
    accent: Color32,
    tint: Color32,
    panel: Color32,
    surface: Color32,
    border: Color32,
    warning: Color32,
    error: Color32,
    primary: Color32,
    selected_border: Color32,
    hover: Color32,
    ir_text: Color32,
    ir_bg: Color32,
    json_text: Color32,
    json_bg: Color32,
    neutral_bg: Color32,
}

impl Palette {
    fn for_dark(dark: bool) -> Self {
        let rgb = Color32::from_rgb;
        if dark {
            Self {
                ink: rgb(228, 234, 239),
                muted: rgb(162, 176, 186),
                accent: rgb(115, 205, 187),
                tint: rgb(34, 65, 61),
                panel: rgb(30, 35, 41),
                surface: rgb(24, 29, 35),
                border: rgb(54, 65, 75),
                warning: rgb(242, 190, 119),
                error: rgb(245, 146, 154),
                primary: rgb(20, 119, 109),
                selected_border: rgb(65, 123, 110),
                hover: rgb(39, 47, 55),
                ir_text: rgb(137, 218, 197),
                ir_bg: rgb(33, 66, 58),
                json_text: rgb(209, 180, 247),
                json_bg: rgb(61, 45, 79),
                neutral_bg: rgb(46, 55, 64),
            }
        } else {
            Self {
                ink: rgb(31, 43, 50),
                muted: rgb(99, 114, 123),
                accent: rgb(20, 119, 109),
                tint: rgb(231, 243, 239),
                panel: Color32::WHITE,
                surface: rgb(246, 248, 249),
                border: rgb(224, 230, 232),
                warning: rgb(154, 87, 24),
                error: rgb(175, 54, 54),
                primary: rgb(20, 119, 109),
                selected_border: rgb(181, 216, 207),
                hover: rgb(236, 241, 242),
                ir_text: rgb(22, 110, 103),
                ir_bg: rgb(221, 241, 235),
                json_text: rgb(111, 76, 158),
                json_bg: rgb(238, 231, 248),
                neutral_bg: rgb(232, 235, 238),
            }
        }
    }
}

fn palette(ui: &egui::Ui) -> Palette {
    ui.ctx()
        .data(|data| data.get_temp::<Palette>(egui::Id::new("ncae_current_palette")))
        .unwrap_or_else(|| Palette::for_dark(ui.visuals().dark_mode))
}

pub(super) fn setup_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::System);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.set_style_of(
            theme,
            themed_style(theme, Palette::for_dark(theme == egui::Theme::Dark)),
        );
    }
}

fn themed_style(theme: egui::Theme, p: Palette) -> egui::Style {
    let mut style = theme.default_style();
    style.visuals = theme.default_visuals();
    style.visuals.override_text_color = Some(p.ink);
    style.visuals.weak_text_color = Some(p.muted);
    style.visuals.panel_fill = p.panel;
    style.visuals.window_fill = p.panel;
    style.visuals.extreme_bg_color = p.surface;
    style.visuals.faint_bg_color = p.surface;
    style.visuals.text_edit_bg_color = Some(p.panel);
    style.visuals.selection.bg_fill = p.tint;
    style.visuals.selection.stroke = Stroke::new(1.0, p.accent);
    style.visuals.hyperlink_color = p.accent;
    style.visuals.window_corner_radius = 10.into();
    style.visuals.menu_corner_radius = 8.into();
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 6.into();
        widget.expansion = 0.0;
        widget.fg_stroke = Stroke::new(1.2, p.ink);
    }
    style.visuals.widgets.inactive.bg_fill = p.panel;
    style.visuals.widgets.inactive.weak_bg_fill = p.panel;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    style.visuals.widgets.hovered.bg_fill = p.tint;
    style.visuals.widgets.hovered.weak_bg_fill = p.tint;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    style.visuals.widgets.active.bg_fill = p.tint;
    style.visuals.widgets.active.weak_bg_fill = p.tint;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    style.spacing.item_spacing = Vec2::new(10.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size = Vec2::new(32.0, 30.0);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(23.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(12.0));
    style.spacing.menu_margin = egui::Margin::symmetric(10, 10);

    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    style.visuals.widgets.noninteractive.bg_fill = p.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = p.surface;
    style.visuals.window_stroke = Stroke::new(1.0, p.border);
    style
}

const THEME_TRANSITION_SECONDS: f64 = 0.25;
#[derive(Clone, Copy)]
struct ThemeTransition {
    from: f32,
    to: f32,
    started: f64,
}
impl ThemeTransition {
    fn sample(self, now: f64) -> f32 {
        let t = ((now - self.started) / THEME_TRANSITION_SECONDS).clamp(0.0, 1.0) as f32;
        let eased = t * t * (3.0 - 2.0 * t);
        self.from + (self.to - self.from) * eased
    }
}

fn blended_palette(amount: f32) -> Palette {
    let light = Palette::for_dark(false);
    let dark = Palette::for_dark(true);
    let mix = |a: Color32, b: Color32| {
        let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Color32::from_rgb(
            channel(a.r(), b.r()),
            channel(a.g(), b.g()),
            channel(a.b(), b.b()),
        )
    };
    Palette {
        ink: mix(light.ink, dark.ink),
        muted: mix(light.muted, dark.muted),
        accent: mix(light.accent, dark.accent),
        tint: mix(light.tint, dark.tint),
        panel: mix(light.panel, dark.panel),
        surface: mix(light.surface, dark.surface),
        border: mix(light.border, dark.border),
        warning: mix(light.warning, dark.warning),
        error: mix(light.error, dark.error),
        primary: mix(light.primary, dark.primary),
        selected_border: mix(light.selected_border, dark.selected_border),
        hover: mix(light.hover, dark.hover),
        ir_text: mix(light.ir_text, dark.ir_text),
        ir_bg: mix(light.ir_bg, dark.ir_bg),
        json_text: mix(light.json_text, dark.json_text),
        json_bg: mix(light.json_bg, dark.json_bg),
        neutral_bg: mix(light.neutral_bg, dark.neutral_bg),
    }
}

fn apply_animated_theme(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let theme = ctx.theme();
    let desired = if theme == egui::Theme::Dark { 1.0 } else { 0.0 };
    let now = ctx.input(|input| input.time);
    let id = egui::Id::new("ncae_theme_transition");
    let mut transition = ctx
        .data(|data| data.get_temp::<ThemeTransition>(id))
        .unwrap_or(ThemeTransition {
            from: desired,
            to: desired,
            started: now,
        });
    let current = transition.sample(now);
    if transition.to != desired {
        transition = ThemeTransition {
            from: current,
            to: desired,
            started: now,
        };
    }
    let amount = transition.sample(now);
    let colors = blended_palette(amount);
    ctx.data_mut(|data| {
        data.insert_temp(id, transition);
        data.insert_temp(egui::Id::new("ncae_current_palette"), colors);
    });
    let style = std::sync::Arc::new(themed_style(theme, colors));
    // Root UI already exists for this frame. Update it as well as newly opened popups.
    ui.set_style(style.clone());
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    if (amount - desired).abs() > 0.001 {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn panel_frame(fill: Color32, x: i8, y: i8) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(x, y))
}

fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).size(12.0).color(palette(ui).muted));
}

fn section(ui: &mut egui::Ui, number: &str, title: &str, description: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(number)
                .size(13.0)
                .strong()
                .color(palette(ui).accent),
        );
        ui.label(RichText::new(title).size(17.0).strong());
        ui.add_space(4.0);
        muted(ui, description);
    });
    ui.add_space(4.0);
}

fn dialog_frame(ctx: &egui::Context) -> egui::Frame {
    let style = ctx.style_of(ctx.theme());
    egui::Frame::popup(&style)
        .inner_margin(24)
        .corner_radius(12)
}

const LOG_VISIBLE_ROWS: usize = 5;
const LOG_FONT_SIZE: f32 = 11.0;
const LOG_ROW_GAP: f32 = 6.0;
fn log_viewport_height(ui: &egui::Ui) -> f32 {
    let line_height = ui.fonts_mut(|fonts| fonts.row_height(&FontId::monospace(LOG_FONT_SIZE)));
    LOG_VISIBLE_ROWS as f32 * line_height + (LOG_VISIBLE_ROWS - 1) as f32 * LOG_ROW_GAP
}

#[cfg(test)]
fn log_viewport(ui: &mut egui::Ui, lines: &[String]) -> f32 {
    let height = log_viewport_height(ui);
    log_viewport_sized(ui, lines, height)
}

fn log_viewport_sized(ui: &mut egui::Ui, lines: &[String], height: f32) -> f32 {
    ui.spacing_mut().item_spacing.y = LOG_ROW_GAP;
    egui::ScrollArea::vertical()
        .id_salt("log_scroll")
        .stick_to_bottom(true)
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in lines {
                let color = if line.contains("失败") {
                    palette(ui).error
                } else {
                    palette(ui).muted
                };
                // Ordinary left-aligned wrapping labels, as in the original log view.
                ui.add(
                    egui::Label::new(
                        RichText::new(line)
                            .monospace()
                            .size(LOG_FONT_SIZE)
                            .color(color),
                    )
                    .halign(egui::Align::Min)
                    .wrap(),
                );
            }
        })
        .inner_rect
        .height()
}

fn theme_toggle(ui: &mut egui::Ui) {
    let dark = ui.visuals().dark_mode;
    let response = ui.add_sized([34.0, 34.0], AnimatedButton::new("").frame(false));
    let center = response.rect.center();
    let stroke = Stroke::new(1.5, palette(ui).ink);
    if dark {
        ui.painter().circle_stroke(center, 4.0, stroke);
        for index in 0..8 {
            let angle = index as f32 * std::f32::consts::FRAC_PI_4;
            let direction = Vec2::new(angle.cos(), angle.sin());
            ui.painter()
                .line_segment([center + direction * 7.0, center + direction * 9.0], stroke);
        }
    } else {
        for points in [
            [
                Vec2::new(-1.0, -8.0),
                Vec2::new(-12.0, -2.0),
                Vec2::new(-5.0, 12.0),
                Vec2::new(7.0, 6.0),
            ],
            [
                Vec2::new(7.0, 6.0),
                Vec2::new(-1.0, 7.0),
                Vec2::new(-6.0, 0.0),
                Vec2::new(-1.0, -8.0),
            ],
        ] {
            ui.painter()
                .add(egui::epaint::CubicBezierShape::from_points_stroke(
                    points.map(|point| center + point),
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
        }
    }
    let label = if dark {
        "切换为亮色主题"
    } else {
        "切换为暗色主题"
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if response.clicked() {
        ui.ctx().request_repaint();
        ui.ctx().set_theme(if dark {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
    }
    response.context_menu(|ui| {
        if ui.animated_button("跟随系统").clicked() {
            ui.ctx().set_theme(egui::ThemePreference::System);
            ui.ctx().request_repaint();
            ui.close();
        }
    });
    response.on_hover_text(format!("{label}\n右键可恢复跟随系统"));
}

fn mix_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let a = a.to_array();
    let b = b.to_array();
    let channel = |index: usize| {
        (a[index] as f32 + (b[index] as f32 - a[index] as f32) * t.clamp(0.0, 1.0)).round() as u8
    };
    Color32::from_rgba_premultiplied(channel(0), channel(1), channel(2), channel(3))
}

fn hover_amount(ui: &egui::Ui, id: egui::Id) -> f32 {
    let hovered = ui.is_enabled()
        && ui
            .ctx()
            .read_response(id)
            .is_some_and(|response| response.hovered());
    let amount = ui
        .ctx()
        .animate_bool_with_time(id.with("ncae_hover"), hovered, 0.16);
    if amount > 0.001 && amount < 0.999 {
        ui.ctx().request_repaint();
    }
    #[cfg(test)]
    ui.ctx()
        .data_mut(|data| data.insert_temp(id.with("hover_test_amount"), amount));
    amount
}

fn gradient_shape(rect: egui::Rect, left: Color32, right: Color32) -> egui::Shape {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), mix_color(left, right, 0.5));
    let radius = 6.0_f32.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -90.0_f32,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            90.0,
        ),
        (egui::pos2(rect.left() + radius, rect.top() + radius), 180.0),
    ];
    for (center, start) in corners {
        for step in 0..=5 {
            let angle = (start + step as f32 * 18.0).to_radians();
            let point = center + Vec2::new(angle.cos(), angle.sin()) * radius;
            let fraction = ((point.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
            mesh.colored_vertex(point, mix_color(left, right, fraction));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for index in 1..=count {
        mesh.add_triangle(0, index, if index == count { 1 } else { index + 1 });
    }
    egui::Shape::mesh(mesh)
}

struct AnimatedButton {
    inner: egui::Button<'static>,
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    selected: bool,
    framed: bool,
}
impl AnimatedButton {
    fn new(text: impl Into<egui::WidgetText>) -> Self {
        let text: egui::WidgetText = text.into();
        Self {
            inner: egui::Button::new(text),
            fill: None,
            stroke: None,
            selected: false,
            framed: true,
        }
    }
    fn fill(mut self, color: Color32) -> Self {
        self.fill = Some(color);
        self
    }
    fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }
    fn selected(mut self, value: bool) -> Self {
        self.selected = value;
        self.inner = self.inner.selected(value);
        self
    }
    fn min_size(mut self, size: Vec2) -> Self {
        self.inner = self.inner.min_size(size);
        self
    }
    fn small(mut self) -> Self {
        self.inner = self.inner.small();
        self
    }
    fn frame(mut self, framed: bool) -> Self {
        self.framed = framed;
        self.inner = self.inner.frame(framed);
        self
    }
}
impl egui::Widget for AnimatedButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let amount = hover_amount(ui, ui.next_auto_id());
        let p = palette(ui);
        let base = self.fill.unwrap_or(if !self.framed {
            Color32::TRANSPARENT
        } else if self.selected {
            p.tint
        } else {
            p.panel
        });
        let primary = self.fill == Some(p.primary);
        let hover = if primary {
            mix_color(base, Color32::WHITE, 0.15)
        } else {
            p.tint
        };
        let right = if primary {
            mix_color(base, Color32::WHITE, 0.05)
        } else {
            mix_color(p.tint, p.accent, 0.08)
        };
        let border = self.stroke.unwrap_or(Stroke::new(
            1.0,
            mix_color(p.border, p.accent, amount * 0.60),
        ));
        let background = ui.painter().add(egui::Shape::Noop);
        let response = egui::Widget::ui(self.inner.fill(Color32::TRANSPARENT).stroke(border), ui);
        if ui.is_rect_visible(response.rect) {
            ui.painter().set(
                background,
                gradient_shape(
                    response.rect,
                    mix_color(base, hover, amount),
                    mix_color(base, right, amount),
                ),
            );
        }
        response
    }
}

trait AnimatedUiExt {
    fn animated_button(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response;
    fn animated_small_button(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response;
    fn animated_selectable_label(
        &mut self,
        selected: bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response;
    fn animated_selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response;
    fn animated_checkbox(
        &mut self,
        checked: &mut bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response;
    fn animated_menu_button<R>(
        &mut self,
        text: impl Into<egui::WidgetText>,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<R>>;
}
impl AnimatedUiExt for egui::Ui {
    fn animated_button(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response {
        self.add(AnimatedButton::new(text))
    }
    fn animated_small_button(&mut self, text: impl Into<egui::WidgetText>) -> egui::Response {
        self.add(AnimatedButton::new(text).small())
    }
    fn animated_selectable_label(
        &mut self,
        selected: bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.add(
            AnimatedButton::new(text)
                .selected(selected)
                .stroke(Stroke::NONE),
        )
    }
    fn animated_selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let mut response = self.animated_selectable_label(*current == value, text);
        if response.clicked() && *current != value {
            *current = value;
            response.mark_changed();
        }
        response
    }
    fn animated_checkbox(
        &mut self,
        checked: &mut bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let amount = hover_amount(self, self.next_auto_id());
        let p = palette(self);
        let previous = self.style().clone();
        let mut style = (*previous).clone();
        for visuals in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
        ] {
            visuals.bg_fill = mix_color(p.panel, p.tint, amount);
            visuals.bg_stroke = Stroke::new(1.0, mix_color(p.border, p.accent, amount));
        }
        self.set_style(style);
        let text: egui::WidgetText = text.into();
        let response = self.checkbox(checked, text);
        self.set_style(previous);
        response
    }
    fn animated_menu_button<R>(
        &mut self,
        text: impl Into<egui::WidgetText>,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<R>> {
        let response = self.animated_button(text);
        egui::Popup::menu(&response).show(contents)
    }
}

fn preview_plot(
    ui: &mut egui::Ui,
    model: &super::ncae_preview::core::Model,
    show_markers: bool,
    module: Option<&str>,
) {
    use super::ncae_preview::core::Kind;
    let curves: Vec<_> = model
        .curves
        .iter()
        .filter(|curve| module.is_none_or(|key| curve.module == key))
        .collect();
    let controls: &[[f64; 2]] = if module.is_none() || module == Some("eq") {
        &model.controls
    } else {
        &[]
    };
    let markers: &[[f64; 2]] = if module.is_none() || module == Some("peq") {
        &model.markers
    } else {
        &[]
    };
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 166.0), egui::Sense::hover());
    let rect = response.rect;
    let area = egui::Rect::from_min_max(
        rect.min + Vec2::new(43.0, 12.0),
        rect.max - Vec2::new(12.0, 26.0),
    );
    let p = palette(ui);
    painter.rect_filled(rect, 8, p.surface);
    let (low, high) = if model.kind == Kind::Ir {
        if curves.iter().all(|curve| curve.silent) {
            (-180.0, 0.0)
        } else {
            let peak = curves
                .iter()
                .flat_map(|curve| curve.points.iter().map(|point| point[1]))
                .fold(f64::NEG_INFINITY, f64::max);
            let top = (peak / 10.0).ceil() * 10.0;
            (top - 60.0, top)
        }
    } else {
        let mut low = -12.0_f64;
        let mut high = 12.0_f64;
        for point in curves.iter().flat_map(|curve| &curve.points) {
            low = low.min(point[1] - 3.0);
            high = high.max(point[1] + 3.0);
        }
        if show_markers {
            for point in markers {
                if point[1].abs() <= 180.0 {
                    low = low.min(point[1] - 3.0);
                    high = high.max(point[1] + 3.0);
                }
            }
        }
        ((low / 6.0).floor() * 6.0, (high / 6.0).ceil() * 6.0)
    };
    let x = |hz: f64| {
        area.left()
            + ((hz / model.min_hz).ln() / (model.max_hz / model.min_hz).ln()) as f32 * area.width()
    };
    let y = |db: f64| area.top() + ((high - db) / (high - low)) as f32 * area.height();
    for i in 0..=4 {
        let db = high - (high - low) * i as f64 / 4.0;
        let yy = y(db);
        painter.line_segment(
            [egui::pos2(area.left(), yy), egui::pos2(area.right(), yy)],
            Stroke::new(0.7, p.border),
        );
        painter.text(
            egui::pos2(area.left() - 6.0, yy),
            egui::Align2::RIGHT_CENTER,
            format!("{db:.0}"),
            FontId::proportional(10.0),
            p.muted,
        );
    }
    painter.text(
        rect.min + Vec2::new(6.0, 7.0),
        egui::Align2::LEFT_TOP,
        "dB",
        FontId::proportional(10.0),
        p.muted,
    );
    let mut previous = -1000.0_f32;
    for frequency in [
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ] {
        if frequency < model.min_hz || frequency > model.max_hz {
            continue;
        }
        let xx = x(frequency);
        painter.line_segment(
            [egui::pos2(xx, area.top()), egui::pos2(xx, area.bottom())],
            Stroke::new(0.7, p.border),
        );
        if xx - previous >= 34.0 {
            let text = if frequency >= 1000.0 {
                format!("{}k", frequency as u32 / 1000)
            } else {
                format!("{frequency:.0}")
            };
            painter.text(
                egui::pos2(xx, area.bottom() + 8.0),
                egui::Align2::CENTER_TOP,
                text,
                FontId::proportional(10.0),
                p.muted,
            );
            previous = xx;
        }
    }
    let colors = [p.accent, p.warning, p.json_text, p.ir_text];
    let clipped = painter.with_clip_rect(area);
    for (index, curve) in curves.iter().enumerate() {
        let points = curve
            .points
            .iter()
            .map(|point| egui::pos2(x(point[0]), y(point[1])))
            .filter(|point| point.is_finite())
            .collect();
        clipped.add(egui::Shape::line(
            points,
            Stroke::new(1.5, colors[index % colors.len()]),
        ));
    }
    for point in controls {
        if point[0] >= model.min_hz
            && point[0] <= model.max_hz
            && point[1] >= low
            && point[1] <= high
        {
            clipped.circle_filled(egui::pos2(x(point[0]), y(point[1])), 2.8, p.accent);
        }
    }
    if show_markers {
        for point in markers {
            if point[0] >= model.min_hz
                && point[0] <= model.max_hz
                && point[1] >= low
                && point[1] <= high
            {
                let center = egui::pos2(x(point[0]), y(point[1]));
                clipped.add(egui::Shape::closed_line(
                    vec![
                        center + Vec2::new(0.0, -4.0),
                        center + Vec2::new(4.0, 0.0),
                        center + Vec2::new(0.0, 4.0),
                        center + Vec2::new(-4.0, 0.0),
                    ],
                    Stroke::new(1.2, p.warning),
                ));
            }
        }
    }
    if curves.is_empty() && !(show_markers && !markers.is_empty()) {
        painter.text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            "没有可确认的 EQ 曲线",
            FontId::proportional(13.0),
            p.muted,
        );
    }
    if let Some(position) = response
        .hover_pos()
        .filter(|position| area.contains(*position))
    {
        let frequency = model.min_hz
            * (model.max_hz / model.min_hz)
                .powf(((position.x - area.left()) / area.width()) as f64);
        painter.line_segment(
            [
                egui::pos2(position.x, area.top()),
                egui::pos2(position.x, area.bottom()),
            ],
            Stroke::new(0.7, p.muted),
        );
        let mut text = format!("{frequency:.0} Hz");
        for curve in &curves {
            if let Some(point) = curve.points.iter().min_by(|a, b| {
                (a[0] - frequency)
                    .abs()
                    .total_cmp(&(b[0] - frequency).abs())
            }) {
                text.push_str(&format!("\n{}：{:.2} dB", curve.label, point[1]));
            }
        }
        response.on_hover_text(text);
    }
    if model.kind == Kind::Ir {
        ui.horizontal_wrapped(|ui| {
            for (index, curve) in curves.iter().enumerate() {
                ui.label(
                    RichText::new(&curve.label)
                        .size(11.0)
                        .color(colors[index % colors.len()]),
                );
            }
        });
    }
}

fn preview_state_text(state: super::ncae_preview::core::State) -> &'static str {
    use super::ncae_preview::core::State;
    match state {
        State::On => "开",
        State::Off => "关",
        State::Unknown => "未知",
        State::Missing => "未配置",
    }
}
fn preview_compact(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{result}…")
    } else {
        result
    }
}
fn preview_parameters(ui: &mut egui::Ui, module: &super::ncae_preview::core::Module) {
    let parameters: Vec<_> = module
        .parameters
        .iter()
        .filter(|parameter| module.key != "peq" || parameter.path != "f")
        .collect();
    if parameters.is_empty() {
        if module.key != "peq" {
            muted(ui, "暂无可显示参数");
        }
        return;
    }
    egui::Frame::NONE
        .fill(palette(ui).surface)
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt(("module_params_scroll", &module.key))
                .max_height(160.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new(("module_params_grid", &module.key))
                        .num_columns(2)
                        .spacing(Vec2::new(18.0, 8.0))
                        .striped(true)
                        .max_col_width((ui.available_width() - 155.0).max(100.0))
                        .show(ui, |ui| {
                            for parameter in parameters {
                                let name = if parameter.path == "on" {
                                    "开关"
                                } else {
                                    parameter.path.as_str()
                                };
                                ui.add_sized(
                                    [130.0, 20.0],
                                    egui::Label::new(
                                        RichText::new(name).size(12.0).color(palette(ui).muted),
                                    )
                                    .halign(egui::Align::Min)
                                    .truncate(),
                                )
                                .on_hover_text(&parameter.path);
                                let value =
                                    match (parameter.path.as_str(), parameter.value.as_str()) {
                                        ("on", "true") => "开启",
                                        ("on", "false") => "关闭",
                                        _ => parameter.value.as_str(),
                                    };
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(preview_compact(value, 220)).size(12.0),
                                    )
                                    .wrap(),
                                )
                                .on_hover_text(preview_compact(value, 4096));
                                ui.end_row();
                            }
                        });
                });
        });
}
fn preview_peq_bands(ui: &mut egui::Ui, bands: &[super::ncae_preview::core::PeqBand]) {
    if bands.is_empty() {
        return;
    }
    egui::Frame::NONE
        .fill(palette(ui).surface)
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            egui::ScrollArea::both()
                .id_salt("peq_band_table")
                .max_height(178.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("peq_band_grid")
                        .num_columns(7)
                        .spacing(Vec2::new(12.0, 8.0))
                        .striped(true)
                        .min_col_width(36.0)
                        .max_col_width(160.0)
                        .show(ui, |ui| {
                            for label in
                                ["频段", "状态", "频率", "增益", "Q 原值", "类型", "解析状态"]
                            {
                                ui.label(RichText::new(label).size(11.0).color(palette(ui).muted));
                            }
                            ui.end_row();
                            for band in bands {
                                for value in [
                                    &band.band,
                                    preview_state_text(band.state),
                                    &band.frequency,
                                    &band.gain,
                                    &band.q,
                                    &band.type_code,
                                    &band.mapping,
                                ] {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(preview_compact(value, 28)).size(11.0),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(preview_compact(value, 4096));
                                }
                                ui.end_row();
                            }
                        });
                });
        });
}

fn kind_label(kind: PayloadKind) -> &'static str {
    match kind {
        PayloadKind::Wav => "WAV / IR 卷积",
        PayloadKind::Json => "JSON 参数",
        PayloadKind::Binary => "二进制音效",
    }
}

const KNOWN_IR_TEMPLATES: [&str; 5] = [
    "HiFi现场",
    "HiFi电子管",
    "复古收音机",
    "录音棚立体声",
    "震撼全景",
];

fn type_rank(kind: PayloadKind) -> u8 {
    match kind {
        PayloadKind::Wav => 0,
        PayloadKind::Json => 1,
        PayloadKind::Binary => 2,
    }
}

fn sort_indices(effects: &[EffectInfo], indices: &mut [usize], by_type: bool) {
    indices.sort_by(|&a, &b| {
        let left = &effects[a];
        let right = &effects[b];
        let kind = if by_type {
            type_rank(left.kind).cmp(&type_rank(right.kind))
        } else {
            std::cmp::Ordering::Equal
        };
        kind.then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
}

fn matches_search(effect: &EffectInfo, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || effect.name.to_lowercase().contains(&query)
        || effect.detail.to_lowercase().contains(&query)
        || kind_label(effect.kind).to_lowercase().contains(&query)
}

/// Hide generated timestamp suffixes for display only; retain full paths and search keys.
fn display_name(name: &str) -> &str {
    let stem = name.strip_suffix(".ncae").unwrap_or(name);
    match stem.rsplit_once('-') {
        Some((title, suffix))
            if !title.is_empty()
                && suffix.len() >= 10
                && suffix.bytes().all(|c| c.is_ascii_digit()) =>
        {
            title
        }
        _ => stem,
    }
}

fn effect_row(ui: &mut egui::Ui, effect: &EffectInfo, selected: bool) -> egui::Response {
    let response = ui.add_sized(
        [ui.available_width(), 48.0],
        AnimatedButton::new("")
            .selected(selected)
            .fill(if selected {
                palette(ui).tint
            } else {
                palette(ui).surface
            })
            .stroke(Stroke::new(
                1.0,
                if selected {
                    palette(ui).selected_border
                } else {
                    palette(ui).surface
                },
            )),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            &effect.name,
        )
    });

    let (badge, foreground, background) = match effect.kind {
        PayloadKind::Wav => ("IR 卷积", palette(ui).ir_text, palette(ui).ir_bg),
        PayloadKind::Json => ("JSON", palette(ui).json_text, palette(ui).json_bg),
        PayloadKind::Binary => ("未知", palette(ui).muted, palette(ui).neutral_bg),
    };
    let badge_rect = egui::Rect::from_min_size(
        egui::pos2(
            response.rect.right() - 75.0,
            response.rect.center().y - 11.0,
        ),
        Vec2::new(63.0, 22.0),
    );
    ui.painter().rect_filled(badge_rect, 5, background);
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge,
        FontId::proportional(11.0),
        foreground,
    );
    let color = if selected {
        palette(ui).accent
    } else {
        palette(ui).ink
    };
    let mut job = egui::text::LayoutJob::simple(
        display_name(&effect.name).to_owned(),
        FontId::proportional(14.0),
        color,
        (response.rect.width() - 100.0).max(1.0),
    );
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    let top = response.rect.center().y - galley.size().y / 2.0;
    ui.painter()
        .galley(egui::pos2(response.rect.left() + 12.0, top), galley, color);
    response.on_hover_text(format!(
        "{}\n{}\n{}",
        effect.name,
        effect.detail,
        effect.path.display()
    ))
}

pub(super) fn input_problem(value: &str) -> Option<&'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Some("导入音频或 IR 采样数据后即可生成");
    }
    let path = Path::new(value);
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ![
        "wav", "wave", "irs", "flac", "aif", "aiff", "au", "snd", "caf", "w64", "rf64", "json",
        "npz",
    ]
    .iter()
    .any(|ext| extension.eq_ignore_ascii_case(ext))
    {
        return Some("不支持此格式；支线支持音频 IR 和 JSON/NPZ 采样数据");
    }
    None
}

impl NcaeGuiApp {
    pub(super) fn render_workspace(&mut self, ui: &mut egui::Ui) {
        apply_animated_theme(ui);
        egui::Panel::top("app_header")
            .frame(panel_frame(palette(ui).panel, 24, 16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 10, Color32::from_rgb(49, 52, 57));
                    for (index, height) in [10.0, 20.0, 26.0, 15.0, 8.0].iter().enumerate() {
                        let x = rect.left() + 10.0 + index as f32 * 4.5;
                        ui.painter().line_segment(
                            [
                                egui::pos2(x, rect.center().y - height / 2.0),
                                egui::pos2(x, rect.center().y + height / 2.0),
                            ],
                            Stroke::new(2.2, Color32::WHITE),
                        );
                    }
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new("NCAE 多格式支线").size(20.0).strong());
                        muted(ui, "独立实验版本 · 音频与 IR 数据转换");
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme_toggle(ui);
                        if ui
                            .add_enabled(!self.busy(), AnimatedButton::new("导出目录"))
                            .on_hover_text(self.output_dir.display().to_string())
                            .clicked()
                        {
                            self.start_job(Job::OpenDirectory(self.output_dir.clone()), ui.ctx());
                        }
                        if ui
                            .add_enabled(!self.busy(), AnimatedButton::new("备份目录"))
                            .on_hover_text(self.backup_directory().display().to_string())
                            .clicked()
                        {
                            self.start_job(Job::OpenDirectory(self.backup_directory()), ui.ctx());
                        }
                    });
                });
            });

        self.render_log(ui);
        self.render_library(ui);
        self.render_action_bar(ui);
        egui::CentralPanel::default()
            .frame(panel_frame(palette(ui).panel, 28, 18))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("editor_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        self.render_target(ui);
                        self.render_selected_preview(ui);
                        ui.add_space(12.0);
                        ui.add_enabled_ui(!self.busy(), |ui| {
                            self.render_import(ui);
                            ui.add_space(12.0);
                            self.render_settings(ui);
                            ui.add_space(12.0);
                            self.render_safety(ui);
                        });
                        ui.add_space(14.0);
                    });
            });
    }

    fn render_library(&mut self, ui: &mut egui::Ui) {
        let max_width = (ui.available_width() * 0.43).max(280.0);
        egui::Panel::left("effect_library")
            .default_size(350.0)
            .min_size(280.0)
            .max_size(max_width)
            .resizable(true)
            .frame(panel_frame(palette(ui).surface, 18, 18))
            .show(ui, |ui| {
                egui::Panel::bottom("library_location")
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.separator();
                        ui.horizontal(|ui| {
                            muted(ui, "本地音效目录");
                            ui.add_enabled_ui(!self.busy(), |ui| {
                                ui.animated_menu_button("目录设置", |ui| {
                                    ui.set_width(390.0);
                                    ui.label(RichText::new("网易云安装位置").strong());
                                    muted(
                                        ui,
                                        self.install_dir
                                            .as_ref()
                                            .map(|path| path.display().to_string())
                                            .unwrap_or_else(|| "尚未识别安装位置".into()),
                                    );
                                    ui.separator();
                                    ui.label("实际音效目录");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.directory_input)
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "音效通常存放于用户数据目录，不一定在程序安装目录中。",
                                        )
                                        .size(12.0)
                                        .color(palette(ui).muted),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui.animated_button("重新自动检测").clicked() {
                                            self.manual_effect_dir = None;
                                            self.start_job(Job::Refresh(None), ui.ctx());
                                            ui.close();
                                        }
                                        if ui
                                            .add_enabled(
                                                !self.directory_input.trim().is_empty(),
                                                AnimatedButton::new("使用此目录"),
                                            )
                                            .clicked()
                                        {
                                            let path = std::path::PathBuf::from(
                                                self.directory_input.trim(),
                                            );
                                            self.manual_effect_dir = Some(path.clone());
                                            self.start_job(Job::Refresh(Some(path)), ui.ctx());
                                            ui.close();
                                        }
                                    });
                                });
                            });
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(self.effect_dir.display().to_string())
                                    .size(11.0)
                                    .color(palette(ui).muted),
                            )
                            .truncate(),
                        )
                        .on_hover_text(format!(
                            "{}\n{}",
                            self.effect_dir.display(),
                            self.directory_note
                        ));
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.directory_note)
                                    .size(11.0)
                                    .color(palette(ui).accent),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&self.directory_note);
                        ui.add_space(4.0);
                    });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("音效库").size(17.0).strong());
                    muted(ui, format!("{} 个音效", self.effects.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(!self.busy(), AnimatedButton::new("刷新").small())
                            .clicked()
                        {
                            self.start_job(Job::Refresh(self.manual_effect_dir.clone()), ui.ctx());
                        }
                    });
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("搜索名称或类型…")
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10, 8)),
                );
                ui.horizontal(|ui| {
                    muted(ui, "选择音效模板");
                    if !self.search.is_empty() && ui.animated_small_button("清除").clicked() {
                        self.search.clear();
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    muted(ui, "排序");
                    egui::ComboBox::from_id_salt("effect_sort")
                        .selected_text(if self.sort_by_type {
                            "类型优先 · IR 在前"
                        } else {
                            "按名称"
                        })
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            ui.animated_selectable_value(
                                &mut self.sort_by_type,
                                true,
                                "类型优先 · IR 在前",
                            );
                            ui.animated_selectable_value(&mut self.sort_by_type, false, "按名称");
                        });
                });
                let mut visible: Vec<usize> = self
                    .effects
                    .iter()
                    .enumerate()
                    .filter(|(_, effect)| matches_search(effect, &self.search))
                    .map(|(index, _)| index)
                    .collect();
                sort_indices(&self.effects, &mut visible, self.sort_by_type);
                egui::ScrollArea::vertical()
                    .id_salt("library_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.effects.is_empty() {
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new(if self.busy() {
                                    "正在检测本地音效…"
                                } else {
                                    "暂无本地音效"
                                })
                                .strong(),
                            );
                            muted(
                                ui,
                                "请在网易云中下载音效后刷新，或通过底部「目录设置」重新检测位置。",
                            );
                        } else if visible.is_empty() {
                            ui.add_space(24.0);
                            ui.label("没有匹配的音效");
                            muted(ui, "试试其他名称，或清除搜索。");
                        }
                        ui.add_enabled_ui(!self.busy(), |ui| {
                            for index in visible {
                                if effect_row(
                                    ui,
                                    &self.effects[index],
                                    self.selected == Some(index),
                                )
                                .clicked()
                                {
                                    self.selected = Some(index);
                                }
                            }
                        });
                    });
            });
    }

    fn render_json_preview(&mut self, ui: &mut egui::Ui, model: &super::ncae_preview::core::Model) {
        use super::ncae_preview::core::State;
        if !model
            .modules
            .iter()
            .any(|module| module.key == self.preview.module && module.state != State::Missing)
        {
            self.preview.module = model.default_module().unwrap_or("").to_owned();
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for module in &model.modules {
                if module.state == State::Missing {
                    continue;
                }
                let selected = self.preview.module == module.key;
                let color = if selected {
                    palette(ui).ink
                } else if module.state == State::On {
                    palette(ui).accent
                } else {
                    palette(ui).muted
                };
                let response = ui
                    .push_id(("preview_module", &module.key), |ui| {
                        ui.add(
                            AnimatedButton::new(
                                RichText::new(format!(
                                    "{} · {}",
                                    module.label,
                                    preview_state_text(module.state)
                                ))
                                .size(12.0)
                                .color(color),
                            )
                            .selected(selected)
                            .min_size(Vec2::new(64.0, 30.0))
                            .stroke(if selected {
                                Stroke::new(1.0, palette(ui).accent)
                            } else {
                                Stroke::NONE
                            }),
                        )
                    })
                    .inner;
                #[cfg(test)]
                ui.ctx().data_mut(|data| {
                    data.insert_temp(
                        egui::Id::new(("preview_module_test", &module.key)),
                        response.rect,
                    )
                });
                if response.clicked() {
                    self.preview.module = module.key.clone();
                    ui.ctx().request_repaint();
                }
                response.on_hover_text(format!(
                    "查看{}参数 · 当前{}\n仅切换查看内容，不修改开关",
                    module.label,
                    preview_state_text(module.state)
                ));
            }
        });
        ui.add_space(6.0);
        let Some(module) = model.module(&self.preview.module) else {
            muted(ui, "未识别到可预览模块");
            return;
        };
        ui.push_id(("selected_module_view", &module.key), |ui| {
            match module.key.as_str() {
                "eq" => {
                    if module.state == State::On {
                        preview_plot(ui, model, false, Some("eq"));
                        ui.add_space(6.0);
                    }
                    preview_parameters(ui, module);
                }
                "peq" => {
                    let has_curve = model.curves.iter().any(|curve| curve.module == "peq");
                    if module.state == State::On && (has_curve || !model.markers.is_empty()) {
                        if has_curve {
                            ui.animated_checkbox(&mut self.preview.show_markers, "参数点");
                        } else {
                            ui.label(
                                RichText::new("参数分布")
                                    .size(11.0)
                                    .color(palette(ui).muted),
                            )
                            .on_hover_text(
                                "这是频段参数的位置，不是滤波响应；未知数字类型码未参与模型计算。",
                            );
                        }
                        preview_plot(
                            ui,
                            model,
                            self.preview.show_markers || !has_curve,
                            Some("peq"),
                        );
                        ui.add_space(6.0);
                    }
                    preview_parameters(ui, module);
                    ui.add_space(6.0);
                    preview_peq_bands(ui, &model.peq_bands);
                }
                _ => preview_parameters(ui, module),
            }
        });
    }

    fn render_selected_preview(&mut self, ui: &mut egui::Ui) {
        use super::ncae_preview::core::Kind;
        let path = self.selected_effect().map(|effect| effect.path);
        self.preview.update(ui.ctx(), path.clone());
        if path.is_none() {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("当前音效预览").strong().size(14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(model)=self.preview.model.clone() {
                    ui.add(AnimatedButton::new("i").small().frame(false)).on_hover_ui(|ui|{
                        ui.set_max_width(420.0);
                        ui.label(RichText::new("预览说明").strong());
                        if model.kind==Kind::Json {ui.label("EQ 为设置参考，PEQ 为可解析的标准模型；不代表混响等整套音效的实测响应。");}
                        egui::ScrollArea::vertical().id_salt("preview_help_scroll").max_height(180.0).show(ui,|ui|{for warning in &model.warnings{ui.label(RichText::new(warning).size(12.0));}});
                    });
                }
                if ui
                    .animated_small_button(if self.preview.expanded {
                        "收起图表"
                    } else {
                        "展开图表"
                    })
                    .clicked()
                {
                    self.preview.expanded = !self.preview.expanded;
                    ui.ctx().request_repaint();
                }
            });
        });
        if !self.preview.expanded {
            return;
        }
        let previous = self.preview.options;
        if let Some(model) = self.preview.model.clone() {
            match model.kind {
                Kind::Ir => {
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_id_salt("preview_channel")
                            .width(100.0)
                            .selected_text(
                                self.preview
                                    .options
                                    .channel
                                    .map(|channel| format!("声道 {}", channel + 1))
                                    .unwrap_or_else(|| {
                                        if model.channels == 1 {
                                            "单声道".into()
                                        } else {
                                            "前两声道".into()
                                        }
                                    }),
                            )
                            .show_ui(ui, |ui| {
                                ui.animated_selectable_value(
                                    &mut self.preview.options.channel,
                                    None,
                                    "前两声道 / 单声道",
                                );
                                for channel in 0..model.channels {
                                    ui.animated_selectable_value(
                                        &mut self.preview.options.channel,
                                        Some(channel),
                                        format!("声道 {}", channel + 1),
                                    );
                                }
                            });
                        let text = if self.preview.options.smoothing == 0 {
                            "不平滑".into()
                        } else {
                            format!("1/{} 倍频程", self.preview.options.smoothing)
                        };
                        egui::ComboBox::from_id_salt("preview_smoothing")
                            .width(112.0)
                            .selected_text(text)
                            .show_ui(ui, |ui| {
                                for value in [0, 3, 6, 12, 24, 48] {
                                    ui.animated_selectable_value(
                                        &mut self.preview.options.smoothing,
                                        value,
                                        if value == 0 {
                                            "不平滑".into()
                                        } else {
                                            format!("1/{value} 倍频程")
                                        },
                                    );
                                }
                            });
                        ui.animated_checkbox(&mut self.preview.options.normalize, "仅预览归一化")
                            .on_hover_text("各声道显示峰值归一化，不修改音效文件或生成设置");
                    });
                }
                Kind::Json => {}
            }
        }
        if previous != self.preview.options {
            self.preview.update(ui.ctx(), path);
            ui.ctx().request_repaint();
        }
        if let Some(model) = self.preview.model.clone() {
            if model.kind == Kind::Json {
                self.render_json_preview(ui, &model);
            } else {
                preview_plot(ui, &model, false, None);
                ui.add(
                    egui::Label::new(
                        RichText::new(&model.summary)
                            .size(11.0)
                            .color(palette(ui).muted),
                    )
                    .truncate(),
                )
                .on_hover_text(&model.summary);
            }
        } else if let Some(error) = &self.preview.error {
            ui.label(
                RichText::new(format!("预览不可用：{error}"))
                    .size(12.0)
                    .color(palette(ui).warning),
            );
        } else {
            egui::Frame::NONE
                .fill(palette(ui).surface)
                .corner_radius(8)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(220.0);
                    ui.horizontal(|ui| {
                        if self.preview.loading() {
                            ui.spinner();
                        }
                        muted(ui, "正在读取所选音效并计算预览…");
                    });
                });
        }
        ui.add_space(6.0);
    }

    fn render_target(&mut self, ui: &mut egui::Ui) {
        if let Some(effect) = self.selected_effect() {
            ui.add(
                egui::Label::new(
                    RichText::new(display_name(&effect.name))
                        .size(23.0)
                        .strong(),
                )
                .truncate(),
            )
            .on_hover_text(&effect.name);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(kind_label(effect.kind))
                        .size(12.0)
                        .color(palette(ui).accent),
                );
                muted(ui, format!("{:.1} KB", effect.size as f64 / 1024.0));
                muted(ui, &effect.detail);
            });
        } else {
            ui.label(RichText::new("从左侧选择目标音效").size(23.0).strong());
        }
        ui.add_space(8.0);
        ui.separator();
    }

    fn render_import(&mut self, ui: &mut egui::Ui) {
        section(ui, "01", "导入源文件", "支持多种音频 IR、JSON/NPZ 采样数据");
        ui.horizontal_wrapped(|ui| {
            muted(ui, "IR 推荐模板：");
            for name in KNOWN_IR_TEMPLATES {
                let index = self.effects.iter().position(|effect| {
                    effect.kind == PayloadKind::Wav
                        && effect.detail.starts_with("WAV/IR 型:")
                        && display_name(&effect.name) == name
                });
                if ui
                    .add_enabled(
                        index.is_some(),
                        AnimatedButton::new(
                            RichText::new(name).size(12.0).color(palette(ui).accent),
                        )
                        .small()
                        .frame(false),
                    )
                    .on_hover_text("当前文件已识别为有效 WAV/IR，点击选为目标模板")
                    .on_disabled_hover_text(
                        "已确认的卷积音效参考；请先在网易云下载，再刷新并核对本地类型",
                    )
                    .clicked()
                {
                    self.selected = index;
                }
            }
        });
        if let Some(effect) = self.selected_effect() {
            let is_wav = self.input_audio;
            if is_wav && effect.kind != PayloadKind::Wav {
                ui.label(RichText::new("当前目标不是 IR 卷积型，替换 WAV / IRS 建议改选上方模板；跨类型封装不保证客户端兼容。").size(12.0).color(palette(ui).warning));
            }
        }
        let has_input = !self.input_path.trim().is_empty();
        let hovering = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
        let filename = Path::new(self.input_path.trim())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("输入文件")
            .to_owned();
        egui::Frame::NONE
            .fill(if hovering {
                palette(ui).tint
            } else {
                palette(ui).surface
            })
            .stroke(Stroke::new(
                1.0,
                if hovering {
                    palette(ui).accent
                } else {
                    palette(ui).border
                },
            ))
            .corner_radius(10)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 115.0).max(120.0));
                        ui.add(
                            egui::Label::new(
                                RichText::new(if has_input {
                                    &filename
                                } else {
                                    "将文件拖入窗口"
                                })
                                .size(16.0)
                                .strong(),
                            )
                            .truncate(),
                        );
                        muted(
                            ui,
                            if has_input {
                                "源文件已选择，可重新选择或直接编辑下方路径"
                            } else {
                                "也可以选择文件，或在下方粘贴完整路径"
                            },
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.animated_button("选择文件").clicked() {
                            self.start_job(Job::PickFile, ui.ctx());
                        }
                    });
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 64.0).max(80.0);
                    ui.add_sized(
                        [width, 34.0],
                        egui::TextEdit::singleline(&mut self.input_path)
                            .hint_text("音频或 IR 采样数据文件完整路径")
                            .margin(egui::Margin::symmetric(10, 8)),
                    );
                    if ui
                        .add_enabled(has_input, AnimatedButton::new("清空"))
                        .clicked()
                    {
                        self.input_path.clear();
                    }
                });
                if !self.input_path.trim().is_empty() {
                    if let Some(problem) = self.input_readiness() {
                        ui.label(RichText::new(problem).size(12.0).color(palette(ui).error));
                    }
                }
            });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        section(
            ui,
            "02",
            "转换设置",
            "音频 IR 使用以下设置；参数 JSON 保留原值",
        );
        let is_json = Path::new(self.input_path.trim())
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("json"));
        ui.add_enabled_ui(!is_json || self.input_audio, |ui| {
            if ui.available_width() >= 560.0 {
                ui.columns(2, |columns| {
                    self.render_length(&mut columns[0]);
                    self.render_channel(&mut columns[1]);
                });
            } else {
                self.render_length(ui);
                ui.add_space(4.0);
                self.render_channel(ui);
            }
            ui.add_space(4.0);
            ui.animated_checkbox(&mut self.peak_match, "峰值对齐")
                .on_hover_text("匹配原音效峰值，避免转换后音量过小。");
        });
    }

    fn render_length(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("长度模式").strong());
        egui::ComboBox::from_id_salt("length_mode")
            .width(ui.available_width())
            .selected_text(match self.mode {
                LengthMode::Full => "完整版 · 推荐",
                LengthMode::Length => "等长能量窗",
                LengthMode::Start => "仅保留开头",
            })
            .show_ui(ui, |ui| {
                ui.animated_selectable_value(&mut self.mode, LengthMode::Full, "完整版 · 推荐");
                ui.animated_selectable_value(&mut self.mode, LengthMode::Length, "等长能量窗");
                ui.animated_selectable_value(&mut self.mode, LengthMode::Start, "仅保留开头");
            });
        muted(
            ui,
            match self.mode {
                LengthMode::Full => "保留完整响应，不裁剪尾部。",
                LengthMode::Length => "提取能量窗口，匹配原 IR 长度。",
                LengthMode::Start => "从开头截取，可能损失尾部效果。",
            },
        );
    }

    fn render_channel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("声道模式").strong());
        egui::ComboBox::from_id_salt("channel_mode")
            .width(ui.available_width())
            .selected_text(match self.channel_mode {
                ChannelMode::Keep => "保持原声道",
                ChannelMode::Stereo => "双声道",
                ChannelMode::Mono => "单声道",
            })
            .show_ui(ui, |ui| {
                ui.animated_selectable_value(
                    &mut self.channel_mode,
                    ChannelMode::Keep,
                    "保持原声道",
                );
                ui.animated_selectable_value(&mut self.channel_mode, ChannelMode::Stereo, "双声道");
                ui.animated_selectable_value(&mut self.channel_mode, ChannelMode::Mono, "单声道");
            });
        muted(ui, "按输入声道保留，或转换为指定声道。");
    }

    fn render_safety(&mut self, ui: &mut egui::Ui) {
        section(ui, "03", "输出与保护", "");
        ui.animated_checkbox(&mut self.replace_after, "生成后直接替换目标音效");
        ui.add_enabled_ui(self.replace_after, |ui| {
            ui.animated_checkbox(
                &mut self.backup_checked,
                "替换前备份原音效 · 同目录 .bak，首次备份保留",
            );
        });
        if self.replace_after {
            ui.label(
                RichText::new(if self.backup_checked {
                    "将修改客户端文件，请确认替换时机；原备份不会重复覆盖。"
                } else {
                    "未开启备份：替换后可能无法恢复，执行前将再次确认。"
                })
                .size(12.0)
                .color(palette(ui).warning),
            );
        }
        if let Some(output) = &self.last_output {
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(format!("最近文件：{}", output.display()))
                        .size(12.0)
                        .color(palette(ui).accent),
                )
                .truncate(),
            )
            .on_hover_text(output.display().to_string());
        }
        ui.add_space(8.0);
        muted(
            ui,
            "仅用于个人学习及合法拥有的文件。修改客户端资源可能违反用户协议，请先备份。",
        );
    }

    fn render_action_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("actions")
            .frame(panel_frame(palette(ui).panel, 28, 12))
            .show(ui, |ui| {
                if let Some(task) = &self.task {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(task.title).color(palette(ui).accent).strong());
                        muted(
                            ui,
                            format!("已用时 {:.1} 秒", task.started.elapsed().as_secs_f32()),
                        );
                    });
                    ui.label(
                        RichText::new(&task.progress)
                            .size(12.0)
                            .color(palette(ui).muted),
                    );
                    ui.add_space(4.0);
                }
                let selected = self.selected_effect().is_some();
                let problem = if self.busy() {
                    Some("后台任务进行中，请稍候；可展开日志查看阶段记录。".to_owned())
                } else if !selected {
                    Some("请先从左侧选择目标音效".to_owned())
                } else {
                    self.input_readiness()
                };
                ui.horizontal(|ui| {
                    let text = if self.busy() {
                        "正在处理…"
                    } else if self.replace_after {
                        "生成并替换音效"
                    } else {
                        "生成 NCAE 文件"
                    };
                    let button =
                        AnimatedButton::new(RichText::new(text).color(Color32::WHITE).strong())
                            .fill(palette(ui).primary)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(160.0, 40.0));
                    if ui
                        .add_enabled(problem.is_none(), button)
                        .on_disabled_hover_text(problem.as_deref().unwrap_or(""))
                        .clicked()
                    {
                        self.request_generate(ui.ctx());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(!self.busy() && (selected || self.wav_conversion_source().is_some()), |ui| {
                            ui.animated_menu_button("更多操作", |ui| {
                                ui.set_min_width(232.0);
                                ui.spacing_mut().button_padding = Vec2::new(14.0, 9.0);
                                ui.spacing_mut().interact_size.y = 36.0;
                                ui.spacing_mut().item_spacing.y = 4.0;
                                let wav_source = self.wav_conversion_source();
                                if ui.add_enabled(wav_source.is_some(), AnimatedButton::new("源文件转为 WAV 格式"))
                                    .on_hover_text("转换当前输入文件；输入为空时导出所选 IR 音效。保留采样率、声道、长度及音量。")
                                    .on_disabled_hover_text("需要有效音频 IR、ir.samples.v1 数据或 WAV/IR 型 NCAE；参数 JSON 不能转为音频。")
                                    .clicked() {
                                    if let Some(source) = wav_source { self.start_job(Job::ConvertToWav { source, output_dir: self.output_dir.clone() }, ui.ctx()); }
                                    ui.close();
                                }
                                ui.add_space(4.0); ui.separator(); ui.add_space(4.0);
                                if ui.add_enabled(selected, AnimatedButton::new("解密当前音效并导出")).clicked() {
                                    ui.close();
                                    if let Some(effect) = self.selected_effect() {
                                        self.start_job(
                                            Job::Decrypt {
                                                path: effect.path,
                                                output_dir: self.output_dir.clone(),
                                            },
                                            ui.ctx(),
                                        );
                                    }
                                }
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);
                                if ui
                                    .add_enabled(selected, AnimatedButton::new(RichText::new("恢复原音效…").color(palette(ui).warning)))
                                    .clicked()
                                {
                                    ui.close();
                                    self.request_restore();
                                }
                            });
                        });
                    });
                });
                muted(
                    ui,
                    problem.as_deref().unwrap_or(if self.replace_after {
                        "即将替换目标文件；请确认备份与转换设置。"
                    } else {
                        "新文件保存至程序 generated 目录，原音效保持不变。"
                    }),
                );
            });
    }

    pub(super) fn render_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(pending) = &self.confirmation {
            let title = pending.title;
            let message = pending.message.clone();
            let mut confirmed = false;
            let mut cancelled = false;
            let response = egui::Modal::new(egui::Id::new("confirm_operation"))
                .frame(dialog_frame(ctx))
                .show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.label(RichText::new(title).size(20.0).strong());
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(RichText::new(message).size(14.0)).wrap());
                    ui.add_space(20.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        confirmed = ui
                            .add_sized(
                                [108.0, 38.0],
                                AnimatedButton::new(
                                    RichText::new("确认继续")
                                        .color(palette(ui).warning)
                                        .strong(),
                                ),
                            )
                            .clicked();
                        cancelled = ui
                            .add_sized([88.0, 38.0], AnimatedButton::new("取消"))
                            .clicked();
                    });
                });
            if confirmed {
                if let Some(pending) = self.confirmation.take() {
                    self.start_job(pending.job, ctx);
                }
            } else if cancelled || response.should_close() {
                self.confirmation = None;
                self.log("已取消操作，未修改音效文件。");
            }
        }
        if let Some(message) = self.notice.clone() {
            let mut dismiss = false;
            let response = egui::Modal::new(egui::Id::new("operation_notice"))
                .frame(dialog_frame(ctx))
                .show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.label(
                        RichText::new(
                            if message.starts_with("解密导出完成")
                                || message.starts_with("WAV 导出完成")
                            {
                                "导出完成"
                            } else {
                                "操作提示"
                            },
                        )
                        .size(20.0)
                        .strong(),
                    );
                    ui.add_space(10.0);
                    ui.add(egui::Label::new(RichText::new(message).size(14.0)).wrap());
                    ui.add_space(20.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        dismiss = ui
                            .add_sized(
                                [104.0, 38.0],
                                AnimatedButton::new(
                                    RichText::new("知道了").color(Color32::WHITE).strong(),
                                )
                                .fill(palette(ui).primary)
                                .stroke(Stroke::NONE),
                            )
                            .clicked();
                    });
                });
            if dismiss || response.should_close() {
                self.notice = None;
            }
        }
    }

    fn render_log(&mut self, ui: &mut egui::Ui) {
        // The panel and its contents must use the same start-of-frame state.
        // Otherwise closing it stores the header-only size as the expanded height.
        let expanded_at_start = self.show_log;
        let panel = if expanded_at_start {
            egui::Panel::bottom("activity_log_expanded")
                .resizable(true)
                .default_size(log_viewport_height(ui) + 72.0)
                .min_size(104.0)
                .max_size((ui.available_height() * 0.60).max(104.0))
        } else {
            egui::Panel::bottom("activity_log_collapsed").resizable(false)
        };
        panel
            .frame(panel_frame(palette(ui).surface, 22, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let toggle = ui.animated_selectable_label(
                        expanded_at_start,
                        if expanded_at_start {
                            "收起日志"
                        } else {
                            "运行日志"
                        },
                    );
                    #[cfg(test)]
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new("log_toggle_test_rect"), toggle.rect)
                    });
                    if toggle.clicked() {
                        self.show_log = !expanded_at_start;
                        ui.ctx().request_repaint();
                    }
                    if let Some(last) = self.log.last() {
                        let color = if last.contains("失败") {
                            palette(ui).error
                        } else {
                            palette(ui).muted
                        };
                        ui.add(
                            egui::Label::new(RichText::new(last).size(12.0).color(color))
                                .truncate(),
                        )
                        .on_hover_text(last);
                    }
                });
                if expanded_at_start {
                    ui.separator();
                    log_viewport_sized(ui, &self.log, ui.available_height().max(18.0));
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_unicode_names_and_type_case_insensitively() {
        let effect = EffectInfo {
            path: "test.ncae".into(),
            name: "音乐厅".into(),
            size: 42,
            kind: PayloadKind::Wav,
            detail: "48000 Hz".into(),
        };
        assert!(matches_search(&effect, " 音乐 "));
        assert!(matches_search(&effect, "wav"));
        assert!(matches_search(&effect, "48000"));
        assert!(matches_search(&effect, ""));
        assert!(!matches_search(&effect, "JSON"));
    }
    #[test]
    fn display_names_remove_only_generated_suffixes() {
        assert_eq!(display_name("音乐厅-1791473965902.ncae"), "音乐厅");
        assert_eq!(display_name("Hall-2.ncae"), "Hall-2");
        assert_eq!(display_name("Room-A.ncae"), "Room-A");
        assert_eq!(display_name("-1791473965902.ncae"), "-1791473965902");
    }
    #[test]
    fn input_validation_rejects_missing_or_unsupported_files() {
        assert_eq!(input_problem(" "), Some("导入音频或 IR 采样数据后即可生成"));
        assert_eq!(
            input_problem("file.mp3"),
            Some("不支持此格式；支线支持音频 IR 和 JSON/NPZ 采样数据")
        );
        // Filesystem validation is deliberately deferred to the background worker.
        assert_eq!(input_problem("__missing_input__.WAV"), None);
    }
    #[test]
    fn sorting_prioritizes_ir_then_json_without_changing_selection_indices() {
        let make = |name: &str, kind| EffectInfo {
            path: name.into(),
            name: name.into(),
            size: 1,
            kind,
            detail: String::new(),
        };
        let effects = vec![
            make("A-json", PayloadKind::Json),
            make("Z-ir", PayloadKind::Wav),
            make("B-unknown", PayloadKind::Binary),
            make("C-ir", PayloadKind::Wav),
        ];
        let mut indices = vec![0, 1, 2, 3];
        sort_indices(&effects, &mut indices, true);
        assert_eq!(indices, vec![3, 1, 0, 2]);
        sort_indices(&effects, &mut indices, false);
        assert_eq!(indices, vec![0, 2, 3, 1]);
    }

    #[test]
    fn system_theme_switches_both_palettes_live() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        for theme in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
            for _ in 0..2 {
                let _ = ctx.run_ui(
                    egui::RawInput {
                        system_theme: Some(theme),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(1180.0, 860.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let p = palette(ui);
                        assert_eq!(ui.visuals().dark_mode, theme == egui::Theme::Dark);
                        assert_eq!(ui.visuals().panel_fill, p.panel);
                        assert_eq!(ui.visuals().override_text_color, Some(p.ink));
                    },
                );
            }
            assert_eq!(ctx.theme(), theme);
        }
    }

    #[test]
    fn expanded_log_reserves_five_lines_with_smaller_wrapping_text() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        for count in [0, 2, 6, 50] {
            let lines: Vec<String> = (0..count).map(|i| format!("Log entry {i}")).collect();
            for _ in 0..2 {
                let _ = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(900.0, 640.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        let expected = log_viewport_height(ui);
                        let height = log_viewport(ui, &lines);
                        assert!((height - expected).abs() < 1.0, "log viewport was {height}");
                    },
                );
            }
        }
    }

    #[test]
    fn dialogs_and_expanded_logs_render_in_both_themes() {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let ctx = egui::Context::default();
            setup_style(&ctx);
            let mut app = NcaeGuiApp::empty("unused".into(), "unused".into());
            app.show_log = true;
            app.log = (0..20).map(|index| format!("测试日志 {index}")).collect();
            app.notice =
                Some("替换完成。请在网易云中切换到其他音效，再切回该音效体验效果。".into());
            for size in [[900.0, 640.0], [1180.0, 860.0]] {
                let output = ctx.run_ui(
                    egui::RawInput {
                        system_theme: Some(theme),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(size[0], size[1]),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        app.render_workspace(ui);
                        app.render_dialogs(ui.ctx());
                    },
                );
                assert!(!output.shapes.is_empty());
                assert_eq!(dialog_frame(&ctx).inner_margin, egui::Margin::same(24));
            }
        }
    }

    #[test]
    fn theme_transition_is_smooth_and_reversible() {
        let transition = ThemeTransition {
            from: 0.0,
            to: 1.0,
            started: 1.0,
        };
        assert_eq!(transition.sample(1.0), 0.0);
        assert!((transition.sample(1.125) - 0.5).abs() < 0.0001);
        assert_eq!(transition.sample(1.25), 1.0);
        let reverse = ThemeTransition {
            from: transition.sample(1.1),
            to: 0.0,
            started: 1.1,
        };
        assert_eq!(reverse.sample(1.1), transition.sample(1.1));
        assert_eq!(reverse.sample(1.4), 0.0);
    }

    #[test]
    fn entire_ui_and_popup_style_share_the_animated_palette() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        ctx.set_theme(egui::Theme::Light);
        let render = |time: f64| {
            let mut color = Color32::TRANSPARENT;
            let _ = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1180.0, 860.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    apply_animated_theme(ui);
                    color = palette(ui).panel;
                    assert_eq!(color, ui.visuals().panel_fill);
                    assert_eq!(color, ctx.global_style().visuals.window_fill);
                },
            );
            color
        };
        let light = render(0.0);
        ctx.set_theme(egui::Theme::Dark);
        assert_eq!(render(1.0), light);
        let middle = render(1.125);
        assert_ne!(middle, light);
        assert_ne!(middle, Palette::for_dark(true).panel);
        assert_eq!(render(1.3), Palette::for_dark(true).panel);
    }

    #[test]
    fn repeated_log_toggle_preserves_default_and_user_resized_height() {
        fn frame(
            ctx: &egui::Context,
            app: &mut NcaeGuiApp,
            time: &mut f64,
            events: Vec<egui::Event>,
        ) {
            *time += 0.05;
            let _ = ctx.run_ui(
                egui::RawInput {
                    time: Some(*time),
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(900.0, 640.0),
                    )),
                    ..Default::default()
                },
                |ui| app.render_log(ui),
            );
        }
        fn click(ctx: &egui::Context, app: &mut NcaeGuiApp, time: &mut f64) {
            let position = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("log_toggle_test_rect")))
                .unwrap()
                .center();
            for pressed in [true, false] {
                frame(
                    ctx,
                    app,
                    time,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ],
                );
            }
        }
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut time = 0.0;
        let mut app = NcaeGuiApp::empty("unused".into(), "unused".into());
        app.show_log = true;
        app.log = (0..30).map(|i| format!("Log {i}")).collect();
        for _ in 0..3 {
            frame(&ctx, &mut app, &mut time, vec![]);
        }
        let panel = egui::Id::new("activity_log_expanded");
        let original = egui::containers::panel::PanelState::load(&ctx, panel)
            .unwrap()
            .size()
            .y;
        assert!(original > 120.0);
        for expected in [original, 300.0] {
            if expected == 300.0 {
                ctx.data_mut(|data| {
                    data.insert_persisted(
                        panel,
                        egui::containers::panel::PanelState {
                            outer_rect: egui::Rect::from_min_max(
                                egui::pos2(0.0, 340.0),
                                egui::pos2(900.0, 640.0),
                            ),
                        },
                    )
                });
                frame(&ctx, &mut app, &mut time, vec![]);
            }
            for _ in 0..3 {
                click(&ctx, &mut app, &mut time);
                assert!(!app.show_log);
                frame(&ctx, &mut app, &mut time, vec![]);
                click(&ctx, &mut app, &mut time);
                assert!(app.show_log);
                frame(&ctx, &mut app, &mut time, vec![]);
                let height = egui::containers::panel::PanelState::load(&ctx, panel)
                    .unwrap()
                    .size()
                    .y;
                assert!(
                    (height - expected).abs() < 1.0,
                    "height changed from {expected} to {height}"
                );
            }
        }
    }

    #[test]
    fn hover_gradient_animates_in_and_out() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut time = 0.0;
        let mut button_id = egui::Id::NULL;
        let mut rect = egui::Rect::NOTHING;
        let mut render = |position: Option<egui::Pos2>| {
            time += 0.04;
            let events = position
                .map(|position| vec![egui::Event::PointerMoved(position)])
                .unwrap_or_default();
            let _ = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(900.0, 640.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let response =
                        ui.add(AnimatedButton::new("Test").min_size(Vec2::new(100.0, 36.0)));
                    button_id = response.id;
                    rect = response.rect;
                },
            );
            (
                rect,
                ctx.data(|data| data.get_temp::<f32>(button_id.with("hover_test_amount")))
                    .unwrap(),
            )
        };
        let (rect, initial) = render(None);
        assert_eq!(initial, 0.0);
        let mut entering = Vec::new();
        for _ in 0..8 {
            entering.push(render(Some(rect.center())).1);
        }
        assert!(entering.iter().any(|amount| *amount > 0.0 && *amount < 1.0));
        assert!(entering.last().unwrap() > &0.99);
        let mut leaving = Vec::new();
        for _ in 0..8 {
            leaving.push(render(Some(egui::pos2(700.0, 500.0))).1);
        }
        assert!(leaving.iter().any(|amount| *amount > 0.0 && *amount < 1.0));
        assert!(leaving.last().unwrap() < &0.01);
    }

    #[test]
    fn module_tabs_switch_read_only_details_without_changing_effect_state() {
        let model = super::super::ncae_preview::core::json_model(
            br#"{"eq":{"on":true,"eqs":[0,0,0,0,0,0,0,0,0,0]},"rvb":{"on":false,"room":35}}"#,
            72,
        )
        .unwrap();
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let mut app = NcaeGuiApp::empty("unused".into(), "unused".into());
        let mut time = 0.0;
        let mut frame = |events: Vec<egui::Event>| {
            time += 0.05;
            let _ = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(900.0, 640.0),
                    )),
                    ..Default::default()
                },
                |ui| app.render_json_preview(ui, &model),
            );
        };
        frame(vec![]);
        frame(vec![]);
        let position = ctx
            .data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new((
                    "preview_module_test",
                    String::from("rvb"),
                )))
            })
            .unwrap()
            .center();
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ]);
        }
        drop(frame);
        assert_eq!(app.preview.module, "rvb");
        assert!(app.task.is_none());
        assert_eq!(
            model.module("rvb").unwrap().state,
            super::super::ncae_preview::core::State::Off
        );
    }
}
