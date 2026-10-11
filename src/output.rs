use crate::{
    colour::Rgb,
    controls::{Controls, Settings},
    layout::regions,
};
use anyhow::Result;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};

pub struct Preview {
    window: Window,
    buffer: Vec<u32>,
    native_buffer: Vec<u32>,
    counts: [usize; 4],
    raw: bool,
    controls: Controls,
    pub editor: crate::editor::Editor,
    pub setup: crate::setup::Wizard,
    pub profiles: crate::profiles::Picker,
}
impl Preview {
    pub fn new(counts: [usize; 4], raw: bool, setup: crate::setup::Wizard) -> Result<Self> {
        let mut window = Window::new(
            "Backlight — Esc to quit",
            1600,
            800,
            WindowOptions {
                resize: true,
                scale_mode: ScaleMode::AspectRatioStretch,
                ..WindowOptions::default()
            },
        )?;
        window.set_target_fps(0);
        Ok(Self {
            window,
            buffer: vec![0; 800 * 400],
            native_buffer: vec![],
            counts,
            raw,
            controls: Controls::default(),
            editor: crate::editor::Editor::new(counts),
            setup,
            profiles: crate::profiles::Picker::default(),
        })
    }
    pub fn open(&self) -> bool {
        self.window.is_open() && !(self.active() && self.window.is_key_down(Key::Escape))
    }
    fn active(&self) -> bool {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                == windows::Win32::Foundation::HWND(self.window.get_window_handle())
        }
    }
    fn minimized(&self) -> bool {
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::IsIconic(windows::Win32::Foundation::HWND(
                self.window.get_window_handle(),
            ))
            .as_bool()
        }
    }
    /// Exclude our visible rectangle from LED readings, not from screenshots.
    pub fn sampling_exclusion(
        &self,
        origin: (i32, i32),
        bounds: crate::layout::Region,
    ) -> Option<crate::layout::Region> {
        use windows::Win32::{Foundation::RECT, UI::WindowsAndMessaging::*};
        let hwnd = windows::Win32::Foundation::HWND(self.window.get_window_handle());
        let mut rect = RECT::default();
        unsafe {
            if self.minimized()
                || !IsWindowVisible(hwnd).as_bool()
                || GetWindowRect(hwnd, &mut rect).is_err()
            {
                return None;
            }
        }
        crate::taskbar::clipped(rect, origin, bounds, true)
    }
    pub fn interact(&mut self, settings: &mut Settings) {
        if self.minimized() {
            return;
        }
        let mouse = self
            .window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .and_then(|point| logical_mouse(point, self.window.get_size()));
        let down = self.window.get_mouse_down(MouseButton::Left);
        let keys = if self.active() {
            self.window.get_keys_pressed(KeyRepeat::Yes)
        } else {
            vec![]
        };
        if !self.setup.open && !self.editor.open && self.profiles.input(mouse, down, &keys) {
            return;
        }
        let was_setup = self.setup.open;
        self.setup.input(mouse, down, &keys);
        if was_setup || self.setup.open {
            return;
        }
        if self.active()
            && (self.window.is_key_down(Key::LeftCtrl) || self.window.is_key_down(Key::RightCtrl))
            && self.window.is_key_pressed(Key::Z, KeyRepeat::No)
        {
            self.editor.undo();
        }
        self.editor.view_size = self.window.get_size();
        let mouse = self
            .window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .and_then(|point| logical_mouse(point, self.window.get_size()));
        let down = self.window.get_mouse_down(MouseButton::Left);
        if self.editor.input(mouse, down) {
            return;
        }
        self.controls.input(mouse, down, settings);
    }
    pub fn reconfigure(&mut self, counts: [usize; 4], profile: Option<crate::sampling::Profile>) {
        self.counts = counts;
        self.editor = crate::editor::Editor::new(counts);
        self.editor.profile = profile;
    }
    pub fn draw(
        &mut self,
        colours: &[Rgb],
        status: &str,
        settings: Settings,
        effective_brightness: f64,
    ) -> Result<()> {
        // Show Desktop/minimize affects presentation only. Keep servicing events
        // so restoration works; the capture/DDP loop continues independently.
        if !can_render(self.minimized(), self.window.get_size()) {
            self.window.update();
            return Ok(());
        }
        self.window
            .set_title(&format!("Backlight — {status} — Esc to quit"));
        self.buffer.fill(0x151515);
        if self.raw {
            for (r, c) in regions(800, 400, self.counts, 30)?.iter().zip(colours) {
                let packed = (u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2);
                for y in r.y..r.y + r.height {
                    self.buffer[y * 800 + r.x..y * 800 + r.x + r.width].fill(packed);
                }
            }
        } else {
            render_glow(&mut self.buffer, colours, self.counts);
        }
        if !self.editor.open && !self.setup.open && !self.profiles.open {
            self.controls
                .draw(&mut self.buffer, settings, effective_brightness);
        }
        if !self.setup.open {
            self.editor.draw(&mut self.buffer);
        }
        self.setup.draw(&mut self.buffer);
        if !self.setup.open && !self.editor.open {
            self.profiles.draw(&mut self.buffer);
        }
        let (w, h) = self.window.get_size();
        if self.editor.open && !self.setup.open && w > 0 && h > 0 {
            self.native_buffer.resize(w * h, 0);
            upscale_ui(&self.buffer, &mut self.native_buffer, w, h);
            self.editor.draw_native(&mut self.native_buffer, w, h);
            self.window.update_with_buffer(&self.native_buffer, w, h)?;
        } else {
            self.window.update_with_buffer(&self.buffer, 800, 400)?;
        }
        Ok(())
    }
}

fn can_render(minimized: bool, (w, h): (usize, usize)) -> bool {
    !minimized && w > 0 && h > 0
}

/// Invert minifb's Windows aspect-ratio scaling, including integer letterbox
/// offsets. Its get_mouse_pos does not account for resized buffer dimensions.
fn logical_mouse((x, y): (f32, f32), (w, h): (usize, usize)) -> Option<(f32, f32)> {
    let r = content_rect(w, h);
    if r.width == 0 || r.height == 0 {
        return None;
    }
    let ox = r.x as f32;
    let oy = r.y as f32;
    if x < ox || y < oy || x >= ox + r.width as f32 || y >= oy + r.height as f32 {
        return None;
    }
    Some((
        (x - ox) * 800.0 / r.width as f32,
        (y - oy) * 400.0 / r.height as f32,
    ))
}
pub(crate) fn content_rect(w: usize, h: usize) -> crate::layout::Region {
    if w == 0 || h == 0 {
        return crate::layout::Region {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }
    let (draw_w, draw_h) = if 2.0 > w as f32 / h as f32 {
        (w, (w as f32 / 2.0) as usize)
    } else {
        ((h as f32 * 2.0) as usize, h)
    };
    crate::layout::Region {
        x: (w - draw_w) / 2,
        y: (h - draw_h) / 2,
        width: draw_w,
        height: draw_h,
    }
}
pub(crate) fn upscale_ui(source: &[u32], target: &mut [u32], w: usize, h: usize) {
    let r = content_rect(w, h);
    target.fill(0);
    if r.width == 0 || r.height == 0 {
        return;
    }
    let xs: Vec<_> = (0..r.width).map(|x| x * 800 / r.width).collect();
    for y in 0..r.height {
        let row = &source[y * 400 / r.height * 800..(y * 400 / r.height + 1) * 800];
        let out = &mut target[(r.y + y) * w + r.x..(r.y + y) * w + r.x + r.width];
        for (pixel, &x) in out.iter_mut().zip(&xs) {
            *pixel = row[x];
        }
    }
}

/// Stylised wall glow outside a dark monitor silhouette; output RGB is unchanged.
fn render_glow(buffer: &mut [u32], colours: &[Rgb], counts: [usize; 4]) {
    let sample = |edge: usize, t: f64| {
        let offset: usize = counts[..edge].iter().sum();
        let n = counts[edge];
        let position = (t.clamp(0.0, 1.0) * n as f64 - 0.5).clamp(0.0, (n - 1) as f64);
        let i = position.floor() as usize;
        let fraction = position - i as f64;
        let a = colours[offset + i];
        let b = colours[offset + (i + 1).min(n - 1)];
        [a.0, a.1, a.2]
            .map(f64::from)
            .into_iter()
            .zip([b.0, b.1, b.2])
            .map(|(a, b)| a + (f64::from(b) - a) * fraction)
            .collect::<Vec<_>>()
    };
    // Precompute edge gradients; the per-pixel pass only applies light falloff.
    let top: Vec<_> = (0..800)
        .map(|x| sample(0, (x as f64 - 100.0) / 600.0))
        .collect();
    let bottom: Vec<_> = (0..800)
        .map(|x| sample(2, 1.0 - (x as f64 - 100.0) / 600.0))
        .collect();
    let right: Vec<_> = (0..400)
        .map(|y| sample(1, (y as f64 - 70.0) / 260.0))
        .collect();
    let left: Vec<_> = (0..400)
        .map(|y| sample(3, 1.0 - (y as f64 - 70.0) / 260.0))
        .collect();
    for y in 0..400 {
        for x in 0..800 {
            if (100..700).contains(&x) && (70..330).contains(&y) {
                buffer[y * 800 + x] = 0x101014;
                continue;
            }
            let dx = if x < 100 {
                (100 - x) as f64
            } else if x >= 700 {
                (x - 699) as f64
            } else {
                0.0
            };
            let dy = if y < 70 {
                (70 - y) as f64
            } else if y >= 330 {
                (y - 329) as f64
            } else {
                0.0
            };
            let falloff = (-((dx * dx + dy * dy) / 5000.0)).exp();
            let mut rgb = [0u32; 3];
            for c in 0..3 {
                let vertical = if y < 70 { top[x][c] } else { bottom[x][c] };
                let horizontal = if x < 100 { left[y][c] } else { right[y][c] };
                let value = if dx == 0.0 {
                    vertical
                } else if dy == 0.0 {
                    horizontal
                } else {
                    (vertical + horizontal) * 0.5
                };
                rgb[c] = (8.0 + value * falloff * 0.9).min(255.0) as u32;
            }
            buffer[y * 800 + x] = (rgb[0] << 16) | (rgb[1] << 8) | rgb[2];
        }
    }
}
pub fn debug(colours: &[Rgb]) {
    println!(
        "{}",
        colours
            .iter()
            .map(|c| format!("{},{},{}", c.0, c.1, c.2))
            .collect::<Vec<_>>()
            .join(" ")
    );
}

#[cfg(test)]
mod sizing_tests {
    use super::*;
    #[test]
    fn minimize_suspends_rendering_until_restored() {
        assert!(!can_render(true, (1600, 800)));
        assert!(!can_render(true, (0, 0)));
        assert!(!can_render(false, (0, 800)));
        assert!(!can_render(false, (1600, 0)));
        assert!(can_render(false, (1600, 800)));
    }
    #[test]
    fn resized_editor_and_controls_match_scaled_picture() {
        assert_eq!(
            logical_mouse((620.0, 590.0), (1600, 800)),
            Some((310.0, 295.0))
        );
        // Square window has vertical margins; wider window has horizontal ones.
        assert_eq!(
            logical_mouse((400.0, 400.0), (800, 800)),
            Some((400.0, 200.0))
        );
        assert_eq!(
            logical_mouse((800.0, 400.0), (1600, 800)),
            Some((400.0, 200.0))
        );
        assert_eq!(
            logical_mouse((600.0, 200.0), (1200, 400)),
            Some((400.0, 200.0))
        );
        assert_eq!(logical_mouse((10.0, 100.0), (800, 800)), None);
        assert_eq!(logical_mouse((100.0, 200.0), (1200, 400)), None);
        assert_eq!(logical_mouse((0.0, 0.0), (0, 0)), None);
        assert_eq!(
            logical_mouse((400.5, 399.0), (801, 799)),
            Some((400.0, 200.0))
        );
    }
}
