#[cfg(test)]
use crate::controls::text;
use crate::{
    capture::Frame,
    layout::Region,
    sampling::{Profile, Rect},
};
use std::time::{Duration, Instant};

const COLOURS: [u32; 4] = [0x44cfff, 0xffbb44, 0xee77ee, 0x66ee99];
#[derive(Clone, Copy)]
enum Drag {
    Move(Rect, f32, f32),
    Resize(Rect),
    New(f32, f32),
    Draw(f32, f32),
}
pub struct Editor {
    pub open: bool,
    pub profile: Option<Profile>,
    pub dirty: bool,
    preset: Option<Profile>,
    selected: usize,
    parts: bool,
    part: Option<usize>,
    pub counts: [usize; 4],
    adding: bool,
    drag: Option<Drag>,
    was_down: bool,
    undo_history: Vec<Option<Profile>>,
    action_before: Option<Option<Profile>>,
    picture: Region,
    thumbnail: Vec<u32>,
    next_picture: Instant,
    readings: Vec<Region>,
    reading_rects: Vec<Rect>,
    blocked: Vec<bool>,
    pub view_size: (usize, usize),
    native_picture: Vec<u32>,
    native_size: (usize, usize),
    part_handle_half: (f32, f32),
    reset_pending: bool,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            open: false,
            profile: None,
            dirty: false,
            preset: None,
            selected: 0,
            parts: false,
            part: None,
            counts: crate::strip::COUNTS,
            adding: false,
            drag: None,
            was_down: false,
            undo_history: vec![],
            action_before: None,
            picture: Region {
                x: 30,
                y: 108,
                width: 740,
                height: 230,
            },
            thumbnail: vec![0; 800 * 400],
            next_picture: Instant::now(),
            readings: vec![],
            reading_rects: vec![],
            blocked: vec![],
            view_size: (1600, 800),
            native_picture: vec![],
            native_size: (0, 0),
            part_handle_half: (8.0, 8.0),
            reset_pending: false,
        }
    }
}
impl Editor {
    pub fn new(counts: [usize; 4]) -> Self {
        Self {
            counts,
            ..Self::default()
        }
    }
    pub fn modern_ui(
        &mut self,
        ui: &mut eframe::egui::Ui,
        cache: &mut crate::gui::Canvas,
        colours: &[crate::colour::Rgb],
        raw: bool,
    ) {
        ui.add_enabled_ui(!self.reset_pending, |ui| {
            self.modern_content(ui, cache, colours, raw)
        });
        if self.reset_pending {
            eframe::egui::Modal::new(eframe::egui::Id::new("confirm-sampling-reset"))
                .frame(crate::gui::card().inner_margin(28))
                .show(ui.ctx(), |ui| {
                ui.set_width(440.0);
                ui.heading("Reset sampling?");
                ui.add_space(12.0);
                ui.label("Restore the edge sections and remove all exclusions and individual LED adjustments in this profile.");
                ui.add_space(28.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() { self.resolve_reset(false); }
                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                        if crate::gui::primary(ui,"Reset sampling").clicked() { self.resolve_reset(true); }
                    });
                });
            });
        }
    }
    fn resolve_reset(&mut self, confirmed: bool) {
        self.reset_pending = false;
        if confirmed {
            self.input(None, false);
            self.input(Some((689.0, 70.0)), true);
            self.input(None, false);
        }
    }
    #[cfg(test)]
    pub fn preview_reset_dialog(&mut self) {
        self.reset_pending = true;
    }
    fn modern_content(
        &mut self,
        ui: &mut eframe::egui::Ui,
        cache: &mut crate::gui::Canvas,
        colours: &[crate::colour::Rgb],
        raw: bool,
    ) {
        use eframe::egui::{self, Color32, Stroke, StrokeKind};
        if !self.reset_pending && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.undo();
        }
        ui.horizontal_wrapped(|ui| {
            for (index, label) in [
                "Top",
                "Right",
                "Bottom",
                "Left",
                "Individual LEDs",
                "Exclude area",
                "Delete",
                "Reset",
            ]
            .into_iter()
            .enumerate()
            {
                let selected = self.toolbar_selected(index);
                let response = ui.add_enabled(
                    index != 6 || self.can_delete(),
                    egui::Button::selectable(selected, label),
                );
                if response.clicked() {
                    if index == 7 {
                        self.reset_pending = true;
                    } else {
                        self.input(Some((45.0 + index as f32 * 92.0, 70.0)), true);
                        self.input(None, false);
                    }
                }
            }
        });
        let hint = if self.adding {
            "Draw an exclusion over the area to ignore.".to_owned()
        } else if self.parts {
            self.part
                .map(|index| {
                    let mut offset = 0;
                    for (edge, count) in self.counts.iter().enumerate() {
                        if index < offset + count {
                            return format!(
                                "{} · Reading {}",
                                ["Top", "Right", "Bottom", "Left"][edge],
                                index - offset + 1
                            );
                        }
                        offset += count;
                    }
                    "Select a reading".into()
                })
                .unwrap_or_else(|| "Select an individual reading box".into())
        } else if self.blocked.contains(&true) {
            "Fully blocked readings are shown in red.".to_owned()
        } else {
            String::new()
        };
        // Reserve this row even when empty so mode changes never resize the preview.
        let (hint_row, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
        ui.painter().text(
            hint_row.left_center(),
            egui::Align2::LEFT_CENTER,
            hint,
            egui::FontId::proportional(16.0),
            crate::gui::MUTED,
        );
        ui.add_space(8.0);
        let available = ui.available_size().max(egui::vec2(100.0, 100.0));
        let (canvas, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        ui.painter()
            .rect_filled(canvas, 12.0, Color32::from_rgb(7, 11, 18));
        crate::gui::colour_frame(ui.painter(), canvas, colours, self.counts, raw);
        let bounds = canvas.shrink(32.0);
        let aspect = self.picture.width as f32 / self.picture.height as f32;
        let size = if bounds.width() / bounds.height() > aspect {
            egui::vec2(bounds.height() * aspect, bounds.height())
        } else {
            egui::vec2(bounds.width(), bounds.width() / aspect)
        };
        let picture = egui::Rect::from_center_size(bounds.center(), size);
        let part_handle_size = (8.0 * picture.width() / self.picture.width as f32).max(10.0);
        self.part_handle_half = (
            part_handle_size * self.picture.width as f32 / (2.0 * picture.width()),
            part_handle_size * self.picture.height as f32 / (2.0 * picture.height()),
        );
        let ppp = ui.ctx().pixels_per_point();
        self.view_size = (
            (picture.width() * ppp * 800.0 / self.picture.width as f32) as usize,
            (picture.height() * ppp * 400.0 / self.picture.height as f32) as usize,
        );
        let (nw, nh) = self.native_size;
        if nw > 0 && nh > 0 {
            if cache
                .texture
                .as_ref()
                .is_none_or(|(stamp, _)| *stamp != self.next_picture)
            {
                let pixels: Vec<_> = self
                    .native_picture
                    .iter()
                    .map(|&v| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
                    .collect();
                let image = egui::ColorImage::new([nw, nh], pixels);
                if let Some((stamp, texture)) = &mut cache.texture {
                    texture.set(image, egui::TextureOptions::LINEAR);
                    *stamp = self.next_picture;
                } else {
                    cache.texture = Some((
                        self.next_picture,
                        ui.ctx().load_texture(
                            "sampling-picture",
                            image,
                            egui::TextureOptions::LINEAR,
                        ),
                    ));
                }
            }
            if let Some((_, texture)) = &cache.texture {
                ui.painter().image(
                    texture.id(),
                    picture,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        } else {
            ui.painter().text(
                picture.center(),
                egui::Align2::CENTER_CENTER,
                "Waiting for screen capture…",
                egui::FontId::proportional(16.0),
                crate::gui::MUTED,
            );
        }
        let map = |r: Rect| {
            egui::Rect::from_min_max(
                egui::pos2(
                    picture.left() + r.0 as f32 / 10000.0 * picture.width(),
                    picture.top() + r.1 as f32 / 10000.0 * picture.height(),
                ),
                egui::pos2(
                    picture.left() + r.2 as f32 / 10000.0 * picture.width(),
                    picture.top() + r.3 as f32 / 10000.0 * picture.height(),
                ),
            )
        };
        for (i, r) in self.reading_rects.iter().enumerate() {
            ui.painter().rect_stroke(
                map(*r),
                0.0,
                Stroke::new(
                    1.0_f32,
                    if self.blocked.get(i) == Some(&true) {
                        Color32::from_rgb(248, 91, 106)
                    } else {
                        Color32::from_white_alpha(160)
                    },
                ),
                StrokeKind::Inside,
            );
        }
        if let Some(profile) = self.current() {
            for (i, r) in profile.bands.iter().chain(&profile.exclusions).enumerate() {
                let v = if i < 4 { COLOURS[i] } else { 0xff5555 };
                let color = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                let rect = map(*r);
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(
                        if !self.parts && i == self.selected {
                            2.0
                        } else {
                            1.0_f32
                        },
                        color,
                    ),
                    StrokeKind::Inside,
                );
                if !self.parts && i == self.selected {
                    let size = (8.0 * picture.width() / self.picture.width as f32).max(10.0);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            rect.right_bottom() - egui::vec2(size / 2.0, size / 2.0),
                            egui::vec2(size, size),
                        ),
                        2.0,
                        color,
                    );
                }
            }
        }
        if self.parts
            && let Some(rect) = self.rect()
        {
            let rect = map(rect);
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(2.0_f32, Color32::YELLOW),
                StrokeKind::Inside,
            );
            ui.painter().rect_filled(
                egui::Rect::from_center_size(
                    rect.right_bottom(),
                    egui::vec2(part_handle_size, part_handle_size),
                ),
                2.0,
                Color32::YELLOW,
            );
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let down = ui.input(|i| i.pointer.primary_down());
        let inside = pointer.is_some_and(|pos| canvas.contains(pos));
        let dragging = self.drag.is_some();
        let logical = pointer.filter(|_| inside || dragging).map(|pos| {
            (
                self.picture.x as f32
                    + (pos.x - picture.left()) * self.picture.width as f32 / picture.width(),
                self.picture.y as f32
                    + (pos.y - picture.top()) * self.picture.height as f32 / picture.height(),
            )
        });
        if self.reset_pending {
            self.input(None, false);
        } else {
            self.input(logical, down && (inside || dragging));
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(if self.adding {
                egui::CursorIcon::Crosshair
            } else {
                egui::CursorIcon::Default
            });
        }
    }
    fn can_delete(&self) -> bool {
        if self.adding {
            return false;
        }
        if self.parts {
            return self.part.is_some_and(|index| {
                self.current()
                    .is_some_and(|profile| profile.parts.contains_key(&index))
            });
        }
        self.selected.checked_sub(4).is_some_and(|index| {
            self.current()
                .is_some_and(|profile| index < profile.exclusions.len())
        })
    }
    fn toolbar_selected(&self, index: usize) -> bool {
        match index {
            0..=3 => !self.adding && !self.parts && self.selected == index,
            4 => !self.adding && self.parts,
            5 => self.adding,
            _ => false,
        }
    }
    fn part_handle_hit(&self, nx: f32, ny: f32) -> bool {
        self.rect().is_some_and(|r| {
            (nx - r.2 as f32).abs() * self.picture.width as f32 / 10000.0 <= self.part_handle_half.0
                && (ny - r.3 as f32).abs() * self.picture.height as f32 / 10000.0
                    <= self.part_handle_half.1
        })
    }
    pub fn set_preset(&mut self, p: Profile) {
        self.preset = Some(p);
    }
    fn current(&self) -> Option<&Profile> {
        self.profile.as_ref().or(self.preset.as_ref())
    }
    fn rect(&self) -> Option<Rect> {
        let p = self.current()?;
        if self.parts {
            return self.part.and_then(|i| {
                p.parts
                    .get(&i)
                    .copied()
                    .or_else(|| self.reading_rects.get(i).copied())
            });
        }
        if self.selected < 4 {
            Some(p.bands[self.selected])
        } else {
            p.exclusions.get(self.selected - 4).copied()
        }
    }
    fn update_rect(&mut self, r: Rect) {
        if self.rect() == Some(r) {
            return;
        }
        if self.profile.is_none() {
            self.profile = self.preset.clone();
        }
        if let Some(p) = &mut self.profile {
            if self.parts {
                if let Some(i) = self.part {
                    p.parts.insert(i, r);
                }
            } else if self.selected < 4 {
                p.bands[self.selected] = r;
            } else if let Some(old) = p.exclusions.get_mut(self.selected - 4) {
                *old = r;
            }
            self.dirty = true;
        }
    }
    pub fn input(&mut self, mouse: Option<(f32, f32)>, down: bool) -> bool {
        if down && !self.was_down {
            self.action_before = Some(self.profile.clone());
        }
        let handled = self.input_inner(mouse, down);
        if !down {
            self.finish_action();
        }
        handled
    }
    fn finish_action(&mut self) {
        if let Some(before) = self.action_before.take()
            && before != self.profile
        {
            // Bounded session history; one entry per completed mouse action.
            if self.undo_history.len() == 100 {
                self.undo_history.remove(0);
            }
            self.undo_history.push(before);
        }
    }
    pub fn undo(&mut self) {
        if !self.open {
            return;
        }
        self.finish_action();
        if let Some(before) = self.undo_history.pop() {
            self.profile = before;
            self.dirty = true;
            self.drag = None;
            self.adding = false;
            self.part = None;
            self.parts = false;
            self.selected = 0;
        }
    }
    fn input_inner(&mut self, mouse: Option<(f32, f32)>, down: bool) -> bool {
        let fresh = down && !self.was_down;
        self.was_down = down;
        if !down {
            self.drag = None;
        }
        let Some((x, y)) = mouse else {
            return self.open;
        };
        if fresh && (35.0..=300.0).contains(&x) && (362.0..=390.0).contains(&y) {
            self.open = !self.open;
            self.drag = None;
            return true;
        }
        if !self.open {
            return false;
        }
        if self.current().is_none() {
            return true;
        }
        if fresh && (35.0..=768.0).contains(&x) && (60.0..=80.0).contains(&y) {
            let button = ((x - 35.0) / 92.0) as usize;
            match button {
                0..=3 => {
                    self.parts = false;
                    self.part = None;
                    self.selected = button;
                    self.adding = false;
                }
                4 => {
                    self.parts = !self.parts;
                    self.part = None;
                    self.adding = false;
                    self.drag = None;
                }
                5 => {
                    self.parts = false;
                    self.part = None;
                    self.adding = true;
                }
                6 if self.parts && self.part.is_some() => {
                    if let Some(p) = &mut self.profile {
                        p.parts.remove(&self.part.unwrap());
                        self.dirty = true;
                    }
                    self.part = None;
                }
                6 if !self.parts && self.selected >= 4 => {
                    if self.profile.is_none() {
                        self.profile = self.preset.clone();
                    }
                    if let Some(p) = &mut self.profile {
                        p.exclusions.remove(self.selected - 4);
                        self.dirty = true;
                    }
                    self.selected = 0;
                    self.adding = false;
                }
                7 => {
                    self.parts = false;
                    self.part = None;
                    // Save an explicit clean layout. None would reactivate
                    // the built-in WoW masks and bottom inset on every reset.
                    self.profile = self.preset.clone().map(|mut p| {
                        p.exclusions.clear();
                        p.parts.clear();
                        let depth = p.bands[0].3 - p.bands[0].1;
                        p.bands[2] = Rect(0, 10000 - depth, 10000, 10000);
                        p
                    });
                    self.dirty = true;
                    self.selected = 0;
                    self.adding = false;
                }
                _ => (),
            }
            return true;
        }
        let pic = self.picture;
        let inside = x >= pic.x as f32
            && x <= (pic.x + pic.width) as f32
            && y >= pic.y as f32
            && y <= (pic.y + pic.height) as f32;
        // Hit-test the drawn handles/borders before clipping to the picture.
        // Clamping here made edge handles unclickable in the surrounding margin.
        let nx = (x - pic.x as f32) / pic.width as f32 * 10000.0;
        let ny = (y - pic.y as f32) / pic.height as f32 * 10000.0;
        let selected_handle = if self.parts {
            self.part_handle_hit(nx, ny)
        } else {
            self.rect().is_some_and(|r| {
                (nx - r.2 as f32).abs() * pic.width as f32 / 10000.0 <= 12.0
                    && (ny - r.3 as f32).abs() * pic.height as f32 / 10000.0 <= 12.0
            })
        };
        let border = |r| near_border(r, nx, ny, pic);
        let outside_hit = !self.adding
            && (selected_handle
                || (!self.parts
                    && (self.rect().is_some_and(border)
                        || self
                            .current()
                            .unwrap()
                            .exclusions
                            .iter()
                            .copied()
                            .any(border))));
        if fresh && (inside || outside_hit) {
            if self.parts {
                let handle = selected_handle;
                if !handle {
                    self.part = self.readings.iter().rposition(|r| {
                        x >= r.x as f32
                            && x < (r.x + r.width) as f32
                            && y >= r.y as f32
                            && y < (r.y + r.height) as f32
                    });
                }
                if let Some(r) = self.rect() {
                    // First click selects/moves a part; only an already-selected
                    // handle resizes, so narrow neighbouring boxes stay selectable.
                    self.drag = Some(if handle {
                        Drag::Resize(r)
                    } else {
                        Drag::Move(r, nx, ny)
                    });
                }
            } else if self.adding {
                if self.current().unwrap().exclusions.len() < 32 {
                    self.drag = Some(Drag::New(nx, ny));
                }
            } else {
                // Prefer the selected band's handle even when an exclusion
                // covers it. Select other exclusions by their red borders,
                // leaving band interiors draggable through overlapping UI.
                let handle = self.rect().is_some_and(|r| {
                    (nx - r.2 as f32).abs() * pic.width as f32 / 10000.0 <= 12.0
                        && (ny - r.3 as f32).abs() * pic.height as f32 / 10000.0 <= 12.0
                });
                if !handle
                    && let Some(i) = self
                        .current()
                        .unwrap()
                        .exclusions
                        .iter()
                        .rposition(|r| border(*r))
                {
                    self.selected = i + 4;
                }
                if let Some(r) = self.rect() {
                    let near = (nx - r.2 as f32).abs() * pic.width as f32 / 10000.0 <= 12.0
                        && (ny - r.3 as f32).abs() * pic.height as f32 / 10000.0 <= 12.0;
                    if near {
                        self.drag = Some(Drag::Resize(r));
                    } else if border(r)
                        || (nx >= r.0 as f32
                            && nx <= r.2 as f32
                            && ny >= r.1 as f32
                            && ny <= r.3 as f32)
                    {
                        self.drag = Some(Drag::Move(r, nx, ny));
                    }
                }
            }
        }
        if down && let Some(drag) = self.drag {
            let r = match drag {
                Drag::Move(r, sx, sy) => {
                    let dx = (nx - sx).round() as isize;
                    let dy = (ny - sy).round() as isize;
                    let l = (r.0 as isize + dx).clamp(0, (10000 - (r.2 - r.0)) as isize) as usize;
                    let t = (r.1 as isize + dy).clamp(0, (10000 - (r.3 - r.1)) as isize) as usize;
                    Rect(l, t, l + r.2 - r.0, t + r.3 - r.1)
                }
                Drag::Resize(r) => Rect(
                    r.0,
                    r.1,
                    (nx.clamp(0.0, 10000.0) as usize).max(r.0 + 1),
                    (ny.clamp(0.0, 10000.0) as usize).max(r.1 + 1),
                ),
                Drag::New(sx, sy) => {
                    let nx = nx.clamp(0.0, 10000.0);
                    let ny = ny.clamp(0.0, 10000.0);
                    if (nx - sx).abs() < 50.0 || (ny - sy).abs() < 50.0 {
                        return true;
                    }
                    let r = Rect(
                        sx.min(nx) as usize,
                        sy.min(ny) as usize,
                        sx.max(nx) as usize,
                        sy.max(ny) as usize,
                    );
                    if self.profile.is_none() {
                        self.profile = self.preset.clone();
                    }
                    let p = self.profile.as_mut().unwrap();
                    p.exclusions.push(r);
                    self.dirty = true;
                    self.selected = p.exclusions.len() + 3;
                    self.adding = false;
                    self.drag = Some(Drag::Draw(sx, sy));
                    r
                }
                Drag::Draw(sx, sy) => {
                    let nx = nx.clamp(0.0, 10000.0);
                    let ny = ny.clamp(0.0, 10000.0);
                    Rect(
                        (sx.min(nx) as usize).min(9999),
                        (sy.min(ny) as usize).min(9999),
                        (sx.max(nx) as usize)
                            .max(sx.min(nx) as usize + 1)
                            .min(10000),
                        (sy.max(ny) as usize)
                            .max(sy.min(ny) as usize + 1)
                            .min(10000),
                    )
                }
            };
            self.update_rect(r);
        }
        true
    }
    pub fn released(&self) -> bool {
        !self.was_down
    }
    pub fn capture(
        &mut self,
        frame: &Frame,
        bounds: Region,
        readings: &[Region],
        blocked: &[bool],
    ) {
        if !self.open {
            return;
        }
        if Instant::now() < self.next_picture {
            return;
        }
        self.next_picture = Instant::now() + Duration::from_millis(50);
        let scale = (740.0 / bounds.width as f64).min(230.0 / bounds.height as f64);
        let w = (bounds.width as f64 * scale).round().max(1.0) as usize;
        let h = (bounds.height as f64 * scale).round().max(1.0) as usize;
        self.picture = Region {
            x: 30 + (740 - w) / 2,
            y: 108 + (230 - h) / 2,
            width: w,
            height: h,
        };
        let view = crate::output::content_rect(self.view_size.0, self.view_size.1);
        let nw = (w * view.width / 800).max(1);
        let nh = (h * view.height / 400).max(1);
        self.native_size = (nw, nh);
        self.native_picture.resize(nw * nh, 0);
        for y in 0..nh {
            for x in 0..nw {
                let i = (bounds.y + y * bounds.height / nh) * frame.pitch
                    + (bounds.x + x * bounds.width / nw) * 4;
                self.native_picture[y * nw + x] = (u32::from(frame.data[i + 2]) << 16)
                    | (u32::from(frame.data[i + 1]) << 8)
                    | u32::from(frame.data[i]);
            }
        }
        self.thumbnail.fill(0x151515);
        for y in 0..h {
            for x in 0..w {
                let i = (bounds.y + y * bounds.height / h) * frame.pitch
                    + (bounds.x + x * bounds.width / w) * 4;
                self.thumbnail[(self.picture.y + y) * 800 + self.picture.x + x] =
                    (u32::from(frame.data[i + 2]) << 16)
                        | (u32::from(frame.data[i + 1]) << 8)
                        | u32::from(frame.data[i]);
            }
        }
        self.readings = readings
            .iter()
            .map(|r| Region {
                x: self.picture.x + (r.x - bounds.x) * w / bounds.width,
                y: self.picture.y + (r.y - bounds.y) * h / bounds.height,
                width: (r.width * w / bounds.width).max(1),
                height: (r.height * h / bounds.height).max(1),
            })
            .collect();
        self.reading_rects = readings
            .iter()
            .map(|r| {
                let x = (r.x - bounds.x) * 10000 / bounds.width;
                let y = (r.y - bounds.y) * 10000 / bounds.height;
                Rect(
                    x,
                    y,
                    ((r.x - bounds.x + r.width) * 10000 / bounds.width)
                        .max(x + 1)
                        .min(10000),
                    ((r.y - bounds.y + r.height) * 10000 / bounds.height)
                        .max(y + 1)
                        .min(10000),
                )
            })
            .collect();
        self.blocked = blocked.to_vec();
    }
    /// Replace only the picture with full viewport detail, then draw geometry
    /// over it. UI coordinates remain stable and the colour frame stays intact.
    #[cfg(test)]
    pub fn draw_native(&self, buffer: &mut [u32], width: usize, height: usize) {
        let view = crate::output::content_rect(width, height);
        let map = |r: Region| Region {
            x: view.x + r.x * view.width / 800,
            y: view.y + r.y * view.height / 400,
            width: (r.width * view.width / 800).max(1),
            height: (r.height * view.height / 400).max(1),
        };
        let picture = map(self.picture);
        let (nw, nh) = self.native_size;
        if nw == 0 || nh == 0 {
            return;
        }
        if (nw, nh) == (picture.width, picture.height) {
            for y in 0..nh {
                buffer
                    [(picture.y + y) * width + picture.x..(picture.y + y) * width + picture.x + nw]
                    .copy_from_slice(&self.native_picture[y * nw..(y + 1) * nw]);
            }
        } else {
            for y in 0..picture.height {
                for x in 0..picture.width {
                    buffer[(picture.y + y) * width + picture.x + x] = self.native_picture
                        [(y * nh / picture.height) * nw + x * nw / picture.width];
                }
            }
        }
        for (i, r) in self.reading_rects.iter().enumerate() {
            let q = r.pixels(picture.width, picture.height);
            outline_native(
                buffer,
                width,
                height,
                Region {
                    x: q.x + picture.x,
                    y: q.y + picture.y,
                    ..q
                },
                if self.blocked.get(i) == Some(&true) {
                    0xff4444
                } else {
                    0xffffff
                },
            );
        }
        if let Some(p) = self.current() {
            for (i, r) in p.bands.iter().chain(&p.exclusions).enumerate() {
                let q = r.pixels(picture.width, picture.height);
                let q = Region {
                    x: q.x + picture.x,
                    y: q.y + picture.y,
                    ..q
                };
                let colour = if i < 4 { COLOURS[i] } else { 0xff5555 };
                outline_native(buffer, width, height, q, colour);
                if !self.parts && i == self.selected {
                    native_handle(buffer, width, height, q, colour, view.width * 8 / 800);
                }
            }
        }
        if self.parts
            && let Some(r) = self.rect()
        {
            let q = r.pixels(picture.width, picture.height);
            let q = Region {
                x: q.x + picture.x,
                y: q.y + picture.y,
                ..q
            };
            outline_native(buffer, width, height, q, 0xffff44);
            native_handle(buffer, width, height, q, 0xffff44, view.width * 8 / 800);
        }
    }
    #[cfg(test)]
    pub fn draw(&self, buffer: &mut [u32]) {
        if self.open {
            for y in 30..360 {
                buffer[y * 800 + 30..y * 800 + 770]
                    .copy_from_slice(&self.thumbnail[y * 800 + 30..y * 800 + 770]);
            }
            text(buffer, 35, 33, "EDIT SAMPLING", 0xe4eaf2);
            for (i, label) in [
                "TOP", "RIGHT", "BOTTOM", "LEFT", "PARTS", "EXCLUDE", "DELETE", "RESET",
            ]
            .iter()
            .enumerate()
            {
                text(
                    buffer,
                    35 + i * 92,
                    63,
                    label,
                    if (!self.parts && i < 4 && i == self.selected)
                        || (i == 4 && self.parts)
                        || (i == 5 && self.adding)
                    {
                        0x44cfff
                    } else {
                        0xc7d2e2
                    },
                );
            }
            text(
                buffer,
                35,
                87,
                if self.adding {
                    "DRAW A BOX OVER THE UI TO IGNORE"
                } else if self.parts {
                    "CLICK A WHITE BOX - DRAG TO MOVE OR RESIZE"
                } else {
                    ""
                },
                0xc7d2e2,
            );
            for (i, r) in self.readings.iter().enumerate() {
                outline(
                    buffer,
                    *r,
                    if self.blocked.get(i) == Some(&true) {
                        0xff4444
                    } else {
                        0xffffff
                    },
                );
            }
            if let Some(p) = self.current() {
                for (i, r) in p.bands.iter().chain(&p.exclusions).enumerate() {
                    let q = r.pixels(self.picture.width, self.picture.height);
                    let q = Region {
                        x: q.x + self.picture.x,
                        y: q.y + self.picture.y,
                        ..q
                    };
                    let colour = if i < 4 { COLOURS[i] } else { 0xff5555 };
                    outline(buffer, q, colour);
                    if !self.parts && i == self.selected {
                        for y in (q.y + q.height).saturating_sub(8)..(q.y + q.height + 8).min(400) {
                            for x in (q.x + q.width).saturating_sub(8)..(q.x + q.width + 8).min(800)
                            {
                                buffer[y * 800 + x] = colour;
                            }
                        }
                    }
                }
            }
            if self.parts
                && let Some(r) = self.rect()
            {
                let q = r.pixels(self.picture.width, self.picture.height);
                let q = Region {
                    x: q.x + self.picture.x,
                    y: q.y + self.picture.y,
                    ..q
                };
                outline(buffer, q, 0xffff44);
                for y in (q.y + q.height).saturating_sub(8)..(q.y + q.height + 8).min(400) {
                    for x in (q.x + q.width).saturating_sub(8)..(q.x + q.width + 8).min(800) {
                        buffer[y * 800 + x] = 0xffff44;
                    }
                }
            }
            let part_label = self.part.map(|i| {
                let mut offset = 0;
                for (edge, count) in self.counts.iter().enumerate() {
                    if i < offset + count {
                        return format!(
                            "{} PART {} - DELETE RESETS IT",
                            ["TOP", "RIGHT", "BOTTOM", "LEFT"][edge],
                            i - offset + 1
                        );
                    }
                    offset += count;
                }
                "CLICK A WHITE READING BOX".into()
            });
            text(
                buffer,
                325,
                370,
                if self.parts {
                    part_label.as_deref().unwrap_or("CLICK A WHITE READING BOX")
                } else if self.blocked.contains(&true) {
                    "RED READINGS ARE FULLY BLOCKED"
                } else {
                    ""
                },
                0xc7d2e2,
            );
        }
        text(
            buffer,
            35,
            370,
            if self.open {
                "DONE - LIGHTING"
            } else {
                "EDIT SAMPLING"
            },
            0x44cfff,
        );
    }
}
fn near_border(r: Rect, nx: f32, ny: f32, pic: Region) -> bool {
    let dx = 5.0 * 10000.0 / pic.width as f32;
    let dy = 5.0 * 10000.0 / pic.height as f32;
    nx >= r.0 as f32 - dx
        && nx <= r.2 as f32 + dx
        && ny >= r.1 as f32 - dy
        && ny <= r.3 as f32 + dy
        && ((nx - r.0 as f32).abs() <= dx
            || (nx - r.2 as f32).abs() <= dx
            || (ny - r.1 as f32).abs() <= dy
            || (ny - r.3 as f32).abs() <= dy)
}

#[cfg(test)]
fn outline_native(buffer: &mut [u32], width: usize, height: usize, r: Region, colour: u32) {
    let right = (r.x + r.width).min(width);
    let bottom = (r.y + r.height).min(height);
    if r.x >= right || r.y >= bottom {
        return;
    }
    buffer[r.y * width + r.x..r.y * width + right].fill(colour);
    buffer[(bottom - 1) * width + r.x..(bottom - 1) * width + right].fill(colour);
    for y in r.y..bottom {
        buffer[y * width + r.x] = colour;
        buffer[y * width + right - 1] = colour;
    }
}
#[cfg(test)]
fn native_handle(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    r: Region,
    colour: u32,
    size: usize,
) {
    let size = size.max(3);
    for y in (r.y + r.height).saturating_sub(size)..(r.y + r.height + size).min(height) {
        buffer[y * width + (r.x + r.width).saturating_sub(size)
            ..y * width + (r.x + r.width + size).min(width)]
            .fill(colour);
    }
}
#[cfg(test)]
fn outline(buffer: &mut [u32], r: Region, colour: u32) {
    for y in r.y..(r.y + r.height).min(400) {
        for x in r.x..(r.x + r.width).min(800) {
            if y == r.y || y + 1 == r.y + r.height || x == r.x || x + 1 == r.x + r.width {
                buffer[y * 800 + x] = colour;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn delete_requires_a_selected_exclusion_or_adjusted_reading() {
        let mut e = Editor::default();
        let mut profile = Profile::preset(3440, 1440, 120, 0, false);
        profile.exclusions.push(Rect(100, 100, 200, 200));
        profile.parts.insert(0, Rect(100, 100, 200, 200));
        e.profile = Some(profile);
        assert!(!e.can_delete());
        e.selected = 4;
        assert!(e.can_delete());
        e.adding = true;
        assert!(!e.can_delete());
        e.adding = false;
        e.selected = 5;
        assert!(!e.can_delete());
        e.parts = true;
        assert!(!e.can_delete());
        e.part = Some(0);
        assert!(e.can_delete());
        e.part = Some(1);
        assert!(!e.can_delete());
    }
    #[test]
    fn hints_do_not_change_preview_size() {
        let ctx = eframe::egui::Context::default();
        crate::gui::theme(&ctx);
        let mut e = Editor::default();
        let mut canvas = crate::gui::Canvas::default();
        let mut sizes = Vec::new();
        for mode in 0..4 {
            e.parts = mode == 1;
            e.adding = mode == 2;
            e.blocked = vec![mode == 3];
            let _ = ctx.run(
                eframe::egui::RawInput {
                    screen_rect: Some(eframe::egui::Rect::from_min_size(
                        eframe::egui::Pos2::ZERO,
                        eframe::egui::vec2(1200.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    eframe::egui::CentralPanel::default()
                        .show(ctx, |ui| e.modern_ui(ui, &mut canvas, &[], false));
                },
            );
            sizes.push(e.view_size);
        }
        assert!(sizes.iter().all(|size| *size == sizes[0]), "{sizes:?}");
    }
    #[test]
    fn reset_confirmation_cancel_preserves_layout_and_confirm_is_undoable() {
        let mut e = selected_part_fixture();
        e.set_preset(Profile::preset(100, 100, 10, 20, true));
        e.profile
            .as_mut()
            .unwrap()
            .exclusions
            .push(Rect(1000, 1000, 2000, 2000));
        let original = e.profile.clone();
        e.reset_pending = true;
        e.resolve_reset(false);
        assert_eq!(e.profile, original);
        assert!(!e.dirty && e.undo_history.is_empty());
        e.reset_pending = true;
        e.resolve_reset(true);
        assert!(!e.reset_pending);
        assert!(e.profile.as_ref().unwrap().exclusions.is_empty());
        assert!(e.profile.as_ref().unwrap().parts.is_empty());
        assert_eq!(e.profile.as_ref().unwrap().bands[2].3, 10000);
        assert!(e.dirty);
        e.undo();
        assert_eq!(e.profile, original);
    }
    fn selected_part_fixture() -> Editor {
        let mut profile = Profile::preset(100, 100, 10, 0, false);
        profile.parts.insert(0, Rect(2000, 2000, 4000, 4000));
        Editor {
            open: true,
            parts: true,
            part: Some(0),
            profile: Some(profile),
            picture: Region {
                x: 100,
                y: 100,
                width: 100,
                height: 100,
            },
            readings: vec![Region {
                x: 120,
                y: 120,
                width: 20,
                height: 20,
            }],
            part_handle_half: (4.0, 4.0),
            ..Default::default()
        }
    }
    #[test]
    fn dragging_selected_cell_near_corner_moves_without_resizing() {
        let mut e = selected_part_fixture();
        e.input(Some((134.0, 134.0)), true); // Within the old resize hotspot, outside the yellow square.
        assert!(matches!(e.drag, Some(Drag::Move(..))));
        e.input(Some((137.0, 136.0)), true);
        e.input(None, false);
        assert_eq!(
            e.profile.as_ref().unwrap().parts[&0],
            Rect(2300, 2200, 4300, 4200)
        );
        e.undo();
        assert_eq!(
            e.profile.as_ref().unwrap().parts[&0],
            Rect(2000, 2000, 4000, 4000)
        );
    }
    #[test]
    fn selected_cell_resizes_only_from_visible_yellow_square() {
        let mut e = selected_part_fixture();
        e.input(Some((142.0, 142.0)), true); // Visible square extends beyond the cell itself.
        assert!(matches!(e.drag, Some(Drag::Resize(..))));
        e.input(Some((145.0, 146.0)), true);
        e.input(None, false);
        assert_eq!(
            e.profile.as_ref().unwrap().parts[&0],
            Rect(2000, 2000, 4500, 4600)
        );
    }
    #[test]
    fn exclusion_mode_highlights_only_exclude_area() {
        for edge in [0, 3] {
            let mut e = Editor {
                open: true,
                selected: edge,
                ..Default::default()
            };
            e.set_preset(Profile::preset(1000, 500, 50, 0, false));
            e.input(Some((505.0, 70.0)), true);
            e.input(None, false);
            assert!(e.toolbar_selected(5));
            for index in 0..5 {
                assert!(!e.toolbar_selected(index));
            }
            e.input(Some((45.0 + edge as f32 * 92.0, 70.0)), true);
            e.input(None, false);
            assert!(e.toolbar_selected(edge));
            assert!(!e.toolbar_selected(5));
        }
    }
    #[test]
    fn undo_groups_drag_updates_and_restores_resize_then_move() {
        let mut e = Editor {
            open: true,
            selected: 2,
            ..Default::default()
        };
        let p = Profile::preset(1000, 500, 50, 0, false);
        e.profile = Some(p.clone());
        e.input(Some((400.0, 330.0)), true);
        e.input(Some((400.0, 325.0)), true);
        e.input(Some((400.0, 310.0)), true);
        e.input(None, false);
        let moved = e.profile.clone();
        assert_ne!(moved, Some(p.clone()));
        assert_eq!(e.undo_history.len(), 1);
        let r = e.rect().unwrap();
        let hx = e.picture.x as f32 + r.2 as f32 * e.picture.width as f32 / 10000.0;
        let hy = e.picture.y as f32 + r.3 as f32 * e.picture.height as f32 / 10000.0;
        e.input(Some((hx, hy)), true);
        e.input(Some((hx - 30.0, hy - 5.0)), true);
        e.input(None, false);
        assert_eq!(e.undo_history.len(), 2);
        e.undo();
        assert_eq!(e.profile, moved);
        e.undo();
        assert_eq!(e.profile, Some(p));
        assert!(e.dirty);
        let restored = e.profile.clone();
        e.undo();
        assert_eq!(e.profile, restored);
    }
    #[test]
    fn undo_restores_deleted_exclusion_and_reset_without_recording_selection() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        let p = Profile::preset(1000, 500, 50, 80, true);
        e.profile = Some(p.clone());
        e.set_preset(p.clone());
        e.input(Some((150.0, 70.0)), true); // Select right; not a change.
        e.input(None, false);
        assert!(e.undo_history.is_empty());
        e.selected = 4;
        e.input(Some((620.0, 70.0)), true);
        e.input(None, false);
        assert_eq!(
            e.profile.as_ref().unwrap().exclusions.len(),
            p.exclusions.len() - 1
        );
        e.undo();
        assert_eq!(e.profile, Some(p.clone()));
        e.input(Some((710.0, 70.0)), true);
        e.input(None, false);
        assert!(e.profile.as_ref().unwrap().exclusions.is_empty());
        e.undo();
        assert_eq!(e.profile, Some(p));
    }
    #[test]
    fn undo_during_drag_stops_it_and_preserves_default_profile() {
        let mut e = Editor {
            open: true,
            selected: 2,
            ..Default::default()
        };
        e.set_preset(Profile::preset(1000, 500, 50, 0, false));
        e.input(Some((400.0, 330.0)), true);
        assert!(e.profile.is_none()); // Click without moving is not an edit.
        e.input(Some((400.0, 300.0)), true);
        e.undo();
        assert!(e.profile.is_none());
        e.input(Some((400.0, 280.0)), true);
        e.input(None, false);
        assert!(e.profile.is_none());
        assert!(e.undo_history.is_empty());
    }

    #[test]
    fn margin_handle_and_border_are_draggable_without_offscreen_geometry() {
        let mut e = Editor {
            open: true,
            selected: 2,
            ..Default::default()
        };
        e.set_preset(Profile::preset(1000, 500, 50, 0, false));
        // Bottom-right handle extends beyond the picture (right770, bottom338).
        e.input(Some((777.0, 345.0)), true);
        assert!(matches!(e.drag, Some(Drag::Resize(_))));
        e.input(Some((700.0, 315.0)), true);
        let resized = e.profile.as_ref().unwrap().bands[2];
        assert!(resized.2 < 10000 && resized.3 < 10000);
        e.input(None, false);
        e.profile = None;
        // Grab the selected bottom border just below the picture and move up.
        e.input(Some((400.0, 341.0)), true);
        assert!(matches!(e.drag, Some(Drag::Move(_, _, _))));
        e.input(Some((400.0, 320.0)), true);
        assert!(e.profile.as_ref().unwrap().bands[2].3 < 10000);
        e.input(None, false);
        e.profile = None;
        e.input(Some((790.0, 350.0)), true);
        assert!(e.drag.is_none()); // Distant margin is not a hit.
    }

    use super::*;
    #[test]
    fn edit_band_add_delete_exclusion_and_reset() {
        let mut e = Editor::default();
        e.set_preset(Profile::preset(1000, 500, 50, 80, false));
        e.input(Some((50.0, 375.0)), true);
        e.input(None, false);
        assert!(e.open);
        e.input(Some((100.0, 110.0)), true);
        e.input(Some((100.0, 140.0)), true);
        e.input(None, false);
        assert!(e.profile.as_ref().unwrap().bands[0].1 > 0);
        e.input(Some((520.0, 70.0)), true);
        e.input(None, false);
        e.input(Some((600.0, 150.0)), true);
        e.input(Some((740.0, 240.0)), true);
        e.input(None, false);
        assert_eq!(e.profile.as_ref().unwrap().exclusions.len(), 1);
        e.input(Some((620.0, 70.0)), true);
        e.input(None, false);
        assert!(e.profile.as_ref().unwrap().exclusions.is_empty());
        e.input(Some((710.0, 70.0)), true);
        e.input(None, false);
        assert_eq!(e.profile, Some(Profile::preset(1000, 500, 50, 0, false)));
        assert!(e.dirty);
    }
    #[test]
    fn individual_part_moves_resizes_and_resets_without_moving_its_band() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        let preset = Profile::preset(1000, 500, 50, 80, false);
        let (readings, blocked) = preset.regions(1000, 500, crate::strip::COUNTS);
        e.set_preset(preset.clone());
        let frame = Frame {
            data: vec![0; 1000 * 500 * 4],
            width: 1000,
            height: 500,
            pitch: 4000,
        };
        e.capture(
            &frame,
            Region {
                x: 0,
                y: 0,
                width: 1000,
                height: 500,
            },
            &readings,
            &blocked,
        );
        e.input(Some((440.0, 70.0)), true);
        e.input(None, false);
        assert!(e.parts);
        let q = e.readings[10];
        let point = ((q.x + q.width / 2) as f32, (q.y + q.height / 2) as f32);
        e.input(Some(point), true);
        e.input(Some((point.0 + 30.0, point.1 + 50.0)), true);
        e.input(None, false);
        assert_eq!(e.part, Some(10));
        let p = e.profile.as_ref().unwrap();
        assert_eq!(p.bands, preset.bands);
        assert_eq!(p.parts.len(), 1);
        let moved = p.parts[&10];
        assert!(moved.1 > 1000);
        let q = moved.pixels(e.picture.width, e.picture.height);
        let handle = (
            (e.picture.x + q.x + q.width) as f32,
            (e.picture.y + q.y + q.height) as f32,
        );
        e.input(Some(handle), true);
        e.input(Some((handle.0 + 20.0, handle.1 + 20.0)), true);
        e.input(None, false);
        let resized = e.profile.as_ref().unwrap().parts[&10];
        assert!(resized.2 > moved.2 && resized.3 > moved.3);
        assert_eq!(
            crate::sampling::decode(&crate::sampling::encode(e.profile.as_ref().unwrap())).unwrap(),
            *e.profile.as_ref().unwrap()
        );
        e.input(Some((620.0, 70.0)), true);
        e.input(None, false);
        assert!(e.profile.as_ref().unwrap().parts.is_empty());
        assert!(e.parts && e.part.is_none());
        assert_eq!(e.profile.as_ref().unwrap().bands, preset.bands);
    }
    #[test]
    fn reset_clears_wow_masks_parts_and_bottom_inset_and_survives_reload() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        let preset = Profile::preset(3440, 1440, 120, 240, true);
        assert!(!preset.exclusions.is_empty());
        e.set_preset(preset.clone());
        let mut edited = preset.clone();
        edited.bands[0] = Rect(1000, 2000, 9000, 3000);
        edited.parts.insert(10, Rect(2000, 2000, 2200, 2400));
        edited.exclusions.push(Rect(5000, 4000, 8000, 9000));
        e.profile = Some(edited);
        e.input(Some((710.0, 70.0)), true);
        e.input(None, false);
        let clean = e.profile.clone().unwrap();
        assert_eq!(clean, Profile::preset(3440, 1440, 120, 0, false));
        assert!(e.dirty && clean.exclusions.is_empty() && clean.parts.is_empty());
        let loaded = crate::sampling::decode(&crate::sampling::encode(&clean)).unwrap();
        e.profile = Some(loaded);
        // Reapplying the launcher's WoW preset must not restore its exclusions.
        e.set_preset(preset);
        assert_eq!(e.current(), Some(&clean));
        assert!(
            e.current()
                .unwrap()
                .regions(3440, 1440, crate::strip::COUNTS)
                .1
                .iter()
                .all(|b| !b)
        );
    }
    #[test]
    fn padded_capture_renders_active_picture_without_chrome() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        let f = Frame {
            data: vec![0; 32 * 4],
            width: 6,
            height: 4,
            pitch: 32,
        };
        let mut f = f;
        for y in 1..3 {
            for x in 1..5 {
                f.data[y * 32 + x * 4..y * 32 + x * 4 + 4].copy_from_slice(&[3, 2, 1, 255]);
            }
        }
        e.capture(
            &f,
            Region {
                x: 1,
                y: 1,
                width: 4,
                height: 2,
            },
            &[],
            &[],
        );
        let q = e.picture;
        assert_eq!(
            e.thumbnail[(q.y + q.height / 2) * 800 + q.x + q.width / 2],
            0x010203
        );
        let mut b = vec![0; 800 * 400];
        e.draw(&mut b);
    }
    #[test]
    fn native_picture_retains_detail_lost_by_small_preview() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        let mut data = vec![0; 920 * 460 * 4];
        for (i, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let v = if i % 920 % 2 == 0 { 0 } else { 255 };
            pixel.copy_from_slice(&[v, v, v, 255]);
        }
        let frame = Frame {
            data,
            width: 920,
            height: 460,
            pitch: 920 * 4,
        };
        e.capture(
            &frame,
            Region {
                x: 0,
                y: 0,
                width: 920,
                height: 460,
            },
            &[],
            &[],
        );
        assert_eq!(e.native_size, (920, 460));
        let logical = e.picture;
        assert_eq!(e.thumbnail[(logical.y + 100) * 800 + logical.x + 150], 0);
        assert_eq!(e.thumbnail[(logical.y + 100) * 800 + logical.x + 151], 0);
        let mut native = vec![0x224466; 1600 * 800];
        e.draw_native(&mut native, 1600, 800);
        let i = (logical.y * 2 + 200) * 1600 + logical.x * 2 + 300;
        assert_eq!(native[i], 0);
        assert_eq!(native[i + 1], 0xffffff);
        assert_eq!(native[0], 0x224466);
    }
    #[test]
    fn drawing_reverse_resize_and_drag_clamp_keep_valid_saved_geometry() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        e.set_preset(Profile::preset(1000, 500, 50, 80, false));
        e.input(Some((520.0, 70.0)), true);
        e.input(None, false);
        e.input(Some((740.0, 300.0)), true);
        e.input(Some((700.0, 270.0)), true);
        e.input(Some((600.0, 200.0)), true);
        e.input(None, false);
        let r = e.profile.as_ref().unwrap().exclusions[0];
        assert!(r.2 - r.0 > 1800 && r.3 - r.1 > 4000);
        e.input(Some((740.0, 300.0)), true);
        e.input(Some((1000.0, 600.0)), true);
        e.input(None, false);
        assert_eq!(e.profile.as_ref().unwrap().exclusions[0].2, 10000);
        let text = crate::sampling::encode(e.profile.as_ref().unwrap());
        assert!(crate::sampling::decode(&text).is_ok());
        e.input(Some((650.0, 250.0)), true);
        e.input(Some((-500.0, -500.0)), true);
        e.input(None, false);
        let r = e.profile.as_ref().unwrap().exclusions[0];
        assert_eq!((r.0, r.1), (0, 0));
    }
    #[test]
    fn band_handle_is_editable_under_overlapping_minimap_exclusion() {
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        e.set_preset(Profile::preset(1000, 500, 50, 80, true));
        // Top-right handle is inside the minimap mask. It must resize Top.
        e.input(Some((770.0, 131.0)), true);
        e.input(Some((740.0, 150.0)), true);
        e.input(None, false);
        assert_eq!(e.selected, 0);
        assert!(e.profile.as_ref().unwrap().bands[0].3 > 1000);
        // Selecting Right lets its interior move even inside objectives masks.
        e.input(Some((150.0, 70.0)), true);
        e.input(None, false);
        e.input(Some((752.0, 210.0)), true);
        e.input(Some((700.0, 210.0)), true);
        e.input(None, false);
        assert_eq!(e.selected, 1);
        assert!(e.profile.as_ref().unwrap().bands[1].0 < 9500);
    }
    #[test]
    #[ignore = "requires supplied 3439x1439 WoW screenshot as BACKLIGHT_EDITOR_BGRA"]
    fn supplied_editor_screenshot_render() {
        let data = std::fs::read(std::env::var("BACKLIGHT_EDITOR_BGRA").unwrap()).unwrap();
        assert_eq!(data.len(), 3439 * 1439 * 4);
        let frame = Frame {
            data,
            width: 3439,
            height: 1439,
            pitch: 3439 * 4,
        };
        let bounds = Region {
            x: 0,
            y: 0,
            width: 3439,
            height: 1439,
        };
        let preset = Profile::preset(3439, 1439, 120, 239, true);
        let (readings, blocked) = preset.regions(3439, 1439, crate::strip::COUNTS);
        let mut e = Editor {
            open: true,
            ..Default::default()
        };
        e.set_preset(preset);
        e.capture(&frame, bounds, &readings, &blocked);
        let mut buffer = vec![0x224466; 800 * 400];
        e.draw(&mut buffer);
        let mut native = vec![0; 1600 * 800];
        crate::output::upscale_ui(&buffer, &mut native, 1600, 800);
        e.draw_native(&mut native, 1600, 800);
        // Offline UI render only: no desktop capture, window or DDP sender.
        let mut bmp = vec![0u8; 54];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&(54u32 + 1600 * 800 * 4).to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&1600i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-800i32).to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
        for pixel in native {
            bmp.extend_from_slice(&pixel.to_le_bytes());
        }
        std::fs::write("target/sampling-editor.bmp", bmp).unwrap();
    }
}
