use crate::{colour::Rgb, controls::Settings};
use anyhow::Result;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub struct View {
    pub editor: crate::editor::Editor,
    pub setup: crate::setup::Wizard,
    pub profiles: crate::profiles::Picker,
    pub settings: Option<Settings>,
    pub effective_brightness: f64,
    pub colours: Vec<Rgb>,
    pub counts: [usize; 4],
    pub raw: bool,
    pub status: String,
    pub alive: bool,
    pub context: Option<eframe::egui::Context>,
    pub hwnd: isize,
    pub(crate) next_repaint: Instant,
}
impl View {
    pub fn interact(&mut self, settings: &mut Settings) {
        if let Some(current) = self.settings {
            *settings = current;
        } else {
            self.settings = Some(*settings);
        }
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
        _settings: Settings,
        effective: f64,
    ) -> Result<()> {
        self.colours.clear();
        self.colours.extend_from_slice(colours);
        self.status = status.into();
        self.effective_brightness = effective;
        if Instant::now() >= self.next_repaint {
            if let Some(ctx) = &self.context {
                ctx.request_repaint();
            }
            self.next_repaint = Instant::now() + Duration::from_millis(16);
        }
        Ok(())
    }
}
pub struct Preview {
    state: Arc<Mutex<View>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Preview {
    pub fn new(counts: [usize; 4], raw: bool, setup: crate::setup::Wizard) -> Result<Self> {
        let state = Arc::new(Mutex::new(View {
            editor: crate::editor::Editor::new(counts),
            setup,
            profiles: crate::profiles::Picker::default(),
            settings: None,
            effective_brightness: 0.0,
            colours: vec![Rgb(0, 0, 0); counts.iter().sum()],
            counts,
            raw,
            status: "Waiting for capture".into(),
            alive: true,
            context: None,
            hwnd: 0,
            next_repaint: Instant::now(),
        }));
        let gui_state = state.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            let options = eframe::NativeOptions {
                viewport: eframe::egui::ViewportBuilder::default()
                    .with_inner_size([1200.0, 800.0])
                    .with_min_inner_size([900.0, 650.0]),
                event_loop_builder: Some(Box::new(|builder| {
                    builder.with_any_thread(true);
                })),
                persist_window: false,
                ..Default::default()
            };
            let app_state = gui_state.clone();
            let initialized = ready_tx.clone();
            let result = eframe::run_native(
                "Backlight",
                options,
                Box::new(move |cc| {
                    let app = crate::gui::App::new(cc, app_state);
                    let _ = initialized.send(Ok(()));
                    Ok(Box::new(app))
                }),
            );
            if let Err(e) = result {
                let _ = ready_tx.send(Err(e.to_string()));
            }
            let mut view = gui_state.lock().unwrap_or_else(|e| e.into_inner());
            view.alive = false;
            view.hwnd = 0;
        });
        match ready_rx.recv_timeout(Duration::from_secs(45)) {
            Ok(Ok(())) => Ok(Self {
                state,
                thread: Some(thread),
            }),
            Ok(Err(e)) => anyhow::bail!("Unable to open Backlight: {e}"),
            Err(e) => anyhow::bail!("Unable to start the Backlight window: {e}"),
        }
    }
    pub fn lock(&self) -> MutexGuard<'_, View> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn open(&self) -> bool {
        self.lock().alive
    }
    pub fn sampling_exclusion(
        &self,
        origin: (i32, i32),
        bounds: crate::layout::Region,
    ) -> Option<crate::layout::Region> {
        use windows::Win32::{
            Foundation::{HWND, RECT},
            UI::WindowsAndMessaging::*,
        };
        let raw = self.lock().hwnd;
        if raw == 0 {
            return None;
        }
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        let mut rect = RECT::default();
        unsafe {
            if IsIconic(hwnd).as_bool()
                || !IsWindowVisible(hwnd).as_bool()
                || GetWindowRect(hwnd, &mut rect).is_err()
            {
                return None;
            }
        }
        crate::taskbar::clipped(rect, origin, bounds, true)
    }
}
impl Drop for Preview {
    fn drop(&mut self) {
        {
            let mut view = self.lock();
            view.alive = false;
            if let Some(ctx) = &view.context {
                ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(test)]
fn can_render(minimized: bool, (w, h): (usize, usize)) -> bool {
    !minimized && w > 0 && h > 0
}

/// Invert minifb's Windows aspect-ratio scaling, including integer letterbox
/// offsets. Its get_mouse_pos does not account for resized buffer dimensions.
#[cfg(test)]
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
#[cfg(test)]
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
