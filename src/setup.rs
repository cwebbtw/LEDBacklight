//! User-calibrated strip geometry and an in-window first-run setup wizard.
use crate::{colour::Rgb, controls::text, ddp};
use anyhow::{Context, Result, ensure};
use minifb::Key;
use std::{
    net::SocketAddr,
    path::Path,
    time::{Duration, Instant},
};

const NAMES: [&str; 4] = ["TOP LEFT", "TOP RIGHT", "BOTTOM RIGHT", "BOTTOM LEFT"];
const SIDE_COLOURS: [[u8; 3]; 4] = [[48, 0, 0], [0, 48, 0], [0, 0, 48], [48, 32, 0]];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    pub address: SocketAddr,
    pub monitor: usize,
    pub leds: usize,
    pub clockwise: bool,
    /// One-based LED at each corner, or last LED before it along the strip.
    pub corners: [usize; 4],
    pub between: [bool; 4],
}
pub struct Mapping {
    pub counts: [usize; 4],
    physical: Vec<usize>,
    corner_indices: Vec<usize>,
}
impl Hardware {
    pub fn mapping(&self) -> Result<Mapping> {
        ensure!((4..=1200).contains(&self.leds), "Use 4 to 1200 LEDs");
        ensure!(
            self.corners.iter().all(|&n| n > 0 && n <= self.leds),
            "Corner LED outside strip"
        );
        let ring = (self.leds * 2) as isize;
        let anchor = |i: usize| ((self.corners[i] - 1) * 2 + usize::from(self.between[i])) as isize;
        let sign = if self.clockwise { 1 } else { -1 };
        let origin = anchor(0);
        let distance = |p: isize| ((p - origin) * sign).rem_euclid(ring) as usize;
        let anchors = [
            0,
            distance(anchor(1)),
            distance(anchor(2)),
            distance(anchor(3)),
            ring as usize,
        ];
        ensure!(
            anchors.windows(2).all(|p| p[0] < p[1]),
            "Corners must follow the chosen direction"
        );
        let mut order: Vec<_> = (0..self.leds)
            .map(|i| (distance((i * 2) as isize), i))
            .collect();
        order.sort_unstable();
        let mut counts = [0; 4];
        let mut physical = vec![0; self.leds];
        for (canonical, &(d, led)) in order.iter().enumerate() {
            let edge = (0..4)
                .find(|&e| d >= anchors[e] && d < anchors[e + 1])
                .unwrap();
            counts[edge] += 1;
            physical[led] = canonical;
        }
        ensure!(
            counts.iter().all(|&n| n > 0),
            "Each side needs at least one LED"
        );
        ensure!(
            counts[0] <= 800 && counts[2] <= 800 && counts[1] <= 400 && counts[3] <= 400,
            "Too many LEDs on one side for the preview"
        );
        let corner_indices = (0..4)
            .filter(|&i| !self.between[i])
            .map(|i| physical[self.corners[i] - 1])
            .collect();
        Ok(Mapping {
            counts,
            physical,
            corner_indices,
        })
    }
}
impl Mapping {
    pub fn output(&self, colours: &[Rgb]) -> Vec<[u8; 3]> {
        let n = self.physical.len();
        self.physical
            .iter()
            .map(|&i| {
                let c = if self.corner_indices.contains(&i) {
                    let a = colours[(i + n - 1) % n];
                    let b = colours[(i + 1) % n];
                    Rgb(
                        ((u16::from(a.0) + u16::from(b.0)) / 2) as u8,
                        ((u16::from(a.1) + u16::from(b.1)) / 2) as u8,
                        ((u16::from(a.2) + u16::from(b.2)) / 2) as u8,
                    )
                } else {
                    colours[i]
                };
                [c.0, c.1, c.2]
            })
            .collect()
    }
    fn sides(&self) -> Vec<[u8; 3]> {
        self.physical
            .iter()
            .map(|&i| {
                let mut offset = 0;
                for (e, &count) in self.counts.iter().enumerate() {
                    offset += count;
                    if i < offset {
                        return SIDE_COLOURS[e];
                    }
                }
                [0; 3]
            })
            .collect()
    }
}
pub fn save(path: &Path, h: &Hardware) -> Result<()> {
    h.mapping()?;
    let mut s = format!(
        "version=1\naddress={}\nmonitor={}\nleds={}\nclockwise={}\n",
        h.address, h.monitor, h.leds, h.clockwise
    );
    for i in 0..4 {
        s.push_str(&format!("corner{i}={},{}\n", h.corners[i], h.between[i]));
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, s)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
/// Saved fallback for the original installation; full Setup takes priority.
pub fn load_installed(path: &Path) -> Result<Option<SocketAddr>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(
            text.trim()
                .strip_prefix("address=")
                .context("Invalid installed strip settings")?
                .parse()?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn load(path: &Path) -> Result<Option<Hardware>> {
    let s = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let fields: std::collections::HashMap<_, _> =
        s.lines().filter_map(|l| l.split_once('=')).collect();
    let get = |k: &str| fields.get(k).copied().context("Incomplete hardware setup");
    ensure!(get("version")? == "1", "Unknown hardware setup version");
    let mut h = Hardware {
        address: get("address")?.parse()?,
        monitor: get("monitor")?.parse()?,
        leds: get("leds")?.parse()?,
        clockwise: get("clockwise")?.parse()?,
        corners: [0; 4],
        between: [false; 4],
    };
    for i in 0..4 {
        let key = format!("corner{i}");
        let (n, between) = get(&key)?.split_once(',').context("Invalid corner")?;
        h.corners[i] = n.parse()?;
        h.between[i] = between.parse()?;
    }
    h.mapping()?;
    Ok(Some(h))
}

pub struct Wizard {
    pub open: bool,
    pub apply: Option<Hardware>,
    draft: Hardware,
    saved: Hardware,
    address: String,
    number: String,
    step: usize,
    field: bool,
    caret: usize,
    was_down: bool,
    test: bool,
    sender: Option<ddp::Sender>,
    next_test: Instant,
    rotation_started: Instant,
    pub message: String,
    pub monitors: Vec<String>,
}
impl Wizard {
    pub fn new(draft: Hardware, monitors: Vec<String>) -> Self {
        Self {
            open: false,
            apply: None,
            address: draft.address.to_string(),
            number: draft.leds.to_string(),
            saved: draft.clone(),
            draft,
            step: 0,
            field: false,
            caret: 0,
            was_down: false,
            test: false,
            sender: None,
            next_test: Instant::now(),
            rotation_started: Instant::now(),
            message: String::new(),
            monitors,
        }
    }
    pub fn begin(&mut self) {
        self.draft = self.saved.clone();
        self.open = true;
        self.step = 0;
        self.test = false;
        self.address = self.draft.address.to_string();
        self.field = false;
        self.message.clear();
    }
    fn stop(&mut self) {
        if let Some(s) = &mut self.sender {
            let _ = s.send(&vec![[0; 3]; self.draft.leds]);
        }
        self.sender = None;
        self.test = false;
    }
    pub fn accepted(&mut self, h: Hardware) {
        self.stop();
        self.saved = h.clone();
        self.draft = h;
        self.open = false;
        self.message.clear();
    }
    fn commit_field(&mut self) -> Result<()> {
        if self.step == 0 {
            let address = if self.address.contains(':') {
                self.address.clone()
            } else {
                format!("{}:4048", self.address)
            };
            self.draft.address = address.parse().context("Enter controller IP and port")?;
        } else if self.step == 1 {
            let count: usize = self.number.parse().context("Enter the LED count")?;
            ensure!((4..=1200).contains(&count), "Use 4 to 1200 LEDs");
            if count != self.draft.leds {
                self.stop();
                self.draft.leds = count;
                // Neutral initial guesses; the user locates each corner next.
                self.draft.corners = std::array::from_fn(|i| 1 + i * count / 4);
                self.draft.between = [false; 4];
            }
        } else if (2..6).contains(&self.step) {
            let n: usize = self.number.parse().context("Enter a corner LED number")?;
            ensure!((1..=self.draft.leds).contains(&n), "LED outside strip");
            self.draft.corners[self.step - 2] = n;
        }
        Ok(())
    }
    fn change_step(&mut self, step: usize) {
        self.stop();
        self.message.clear();
        self.step = step;
        self.field = false;
        self.number = if step == 1 {
            self.draft.leds.to_string()
        } else if (2..6).contains(&step) {
            self.draft.corners[step - 2].to_string()
        } else {
            String::new()
        };
        self.next_test = Instant::now();
        self.rotation_started = self.next_test;
    }
    pub fn input(&mut self, mouse: Option<(f32, f32)>, down: bool, keys: &[Key]) {
        let fresh = down && !self.was_down;
        self.was_down = down;
        if !self.open {
            if fresh
                && mouse.is_some_and(|(x, y)| {
                    (655.0..=770.0).contains(&x) && (362.0..=390.0).contains(&y)
                })
            {
                self.begin();
            }
            return;
        }
        if self.field {
            let value = if self.step == 0 {
                &mut self.address
            } else {
                &mut self.number
            };
            for key in keys {
                edit_text(value, &mut self.caret, *key);
            }
            if (2..6).contains(&self.step) && !keys.is_empty() && self.commit_field().is_ok() {
                self.message.clear();
                self.next_test = Instant::now();
            }
        }
        if !fresh {
            return;
        }
        let Some((x, y)) = mouse else {
            return;
        };
        let field_y = if self.step == 1 { y + 20.0 } else { y };
        if (360.0..=750.0).contains(&x) && (135.0..=166.0).contains(&field_y) && self.step < 6 {
            self.field = true;
            let value = if self.step == 0 {
                &self.address
            } else {
                &self.number
            };
            self.caret = (((x - 365.0).max(0.0) / 12.0).round() as usize).min(value.len());
            return;
        }
        self.field = false;
        if (323.0..=349.0).contains(&y) {
            if (45.0..=160.0).contains(&x) {
                self.stop();
                self.open = false;
                return;
            }
            if (215.0..=325.0).contains(&x) && self.step > 0 {
                self.change_step(self.step - 1);
                return;
            }
            if (625.0..=760.0).contains(&x) {
                let result = self.commit_field().and_then(|()| {
                    if self.step == 6 {
                        self.draft.mapping()?;
                        self.apply = Some(self.draft.clone());
                    } else {
                        self.change_step(self.step + 1);
                    }
                    Ok(())
                });
                self.message = result.err().map(|e| e.to_string()).unwrap_or_default();
                return;
            }
        }
        let section_y = if self.step == 0 { y - 50.0 } else { y };
        if (195.0..=225.0).contains(&section_y) {
            if self.step == 0 {
                if (620.0..=750.0).contains(&x) && self.monitors.len() > 1 {
                    self.draft.monitor = (self.draft.monitor + 1) % self.monitors.len();
                }
            } else if self.step == 1 {
                self.draft.clockwise = !self.draft.clockwise;
            } else if (2..6).contains(&self.step) && (45.0..=750.0).contains(&x) {
                let corner = self.step - 2;
                self.draft.between[corner] = !self.draft.between[corner];
                if let Err(e) = self.commit_field() {
                    self.message = e.to_string();
                }
            }
        }
    }
    pub fn tick(&mut self) {
        if self.open && self.step > 0 && !self.test && self.message.is_empty() {
            let valid = if self.step == 6 {
                self.draft.mapping().map(|_| ())
            } else {
                Ok(())
            };
            match valid.and_then(|_| ddp::Sender::new(self.draft.address)) {
                Ok(sender) => {
                    self.sender = Some(sender);
                    self.test = true;
                    self.next_test = Instant::now();
                    self.rotation_started = self.next_test;
                }
                Err(e) => self.message = e.to_string(),
            }
        }
        if !self.open || !self.test || Instant::now() < self.next_test {
            return;
        }
        self.next_test = Instant::now() + Duration::from_millis(50);
        let mut pixels = vec![[0; 3]; self.draft.leds];
        if self.step == 6 {
            if let Ok(m) = self.draft.mapping() {
                pixels = m.sides();
            }
        } else if self.step == 1 {
            pixels = rotation_pixels(self.draft.leds, self.rotation_started.elapsed());
        } else {
            let i = self.draft.corners[self.step.saturating_sub(2).min(3)] - 1;
            pixels[i] = [48, 48, 48];
            if self.draft.between[self.step.saturating_sub(2).min(3)] {
                pixels[(i + 1) % self.draft.leds] = [48, 48, 48];
            }
        }
        if let Some(s) = &mut self.sender
            && let Err(e) = s.send(&pixels)
        {
            self.message = format!("SEND FAILED - {e}");
        }
    }
    fn test_instruction(&self) -> String {
        match self.step {
            1 => "WATCH FROM THE FRONT - CHOOSE ITS DIRECTION BELOW".into(),
            0 => String::new(),
            2..=5 => {
                let c = self.step - 2;
                let led = self.draft.corners[c];
                if self.draft.between[c] {
                    format!(
                        "LEDS {led} AND {} SHOULD STRADDLE {}",
                        led % self.draft.leds + 1,
                        NAMES[c]
                    )
                } else {
                    format!("LED {led} SHOULD BE WHITE AT {}", NAMES[c])
                }
            }
            _ => "DO THE SIDE COLOURS MATCH THE MONITOR?".into(),
        }
    }
    pub fn draw(&self, buffer: &mut [u32]) {
        if !self.open {
            text(buffer, 660, 370, "SETUP", 0x44cfff);
            return;
        }
        for y in 30..360 {
            buffer[y * 800 + 30..y * 800 + 770].fill(0x151515);
        }
        text(
            buffer,
            45,
            45,
            &format!("SETUP {} OF 7", self.step + 1),
            0xe4eaf2,
        );
        let title = match self.step {
            0 => "CONTROLLER AND SCREEN",
            1 => "STRIP LENGTH AND DIRECTION",
            2..=5 => NAMES[self.step - 2],
            _ => "CHECK THE FOUR SIDES",
        };
        text(buffer, 45, 85, title, 0x44cfff);
        if self.step == 0 {
            text(
                buffer,
                45,
                112,
                "SET THE CONNECTED LED COUNT IN WLED FIRST",
                0xc7d2e2,
            );
        }
        if self.step < 6 {
            let field_offset = if self.step == 1 { 20 } else { 0 };
            text(
                buffer,
                45,
                145 - field_offset,
                if self.step == 0 {
                    "IP AND PORT"
                } else if self.step == 1 {
                    "TOTAL LEDS"
                } else {
                    "CORNER LED"
                },
                0xc7d2e2,
            );
            for y in 135 - field_offset..166 - field_offset {
                buffer[y * 800 + 360..y * 800 + 750].fill(if self.field {
                    0x384857
                } else {
                    0x252d35
                });
            }
            text(
                buffer,
                365,
                145 - field_offset,
                if self.step == 0 {
                    &self.address
                } else {
                    &self.number
                },
                0xffffff,
            );
            if self.field {
                let x = (365 + self.caret * 12).min(748);
                for y in 142 - field_offset..161 - field_offset {
                    buffer[y * 800 + x] = 0x44cfff;
                }
            }
        }
        match self.step {
            0 => {
                text(buffer, 45, 230, "MONITOR WITH THE LED STRIP", 0xc7d2e2);
                if let Some(m) = self.monitors.get(self.draft.monitor) {
                    text(buffer, 45, 258, &format!("MONITOR {m}"), 0xffffff);
                }
                if self.monitors.len() > 1 {
                    text(buffer, 625, 258, "CHANGE", 0x44cfff);
                }
            }
            1 => {
                text(buffer, 45, 180, &self.test_instruction(), 0xffbb44);
                let direction = if self.draft.clockwise {
                    "DIRECTION IS CLOCKWISE"
                } else {
                    "DIRECTION IS ANTICLOCKWISE"
                };
                text(buffer, 45, 208, direction, 0xffffff);
                text(
                    buffer,
                    45 + (direction.len() + 3) * 12,
                    208,
                    "CLICK TO CHANGE",
                    0x44cfff,
                );
            }
            2..=5 => {
                text(
                    buffer,
                    45,
                    180,
                    "CHANGE THE NUMBER TO LIGHT UP THAT LED",
                    0xc7d2e2,
                );
                text(
                    buffer,
                    45,
                    208,
                    if self.draft.between[self.step - 2] {
                        "CORNER IS BETWEEN TWO LEDS - CHANGE"
                    } else {
                        "CORNER HAS AN LED - CHANGE"
                    },
                    0x44cfff,
                );
                text(
                    buffer,
                    45,
                    258,
                    if self.draft.between[self.step - 2] {
                        "ENTER LAST LED BEFORE THE CORNER"
                    } else {
                        "ENTER THE LED AT THE CORNER"
                    },
                    0xc7d2e2,
                );
            }
            _ => {
                if self.draft.mapping().is_ok() {
                    text(buffer, 45, 180, "TOP RED - RIGHT GREEN", 0xc7d2e2);
                    text(buffer, 45, 208, "BOTTOM BLUE - LEFT YELLOW", 0xc7d2e2);
                } else {
                    text(buffer, 45, 145, "GO BACK AND CHECK CORNER ORDER", 0xff8888);
                }
            }
        }
        let instruction = if self.step > 1 && self.test && self.message.is_empty() {
            self.test_instruction()
        } else {
            self.message.clone()
        };
        let msg: String = instruction.to_uppercase().chars().take(58).collect();
        text(
            buffer,
            45,
            if self.step == 1 || (2..6).contains(&self.step) {
                307
            } else {
                292
            },
            &msg,
            0xffbb44,
        );
        text(buffer, 45, 333, "CANCEL", 0xc7d2e2);
        if self.step > 0 {
            text(buffer, 215, 333, "BACK", 0xc7d2e2);
        }
        text(
            buffer,
            625,
            333,
            if self.step == 6 { "SAVE" } else { "NEXT" },
            0x44cfff,
        );
    }
}
impl Drop for Wizard {
    fn drop(&mut self) {
        self.stop();
    }
}
/// Chase increasing physical LED numbers. The user observes the actual wiring
/// direction; their direction choice must not reverse this diagnostic.
fn rotation_pixels(leds: usize, elapsed: Duration) -> Vec<[u8; 3]> {
    let head = ((elapsed.as_millis() % 3000) * leds as u128 / 3000) as usize;
    let mut pixels = vec![[0; 3]; leds];
    let width = 5.min((leds / 4).max(1));
    for offset in 0..width {
        pixels[(head + leds - offset) % leds] = [255, 0, 0];
    }
    for tail in width..width + 2 {
        pixels[(head + leds - tail) % leds] = [128, 0, 0];
    }
    pixels
}
fn edit_text(value: &mut String, caret: &mut usize, key: Key) {
    *caret = (*caret).min(value.len());
    match key {
        Key::Left => *caret = caret.saturating_sub(1),
        Key::Right => *caret = (*caret + 1).min(value.len()),
        Key::Home => *caret = 0,
        Key::End => *caret = value.len(),
        Key::Backspace if *caret > 0 => {
            *caret -= 1;
            value.remove(*caret);
        }
        Key::Delete if *caret < value.len() => {
            value.remove(*caret);
        }
        _ => {
            if value.len() < 50
                && let Some(c) = typed(key)
            {
                value.insert(*caret, c);
                *caret += 1;
            }
        }
    }
}
fn typed(key: Key) -> Option<char> {
    Some(match key {
        Key::Key0 => '0',
        Key::Key1 => '1',
        Key::Key2 => '2',
        Key::Key3 => '3',
        Key::Key4 => '4',
        Key::Key5 => '5',
        Key::Key6 => '6',
        Key::Key7 => '7',
        Key::Key8 => '8',
        Key::Key9 => '9',
        Key::Period => '.',
        Key::Semicolon => ':',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corner_number_edits_apply_live_and_keep_last_valid_selection() {
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.begin();
        w.change_step(2);
        w.input(Some((450.0, 150.0)), true, &[]);
        w.input(None, false, &[Key::End, Key::Backspace, Key::Key5]);
        assert_eq!(w.draft.corners[0], 45);
        w.input(None, false, &[Key::Home, Key::Delete, Key::Delete]);
        assert_eq!(w.draft.corners[0], 45);
        w.input(None, false, &[Key::Key9, Key::Key9]);
        w.input(None, false, &[Key::Key9]);
        assert_eq!(w.draft.corners[0], 99);
        assert!(w.sender.is_none());
    }
    #[test]
    fn rotation_follows_led_order_and_repeats_without_expiring() {
        for leds in [4, 140, 300, 1200] {
            let first = rotation_pixels(leds, Duration::ZERO);
            let quarter = rotation_pixels(leds, Duration::from_millis(750));
            assert_eq!(first[0], [255, 0, 0]);
            assert_eq!(quarter[leds / 4], [255, 0, 0]);
            assert_eq!(rotation_pixels(leds, Duration::from_millis(3000)), first);
            assert_eq!(
                rotation_pixels(leds, Duration::from_millis(300750)),
                quarter
            );
        }
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.begin();
        w.change_step(1);
        w.input(Some((650.0, 259.0)), true, &[]);
        assert!(w.sender.is_none());
        assert!(!w.test); // Removed LED 1 control has no hotspot.
    }
    #[test]
    fn connection_page_has_no_light_tests_or_hidden_test_clicks() {
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.begin();
        w.input(Some((80.0, 259.0)), true, &[]);
        w.input(None, false, &[]);
        w.input(Some((650.0, 259.0)), true, &[]);
        assert!(!w.test);
        assert!(w.sender.is_none());
        w.test = true;
        w.change_step(0);
        assert!(!w.test);
    }
    #[test]
    fn test_instructions_describe_the_expected_physical_lights() {
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.change_step(1);
        assert_eq!(
            w.test_instruction(),
            "WATCH FROM THE FRONT - CHOOSE ITS DIRECTION BELOW"
        );
        w.change_step(2);
        assert_eq!(w.test_instruction(), "LED 44 SHOULD BE WHITE AT TOP LEFT");
        w.change_step(4);
        assert_eq!(
            w.test_instruction(),
            "LEDS 113 AND 114 SHOULD STRADDLE BOTTOM RIGHT"
        );
        w.change_step(6);
        assert_eq!(
            w.test_instruction(),
            "DO THE SIDE COLOURS MATCH THE MONITOR?"
        );
    }
    #[test]
    fn monitor_changes_only_with_its_change_button() {
        let mut w = Wizard::new(
            h(),
            vec!["1 - 3440 X 1440".into(), "2 - 1920 X 1080".into()],
        );
        w.begin();
        w.input(Some((100.0, 258.0)), true, &[]);
        assert_eq!(w.draft.monitor, 0);
        w.input(None, false, &[]);
        w.input(Some((650.0, 258.0)), true, &[]);
        assert_eq!(w.draft.monitor, 1);
        w.input(None, false, &[]);
        w.input(Some((650.0, 258.0)), true, &[]);
        assert_eq!(w.draft.monitor, 0);
    }
    #[test]
    fn address_edits_at_clicked_cursor_without_replacing_other_characters() {
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.begin();
        w.input(Some((509.0, 150.0)), true, &[]); // Cursor after the last IP digit.
        w.input(None, false, &[Key::Backspace, Key::Key4]);
        assert_eq!(w.address, "192.168.4.24:4048");
        assert_eq!(w.caret, 12);
        w.input(None, false, &[Key::End, Key::Left, Key::Delete, Key::Key9]);
        assert_eq!(w.address, "192.168.4.24:4049");
        w.commit_field().unwrap();
        assert_eq!(w.draft.address, "192.168.4.24:4049".parse().unwrap());
    }
    #[test]
    fn field_navigation_and_deletion_clamp_at_both_ends() {
        let mut s = String::from("12");
        let mut cursor = 0;
        for key in [Key::Left, Key::Backspace, Key::Home] {
            edit_text(&mut s, &mut cursor, key);
        }
        assert_eq!(s, "12");
        assert_eq!(cursor, 0);
        edit_text(&mut s, &mut cursor, Key::Key3);
        assert_eq!(s, "312");
        for key in [Key::End, Key::Right, Key::Delete] {
            edit_text(&mut s, &mut cursor, key);
        }
        assert_eq!(s, "312");
        assert_eq!(cursor, 3);
        edit_text(&mut s, &mut cursor, Key::Backspace);
        assert_eq!(s, "31");
    }
    fn h() -> Hardware {
        Hardware {
            address: "192.168.4.23:4048".parse().unwrap(),
            monitor: 0,
            leds: 140,
            clockwise: true,
            corners: [44, 93, 113, 24],
            between: [false, false, true, false],
        }
    }
    #[test]
    fn geometry_handles_different_counts_starts_and_direction() {
        for n in [12, 37, 140, 300, 600] {
            for shift in [0, 1, n / 3, n - 1] {
                for clockwise in [true, false] {
                    let mut a = h();
                    a.leds = n;
                    a.clockwise = clockwise;
                    a.between = [false, true, false, true];
                    a.corners = std::array::from_fn(|i| {
                        let offset = i * n / 4;
                        1 + if clockwise {
                            (shift + offset) % n
                        } else {
                            (shift + n - offset) % n
                        }
                    });
                    let m = a.mapping().unwrap();
                    assert_eq!(m.counts.iter().sum::<usize>(), n);
                    let mut p = m.physical.clone();
                    p.sort_unstable();
                    assert_eq!(p, (0..n).collect::<Vec<_>>());
                    let colours: Vec<_> = m
                        .sides()
                        .into_iter()
                        .map(|c| Rgb(c[0], c[1], c[2]))
                        .collect();
                    assert_eq!(m.output(&colours).len(), n);
                }
            }
        }
    }
    #[test]
    fn wizard_edits_count_completes_and_cancel_discards_draft() {
        fn click(w: &mut Wizard, x: f32, y: f32) {
            w.input(None, false, &[]);
            w.input(Some((x, y)), true, &[]);
            w.input(None, false, &[]);
        }
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.begin();
        click(&mut w, 680.0, 333.0); // Connection -> length.
        click(&mut w, 450.0, 130.0);
        w.input(
            None,
            false,
            &[
                Key::Home,
                Key::Delete,
                Key::Delete,
                Key::Delete,
                Key::Key1,
                Key::Key2,
            ],
        );
        click(&mut w, 680.0, 333.0);
        assert_eq!(w.draft.leds, 12);
        assert_eq!(w.step, 2);
        for _ in 0..4 {
            click(&mut w, 680.0, 333.0);
        }
        assert_eq!(w.step, 6);
        click(&mut w, 680.0, 333.0);
        assert_eq!(w.apply.as_ref().unwrap().leds, 12);
        w.apply = None;
        click(&mut w, 80.0, 333.0);
        w.begin();
        assert_eq!(w.draft, h());
    }
    #[test]
    #[ignore = "offline wizard visual render"]
    fn render_setup_pages() {
        let mut w = Wizard::new(h(), vec!["1 - 3440 X 1440".into()]);
        w.begin();
        for step in 0..7 {
            w.change_step(step);
            let mut source = vec![0x151515; 800 * 400];
            w.draw(&mut source);
            let mut native = vec![0; 1600 * 800];
            crate::output::upscale_ui(&source, &mut native, 1600, 800);
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
            std::fs::write(format!("target/setup-{step}.bmp"), bmp).unwrap();
        }
    }
    #[test]
    fn arbitrary_bottom_start_maps_each_led_and_between_corner() {
        let m = h().mapping().unwrap();
        assert_eq!(m.counts, [49, 21, 50, 20]);
        assert_eq!(m.physical[43], 0);
        assert_eq!(m.physical[92], 49);
        assert_eq!(m.physical[113], 70);
        assert_eq!(m.physical[0], 97);
        let mut indexes = m.physical.clone();
        indexes.sort_unstable();
        assert_eq!(indexes, (0..140).collect::<Vec<_>>());
    }
    #[test]
    fn reversed_and_rotated_strips_keep_screen_order() {
        let mut a = h();
        let expected = a.mapping().unwrap().physical;
        a.clockwise = false;
        for i in 0..4 {
            a.corners[i] = 141 - a.corners[i];
            if a.between[i] {
                a.corners[i] = a.corners[i].saturating_sub(1).max(1);
            }
        }
        let actual = a.mapping().unwrap().physical;
        for i in 0..140 {
            assert_eq!(expected[i], actual[139 - i]);
        }
        a.corners = [1, 1, 1, 1];
        assert!(a.mapping().is_err());
    }
    #[test]
    fn setup_roundtrip_and_wizard_cancel_does_not_apply() {
        let path = std::env::temp_dir().join(format!(
            "backlight-hardware-test-{}.ini",
            std::process::id()
        ));
        save(&path, &h()).unwrap();
        assert_eq!(load(&path).unwrap(), Some(h()));
        std::fs::remove_file(path).unwrap();
        let mut w = Wizard::new(h(), vec!["SCREEN".into()]);
        w.input(Some((700.0, 375.0)), true, &[]);
        assert!(w.open);
        w.input(None, false, &[]);
        w.input(Some((80.0, 333.0)), true, &[]);
        assert!(!w.open);
        assert!(w.apply.is_none());
    }
}
