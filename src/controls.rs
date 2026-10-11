use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub brightness: f64,
    pub adaptive: bool,
    pub whiteness_correction: bool,
    pub fps: u32,
    pub smoothing_ms: u64,
}
impl Settings {
    pub fn period(self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.fps))
    }
}

const LEFT: f32 = 310.0;
const RIGHT: f32 = 670.0;
const ROWS: [f32; 3] = [155.0, 225.0, 295.0];

#[derive(Default)]
pub struct Controls {
    active: Option<usize>,
    was_down: bool,
}
impl Controls {
    pub fn input(&mut self, mouse: Option<(f32, f32)>, down: bool, settings: &mut Settings) {
        if !down {
            self.active = None;
        } else if let Some((x, y)) = mouse {
            if !self.was_down && (110.0..=370.0).contains(&x) && (96.0..=117.0).contains(&y) {
                settings.adaptive = !settings.adaptive;
                self.active = None;
            }
            if !self.was_down && (420.0..=705.0).contains(&x) && (96.0..=117.0).contains(&y) {
                settings.whiteness_correction = !settings.whiteness_correction;
                self.active = None;
            }
            if !self.was_down && (LEFT - 12.0..=RIGHT + 12.0).contains(&x) {
                self.active = ROWS.iter().position(|row| (y - row).abs() <= 16.0);
                if settings.adaptive && self.active == Some(0) {
                    self.active = None;
                }
            }
            if let Some(row) = self.active {
                let t = f64::from(((x - LEFT) / (RIGHT - LEFT)).clamp(0.0, 1.0));
                match row {
                    0 => settings.brightness = (t * 200.0).round() / 100.0,
                    1 => settings.fps = (1.0 + t * 239.0).round() as u32,
                    _ => settings.smoothing_ms = (t * 500.0).round() as u64,
                }
            }
        }
        self.was_down = down;
    }
    pub fn draw(&self, buffer: &mut [u32], settings: Settings, effective_brightness: f64) {
        text(buffer, 110, 75, "LIVE LIGHTING CONTROLS", 0xe4eaf2);
        for (left, checked, label) in [
            (110, settings.adaptive, "ADAPTIVE BRIGHTNESS"),
            (420, settings.whiteness_correction, "WHITENESS CORRECTION"),
        ] {
            for y in 97..115 {
                for x in left..left + 18 {
                    let border = x == left || x == left + 17 || y == 97 || y == 114;
                    buffer[y * 800 + x] = if border {
                        0xc7d2e2
                    } else if checked {
                        0x4eabdc
                    } else {
                        0x151515
                    };
                }
            }
            text(buffer, left + 30, 99, label, 0xc7d2e2);
        }
        let labels = [
            if settings.adaptive {
                format!("AUTO BRIGHTNESS {:.2}", effective_brightness)
            } else {
                format!("BRIGHTNESS {:.2}", settings.brightness)
            },
            format!("UPDATE RATE {} HZ", settings.fps),
            format!("SMOOTHING {} MS", settings.smoothing_ms),
        ];
        let values = [
            if settings.adaptive {
                effective_brightness / 2.0
            } else {
                settings.brightness / 2.0
            },
            (settings.fps - 1) as f64 / 239.0,
            settings.smoothing_ms.min(500) as f64 / 500.0,
        ];
        for (i, row) in ROWS.iter().enumerate() {
            let y = *row as usize;
            let disabled = i == 0 && settings.adaptive;
            text(
                buffer,
                110,
                y - 32,
                &labels[i],
                if disabled { 0x8393a7 } else { 0xc7d2e2 },
            );
            let knob =
                (f64::from(LEFT) + values[i].clamp(0.0, 1.0) * f64::from(RIGHT - LEFT)) as usize;
            for yy in y - 3..=y + 3 {
                for x in LEFT as usize..=RIGHT as usize {
                    buffer[yy * 800 + x] = if x <= knob && !disabled {
                        0x4eabdc
                    } else {
                        0x38404c
                    };
                }
            }
            for yy in y - 9..=y + 9 {
                buffer[yy * 800 + knob - 5..yy * 800 + knob + 6].fill(if disabled {
                    0x687487
                } else {
                    0xe8f2ff
                });
            }
        }
        text(
            buffer,
            110,
            335,
            "DRAG TO APPLY - 0 MS IS INSTANT",
            0x8393a7,
        );
    }
}

// Small embedded bitmap font keeps the existing lightweight preview dependency.
pub(crate) fn text(buffer: &mut [u32], x: usize, y: usize, value: &str, colour: u32) {
    for (i, c) in value.chars().enumerate() {
        let rows = match c {
            'A' => [14, 17, 17, 31, 17, 17, 17],
            'B' => [30, 17, 17, 30, 17, 17, 30],
            'C' => [14, 17, 16, 16, 16, 17, 14],
            'D' => [30, 17, 17, 17, 17, 17, 30],
            'E' => [31, 16, 16, 30, 16, 16, 31],
            'F' => [31, 16, 16, 30, 16, 16, 16],
            'G' => [14, 17, 16, 23, 17, 17, 15],
            'H' => [17, 17, 17, 31, 17, 17, 17],
            'I' => [14, 4, 4, 4, 4, 4, 14],
            'J' => [7, 2, 2, 2, 18, 18, 12],
            'K' => [17, 18, 20, 24, 20, 18, 17],
            'L' => [16, 16, 16, 16, 16, 16, 31],
            'M' => [17, 27, 21, 21, 17, 17, 17],
            'N' => [17, 25, 21, 19, 17, 17, 17],
            'O' => [14, 17, 17, 17, 17, 17, 14],
            'P' => [30, 17, 17, 30, 16, 16, 16],
            'Q' => [14, 17, 17, 17, 21, 18, 13],
            'R' => [30, 17, 17, 30, 20, 18, 17],
            'S' => [15, 16, 16, 14, 1, 1, 30],
            'T' => [31, 4, 4, 4, 4, 4, 4],
            'U' => [17, 17, 17, 17, 17, 17, 14],
            'V' => [17, 17, 17, 17, 17, 10, 4],
            'W' => [17, 17, 17, 21, 21, 21, 10],
            'X' => [17, 17, 10, 4, 10, 17, 17],
            'Y' => [17, 17, 10, 4, 4, 4, 4],
            'Z' => [31, 1, 2, 4, 8, 16, 31],
            '0' => [14, 17, 19, 21, 25, 17, 14],
            '1' => [4, 12, 4, 4, 4, 4, 14],
            '2' => [14, 17, 1, 2, 4, 8, 31],
            '3' => [30, 1, 1, 14, 1, 1, 30],
            '4' => [2, 6, 10, 18, 31, 2, 2],
            '5' => [31, 16, 16, 30, 1, 1, 30],
            '6' => [14, 16, 16, 30, 17, 17, 14],
            '7' => [31, 1, 2, 4, 8, 8, 8],
            '8' => [14, 17, 17, 14, 17, 17, 14],
            '9' => [14, 17, 17, 15, 1, 1, 14],
            '.' => [0, 0, 0, 0, 0, 6, 6],
            ':' => [0, 6, 6, 0, 6, 6, 0],
            '-' => [0, 0, 0, 31, 0, 0, 0],
            '(' => [2, 4, 8, 8, 8, 4, 2],
            ')' => [8, 4, 2, 2, 2, 4, 8],
            'c' => [0, 0, 14, 16, 16, 17, 14],
            'u' => [0, 0, 17, 17, 17, 19, 13],
            'r' => [0, 0, 22, 25, 16, 16, 16],
            'e' => [0, 0, 14, 17, 31, 16, 14],
            'n' => [0, 0, 22, 25, 17, 17, 17],
            't' => [8, 8, 30, 8, 8, 9, 6],
            _ => [0; 7],
        };
        for (yy, bits) in rows.iter().enumerate() {
            for xx in 0..5 {
                if bits & (1 << (4 - xx)) != 0 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let px = x + i * 12 + xx * 2 + dx;
                            let py = y + yy * 2 + dy;
                            if px < 800 && py < 400 {
                                buffer[py * 800 + px] = colour;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_marker_renders_every_character_including_parentheses() {
        let mut buffer = vec![0; 800 * 400];
        text(&mut buffer, 0, 0, "(current)", 0xffffff);
        for character in 0..9 {
            assert!((0..14).any(|y| {
                buffer[y * 800 + character * 12..y * 800 + character * 12 + 10]
                    .iter()
                    .any(|&pixel| pixel != 0)
            }));
        }
    }
    #[test]
    fn whiteness_checkbox_toggles_independently_once_per_press() {
        let mut c = Controls::default();
        let mut s = Settings {
            brightness: 0.75,
            adaptive: false,
            whiteness_correction: false,
            fps: 100,
            smoothing_ms: 0,
        };
        c.input(Some((430.0, 106.0)), true, &mut s);
        assert!(s.whiteness_correction && !s.adaptive);
        c.input(Some((430.0, 106.0)), true, &mut s);
        assert!(s.whiteness_correction);
        c.input(None, false, &mut s);
        c.input(Some((120.0, 106.0)), true, &mut s);
        assert!(s.whiteness_correction && s.adaptive);
        c.input(None, false, &mut s);
        c.input(Some((430.0, 106.0)), true, &mut s);
        assert!(!s.whiteness_correction && s.adaptive);
        assert_eq!(s.brightness, 0.75);
        assert_eq!(s.fps, 100);
        assert_eq!(s.smoothing_ms, 0);
    }
    #[test]
    fn adaptive_toggle_disables_manual_slider_and_retains_its_value() {
        let mut c = Controls::default();
        let mut s = Settings {
            brightness: 0.75,
            adaptive: false,
            whiteness_correction: false,
            fps: 100,
            smoothing_ms: 0,
        };
        c.input(Some((120.0, 106.0)), true, &mut s);
        assert!(s.adaptive);
        c.input(Some((120.0, 106.0)), true, &mut s);
        assert!(s.adaptive); // Holding the button does not toggle repeatedly.
        c.input(None, false, &mut s);
        c.input(Some((RIGHT, ROWS[0])), true, &mut s);
        assert_eq!(s.brightness, 0.75);
        c.input(None, false, &mut s);
        c.input(Some((120.0, 106.0)), true, &mut s);
        assert!(!s.adaptive);
        assert_eq!(s.brightness, 0.75);
        c.input(None, false, &mut s);
        c.input(Some((LEFT, ROWS[0])), true, &mut s);
        assert_eq!(s.brightness, 0.0);
    }
    #[test]
    fn dragging_stays_on_selected_slider_and_clamps() {
        let mut c = Controls::default();
        let mut s = Settings {
            brightness: 0.35,
            adaptive: false,
            whiteness_correction: false,
            fps: 60,
            smoothing_ms: 40,
        };
        c.input(Some((LEFT, ROWS[0])), true, &mut s);
        assert_eq!(s.brightness, 0.0);
        c.input(Some((RIGHT + 100.0, ROWS[1])), true, &mut s);
        assert_eq!(s.brightness, 2.0);
        assert_eq!(s.fps, 60);
        c.input(None, false, &mut s);
        c.input(Some((RIGHT, ROWS[1])), true, &mut s);
        assert_eq!(s.fps, 240);
        c.input(Some((LEFT - 100.0, ROWS[1])), true, &mut s);
        assert_eq!(s.fps, 1);
        assert_eq!(s.period(), Duration::from_secs(1));
        c.input(None, false, &mut s);
        c.input(Some((LEFT, ROWS[2])), true, &mut s);
        assert_eq!(s.smoothing_ms, 0);
        c.input(Some((RIGHT, ROWS[2])), true, &mut s);
        assert_eq!(s.smoothing_ms, 500);
    }
}
