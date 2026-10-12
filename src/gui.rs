//! Native, GPU-rendered egui presentation. Lighting remains in the existing engine.
use crate::output::View;
use eframe::egui::{self, Color32, RichText, Vec2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const ACCENT: Color32 = Color32::from_rgb(77, 208, 190);
pub const MUTED: Color32 = Color32::from_rgb(146, 160, 181);
pub const CARD: Color32 = Color32::from_rgb(23, 30, 42);
pub const BACKGROUND: Color32 = Color32::from_rgb(12, 17, 26);

#[derive(Default)]
pub struct Canvas {
    pub texture: Option<(Instant, egui::TextureHandle)>,
}
pub struct App {
    state: Arc<Mutex<View>>,
    canvas: Canvas,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, state: Arc<Mutex<View>>) -> Self {
        theme(&cc.egui_ctx);
        {
            let mut view = state.lock().unwrap_or_else(|e| e.into_inner());
            view.context = Some(cc.egui_ctx.clone());
            if let Ok(handle) = cc.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                view.hwnd = handle.hwnd.get();
            }
        }
        Self {
            state,
            canvas: Canvas::default(),
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut view = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !view.alive {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        screen(ctx, &mut view, &mut self.canvas);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).alive = false;
    }
}
pub fn theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = CARD;
    style.visuals.extreme_bg_color = Color32::from_rgb(14, 21, 33);
    style.visuals.selection.bg_fill = Color32::from_rgb(28, 94, 91);
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(230, 237, 246);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(34, 44, 60);
    style.visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(225, 233, 244);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(46, 62, 79);
    style.spacing.item_spacing = Vec2::new(12.0, 16.0);
    style.spacing.button_padding = Vec2::new(18.0, 10.0);
    style.spacing.interact_size.y = 36.0;
    style.spacing.slider_width = 165.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(28.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
    ctx.set_style(style);
}
pub fn primary(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(BACKGROUND)).fill(ACCENT))
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(14)
        .inner_margin(24)
}
pub fn title(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.heading(title);
    ui.label(RichText::new(description).color(MUTED));
    ui.add_space(12.0);
}
pub fn screen(ctx: &egui::Context, view: &mut View, canvas: &mut Canvas) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Backlight").size(24.0).strong());
                ui.label(
                    RichText::new(if view.setup.open {
                        " / Setup"
                    } else if view.profiles.open {
                        " / Sampling profiles"
                    } else if view.editor.open {
                        " / Sampling"
                    } else {
                        " / Lighting"
                    })
                    .color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Screen lighting").small().color(ACCENT));
                });
            });
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(16.0);
            if view.setup.open {
                egui::ScrollArea::vertical().show(ui, |ui| view.setup.modern_ui(ui));
            } else if view.profiles.open {
                egui::ScrollArea::vertical().show(ui, |ui| view.profiles.modern_ui(ui));
            } else if view.editor.open {
                ui.horizontal(|ui| {
                    ui.heading("Edit sampling");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary(ui, "Done").clicked() {
                            view.editor.open = false;
                        }
                        if ui
                            .button(format!("Profile: {}", view.profiles.active))
                            .clicked()
                        {
                            view.profiles.begin();
                        }
                    });
                });
                ui.add_space(8.0);
                view.editor.modern_ui(ui, canvas, &view.colours, view.raw);
            } else {
                lighting(ui, view);
            }
        });
}
fn lighting(ui: &mut egui::Ui, view: &mut View) {
    ui.horizontal(|ui| {
        title(ui, "Live lighting", "Follow the screen in real time.");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Setup").clicked() {
                view.setup.begin();
            }
            if ui.button("Edit sampling").clicked() {
                view.editor.open = true;
            }
        });
    });
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(335.0, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        card().show(ui, |ui| {
                            ui.set_width(287.0);
                            ui.label(RichText::new("Brightness").size(20.0).strong());
                            if let Some(settings) = &mut view.settings {
                                ui.checkbox(&mut settings.adaptive, "Adaptive brightness");
                                if settings.adaptive {
                                    let mut automatic = view.effective_brightness;
                                    ui.add_enabled(
                                        false,
                                        egui::Slider::new(&mut automatic, 0.0..=2.0)
                                            .fixed_decimals(2),
                                    );
                                } else {
                                    ui.add(
                                        egui::Slider::new(&mut settings.brightness, 0.0..=2.0)
                                            .fixed_decimals(2)
                                            .step_by(0.01),
                                    );
                                }
                                if settings.adaptive {
                                    ui.label(
                                        RichText::new(format!(
                                            "Current scale  {:.2}",
                                            view.effective_brightness
                                        ))
                                        .small()
                                        .color(MUTED),
                                    );
                                }
                                ui.add_space(12.0);
                                ui.separator();
                                ui.label(RichText::new("Response").size(20.0).strong());
                                ui.label("Update rate");
                                ui.add(egui::Slider::new(&mut settings.fps, 1..=240).suffix(" Hz"));
                                ui.label("Blending delay");
                                ui.add(
                                    egui::Slider::new(&mut settings.smoothing_ms, 0..=500)
                                        .suffix(" ms"),
                                );
                                ui.label(
                                    RichText::new("0 ms responds instantly")
                                        .small()
                                        .color(MUTED),
                                );
                                ui.separator();
                                ui.checkbox(
                                    &mut settings.whiteness_correction,
                                    "Whiteness correction",
                                )
                                .on_hover_text(
                                    "Correct a blue tint in neutral white and grey colours.",
                                );
                            } else {
                                ui.spinner();
                            }
                        });
                    });
            },
        );
        ui.vertical(|ui| {
            ui.label(RichText::new("Colour preview").size(20.0).strong());
            ui.label(
                RichText::new("The same colours sent to your LEDs")
                    .small()
                    .color(MUTED),
            );
            let width = ui.available_width().max(180.0);
            let height = (width / 1.9).min(ui.available_height() - 50.0).max(160.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
            colour_frame(ui.painter(), rect, &view.colours, view.counts, view.raw);
            ui.label(
                RichText::new(format!("{} LEDs", view.setup.led_count()))
                    .small()
                    .color(MUTED),
            )
            .on_hover_text(&view.status);
        });
    });
}
pub fn colour_frame(
    p: &egui::Painter,
    rect: egui::Rect,
    colours: &[crate::colour::Rgb],
    counts: [usize; 4],
    raw: bool,
) {
    let thickness = (rect.width() * 0.04).clamp(10.0, 24.0);
    p.rect_filled(rect, 14.0, Color32::from_rgb(7, 11, 18));
    let mut offset = 0;
    for (edge, count) in counts.into_iter().enumerate() {
        for i in 0..count {
            let Some(c) = colours.get(offset + i) else {
                continue;
            };
            let a = i as f32 / count as f32;
            let b = (i + 1) as f32 / count as f32;
            let segment = match edge {
                0 => egui::Rect::from_min_max(
                    egui::pos2(rect.left() + a * rect.width(), rect.top()),
                    egui::pos2(rect.left() + b * rect.width(), rect.top() + thickness),
                ),
                1 => egui::Rect::from_min_max(
                    egui::pos2(rect.right() - thickness, rect.top() + a * rect.height()),
                    egui::pos2(rect.right(), rect.top() + b * rect.height()),
                ),
                2 => egui::Rect::from_min_max(
                    egui::pos2(rect.right() - b * rect.width(), rect.bottom() - thickness),
                    egui::pos2(rect.right() - a * rect.width(), rect.bottom()),
                ),
                _ => egui::Rect::from_min_max(
                    egui::pos2(rect.left(), rect.bottom() - b * rect.height()),
                    egui::pos2(rect.left() + thickness, rect.bottom() - a * rect.height()),
                ),
            };
            let color = Color32::from_rgb(c.0, c.1, c.2);
            if !raw {
                p.rect_filled(segment.expand(8.0), 6.0, color.gamma_multiply(0.2));
            }
            p.rect_filled(segment, 0.0, color);
        }
        offset += count;
    }
}

#[cfg(test)]
#[path = "gui_tests.rs"]
mod tests;
