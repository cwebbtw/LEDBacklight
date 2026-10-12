//! Named sampling layouts and their in-window selector.
#[cfg(test)]
use crate::controls::text;
use crate::sampling;
use anyhow::{Result, ensure};
#[cfg(test)]
use minifb::Key;
use std::path::{Path, PathBuf};

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 24
        && name.trim() == name
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
}
pub fn path(dir: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        valid_name(name),
        "Use up to 24 letters, numbers, spaces or hyphens"
    );
    Ok(dir.join(format!(
        "backlight-profile-{}.ini",
        name.to_ascii_lowercase()
    )))
}
pub fn names(dir: &Path) -> Result<Vec<String>> {
    let mut names = vec!["Default".to_string()];
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let filename = item.file_name().to_string_lossy().into_owned();
        if let Some(name) = filename
            .strip_prefix("backlight-profile-")
            .and_then(|s| s.strip_suffix(".ini"))
            && valid_name(name)
            && !name.eq_ignore_ascii_case("default")
        {
            names.push(if name == "wow" {
                "WoW".into()
            } else {
                name.into()
            });
        }
    }
    names[1..].sort_by_key(|s| s.to_ascii_lowercase());
    Ok(names)
}
pub fn remember(dir: &Path, name: &str) -> Result<()> {
    ensure!(valid_name(name), "Invalid profile name");
    std::fs::write(dir.join("backlight-active-profile.txt"), name)?;
    Ok(())
}
/// Copy the existing layout once; never replace a subsequently edited profile.
pub fn migrate(dir: &Path) -> Result<bool> {
    let marker = dir.join("backlight-profile-migration.txt");
    if marker.exists() {
        return Ok(false);
    }
    let wow = path(dir, "WoW")?;
    if wow.exists() {
        std::fs::write(marker, "complete")?;
        return Ok(false);
    }
    if !wow.exists()
        && let Some(p) = sampling::load(&dir.join("backlight-sampling-wow.ini"))?
    {
        sampling::save(&wow, &p)?;
        if !dir.join("backlight-active-profile.txt").exists() {
            remember(dir, "WoW")?;
        }
        std::fs::write(marker, "complete")?;
        return Ok(true);
    }
    Ok(false)
}
pub fn selected(dir: &Path, requested: Option<&str>) -> Result<String> {
    let name = requested
        .map(str::to_string)
        .or_else(|| std::fs::read_to_string(dir.join("backlight-active-profile.txt")).ok())
        .unwrap_or_else(|| "Default".into());
    ensure!(valid_name(&name), "Invalid profile name");
    ensure!(
        name.eq_ignore_ascii_case("default") || path(dir, &name)?.exists(),
        "Profile does not exist: {name}"
    );
    Ok(if name.eq_ignore_ascii_case("default") {
        "Default".into()
    } else {
        name
    })
}

pub enum Request {
    Select(String),
    Create(String),
    Delete(String),
}
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    pub active: String,
    pub names: Vec<String>,
    pub request: Option<Request>,
    pub message: String,
    index: usize,
    creating: bool,
    deleting: bool,
    name: String,
    #[cfg(test)]
    was_down: bool,
    #[cfg(test)]
    dropdown: bool,
    #[cfg(test)]
    list_start: usize,
    focus_name: bool,
}
impl Picker {
    #[cfg(test)]
    pub fn preview_mode(&mut self, creating: bool, deleting: bool) {
        self.begin();
        self.creating = creating;
        self.deleting = deleting;
    }
    pub fn begin(&mut self) {
        self.open = true;
        self.creating = false;
        self.deleting = false;
        #[cfg(test)]
        {
            self.dropdown = false;
        }
        self.index = self
            .names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(&self.active))
            .unwrap_or(0);
        self.message.clear();
    }
    pub fn modern_ui(&mut self, ui: &mut eframe::egui::Ui) {
        use eframe::egui::{self, RichText};
        ui.vertical_centered(|ui| {
            ui.set_max_width(650.0);
            crate::gui::card().show(ui, |ui| {
                ui.set_width(590.0);
                if self.creating {
                    crate::gui::title(
                        ui,
                        "New sampling profile",
                        "Start with clean default sampling sections.",
                    );
                    ui.label("Profile name");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .margin(egui::Margin::symmetric(12, 10))
                            .hint_text("e.g. Movies")
                            .char_limit(24)
                            .desired_width(f32::INFINITY),
                    );
                    if self.focus_name {
                        response.request_focus();
                        self.focus_name = false;
                    }
                    if response.changed() {
                        self.message.clear();
                    }
                } else if self.deleting {
                    crate::gui::title(
                        ui,
                        "Delete profile",
                        "This removes the saved sampling layout.",
                    );
                    if let Some(name) = self.names.get(self.index) {
                        ui.label(RichText::new(name).size(24.0).strong());
                        if name.eq_ignore_ascii_case(&self.active) {
                            ui.label("Default will become the current profile.");
                        }
                    }
                } else {
                    crate::gui::title(
                        ui,
                        "Sampling profiles",
                        "Choose which sampling layout to edit.",
                    );
                    ui.label("Profile");
                    let selected = self
                        .names
                        .get(self.index)
                        .map(|name| {
                            if name.eq_ignore_ascii_case(&self.active) {
                                format!("{name} (current)")
                            } else {
                                name.clone()
                            }
                        })
                        .unwrap_or_else(|| "Default".into());
                    egui::ComboBox::from_id_salt("sampling-profile")
                        .selected_text(selected)
                        .width(ui.available_width() - 20.0)
                        .show_ui(ui, |ui| {
                            for (index, name) in self.names.iter().enumerate() {
                                let label = if name.eq_ignore_ascii_case(&self.active) {
                                    format!("{name} (current)")
                                } else {
                                    name.clone()
                                };
                                ui.selectable_value(&mut self.index, index, label);
                            }
                        });
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        if ui.button("New profile").clicked() {
                            self.creating = true;
                            self.focus_name = true;
                            self.name.clear();
                            self.message.clear();
                        }
                        if self
                            .names
                            .get(self.index)
                            .is_some_and(|name| !name.eq_ignore_ascii_case("default"))
                            && ui.button("Delete profile").clicked()
                        {
                            self.deleting = true;
                            self.message.clear();
                        }
                    });
                }
                if !self.message.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(250, 158, 122), &self.message);
                }
                ui.add_space(32.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        if self.deleting {
                            self.deleting = false;
                            self.message.clear();
                        } else {
                            self.open = false;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.deleting {
                            "Delete"
                        } else if self.creating {
                            "Create profile"
                        } else {
                            "Apply"
                        };
                        let clicked = if self.deleting {
                            ui.add(
                                egui::Button::new(label).fill(egui::Color32::from_rgb(119, 45, 56)),
                            )
                            .clicked()
                        } else {
                            crate::gui::primary(ui, label).clicked()
                        };
                        if clicked {
                            self.request = if self.creating {
                                Some(Request::Create(self.name.trim().to_string()))
                            } else {
                                self.names.get(self.index).map(|name| {
                                    if self.deleting {
                                        Request::Delete(name.clone())
                                    } else {
                                        Request::Select(name.clone())
                                    }
                                })
                            };
                        }
                    });
                });
            });
        });
    }
    #[cfg(test)]
    pub fn input(&mut self, mouse: Option<(f32, f32)>, down: bool, keys: &[Key]) -> bool {
        let fresh = down && !self.was_down;
        self.was_down = down;
        if !self.open {
            if fresh
                && mouse.is_some_and(|(x, y)| {
                    (430.0..=770.0).contains(&x) && (362.0..=390.0).contains(&y)
                })
            {
                self.open = true;
                self.creating = false;
                self.deleting = false;
                #[cfg(test)]
                {
                    self.dropdown = false;
                }
                self.index = self
                    .names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(&self.active))
                    .unwrap_or(0);
                self.message.clear();
            }
            return self.open;
        }
        if self.creating {
            if !keys.is_empty() {
                self.message.clear();
            }
            for key in keys {
                if *key == Key::Backspace {
                    self.name.pop();
                } else if self.name.len() < 24
                    && let Some(c) = character(*key)
                {
                    self.name.push(c);
                }
            }
        }
        if self.dropdown && !self.names.is_empty() {
            for key in keys {
                match key {
                    Key::Up => self.index = self.index.saturating_sub(1),
                    Key::Down => self.index = (self.index + 1).min(self.names.len() - 1),
                    Key::Home => self.index = 0,
                    Key::End => self.index = self.names.len() - 1,
                    Key::Enter => self.dropdown = false,
                    _ => (),
                }
            }
            if self.index < self.list_start {
                self.list_start = self.index;
            }
            if self.index >= self.list_start + 6 {
                self.list_start = self.index - 5;
            }
        }
        if fresh && let Some((x, y)) = mouse {
            if self.dropdown {
                if (45.0..=750.0).contains(&x) && (154.0..298.0).contains(&y) {
                    let index = self.list_start + ((y - 154.0) / 24.0) as usize;
                    if index < self.names.len() {
                        self.index = index;
                    }
                }
                #[cfg(test)]
                {
                    self.dropdown = false;
                }
                return true;
            }
            if (323.0..=349.0).contains(&y) {
                if (45.0..=200.0).contains(&x) {
                    if self.deleting {
                        self.deleting = false;
                    } else {
                        self.open = false;
                    }
                } else if (580.0..=760.0).contains(&x) {
                    if self.deleting {
                        if let Some(name) = self.names.get(self.index) {
                            self.request = Some(Request::Delete(name.clone()));
                        }
                    } else if self.creating {
                        self.request = Some(Request::Create(self.name.trim().to_string()));
                    } else if let Some(name) = self.names.get(self.index) {
                        self.request = Some(Request::Select(name.clone()));
                    }
                }
            } else if !self.creating
                && !self.deleting
                && (115.0..=150.0).contains(&y)
                && (45.0..=750.0).contains(&x)
            {
                self.dropdown = true;
                self.list_start = self.index.saturating_sub(5);
            } else if !self.creating
                && !self.deleting
                && (259.0..273.0).contains(&y)
                && (45.0..175.0).contains(&x)
            {
                self.creating = true;
                self.name.clear();
                self.message.clear();
            } else if !self.creating
                && !self.deleting
                && (259.0..273.0).contains(&y)
                && (450.0..616.0).contains(&x)
                && self
                    .names
                    .get(self.index)
                    .is_some_and(|name| !name.eq_ignore_ascii_case("default"))
            {
                self.deleting = true;
                self.message.clear();
            }
        }
        true
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn draw(&self, buffer: &mut [u32]) {
        if !self.open {
            let label: String = format!("PROFILE: {}", self.active)
                .chars()
                .take(28)
                .collect();
            text(buffer, 430, 370, &label.to_uppercase(), 0x44cfff);
            return;
        }
        for y in 30..360 {
            buffer[y * 800 + 30..y * 800 + 770].fill(0x151515);
        }
        text(buffer, 45, 65, "SAMPLING PROFILES", 0xe4eaf2);
        if self.creating {
            text(buffer, 45, 105, "NEW PROFILE NAME", 0xc7d2e2);
            for y in 132..166 {
                buffer[y * 800 + 45..y * 800 + 750].fill(0x384857);
            }
            let name_label = self.name.to_uppercase();
            text(
                buffer,
                55,
                145,
                if self.name.is_empty() {
                    "TYPE PROFILE NAME"
                } else {
                    &name_label
                },
                if self.name.is_empty() {
                    0x8796a7
                } else {
                    0xffffff
                },
            );
            let cursor = 55 + self.name.len() * 12;
            for y in 141..161 {
                buffer[y * 800 + cursor..y * 800 + cursor + 2].fill(0x44cfff);
            }
            text(
                buffer,
                45,
                205,
                "STARTS WITH CLEAN DEFAULT SAMPLING",
                0xc7d2e2,
            );
        } else if self.deleting {
            text(buffer, 45, 115, "DELETE THIS PROFILE?", 0xffbb44);
            if let Some(name) = self.names.get(self.index) {
                text(buffer, 45, 165, &self.label(name), 0xffffff);
                if name.eq_ignore_ascii_case(&self.active) {
                    text(buffer, 45, 225, "THIS WILL SWITCH TO DEFAULT", 0xc7d2e2);
                }
            }
        } else {
            for y in 115..151 {
                buffer[y * 800 + 45..y * 800 + 750].fill(0x252d35);
            }
            if let Some(name) = self.names.get(self.index) {
                text(buffer, 55, 128, &self.label(name), 0xffffff);
            }
            for row in 0..8 {
                let y = if self.dropdown { 135 - row } else { 128 + row };
                buffer[y * 800 + 715 + row..y * 800 + 731 - row].fill(0x44cfff);
            }
            if !self.deleting {
                text(buffer, 45, 259, "NEW PROFILE", 0x44cfff);
                if self
                    .names
                    .get(self.index)
                    .is_some_and(|name| !name.eq_ignore_ascii_case("default"))
                {
                    text(buffer, 450, 259, "DELETE PROFILE", 0x44cfff);
                }
            }
        }
        text(
            buffer,
            45,
            295,
            &self
                .message
                .to_uppercase()
                .chars()
                .take(58)
                .collect::<String>(),
            0xffbb44,
        );
        text(buffer, 45, 333, "CANCEL", 0xc7d2e2);
        text(
            buffer,
            580,
            333,
            if self.deleting {
                "DELETE"
            } else if self.creating {
                "CREATE"
            } else {
                "APPLY"
            },
            0x44cfff,
        );
        if self.dropdown {
            for (row, name) in self.names.iter().skip(self.list_start).take(6).enumerate() {
                let top = 154 + row * 24;
                let colour = if self.index == self.list_start + row {
                    0x384857
                } else {
                    0x252d35
                };
                for y in top..top + 24 {
                    buffer[y * 800 + 45..y * 800 + 750].fill(colour);
                }
                text(buffer, 55, top + 5, &self.label(name), 0xffffff);
            }
        }
    }
    #[cfg(test)]
    fn label(&self, name: &str) -> String {
        if name.eq_ignore_ascii_case(&self.active) {
            format!("{} (current)", name.to_uppercase())
        } else {
            name.to_uppercase()
        }
    }
}
#[cfg(test)]
fn character(key: Key) -> Option<char> {
    Some(match key {
        Key::A => 'a',
        Key::B => 'b',
        Key::C => 'c',
        Key::D => 'd',
        Key::E => 'e',
        Key::F => 'f',
        Key::G => 'g',
        Key::H => 'h',
        Key::I => 'i',
        Key::J => 'j',
        Key::K => 'k',
        Key::L => 'l',
        Key::M => 'm',
        Key::N => 'n',
        Key::O => 'o',
        Key::P => 'p',
        Key::Q => 'q',
        Key::R => 'r',
        Key::S => 's',
        Key::T => 't',
        Key::U => 'u',
        Key::V => 'v',
        Key::W => 'w',
        Key::X => 'x',
        Key::Y => 'y',
        Key::Z => 'z',
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
        Key::Space => ' ',
        Key::Minus => '-',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_custom_profile_requires_confirmation_and_default_is_protected() {
        let mut picker = Picker {
            open: true,
            active: "WoW".into(),
            names: vec!["Default".into(), "WoW".into()],
            index: 1,
            ..Default::default()
        };
        fn click(p: &mut Picker, x: f32, y: f32) {
            p.input(Some((x, y)), true, &[]);
            p.input(None, false, &[]);
        }
        click(&mut picker, 500.0, 265.0);
        assert!(picker.deleting);
        assert!(picker.request.is_none());
        click(&mut picker, 100.0, 333.0);
        assert!(!picker.deleting && picker.open);
        click(&mut picker, 500.0, 265.0);
        click(&mut picker, 600.0, 333.0);
        assert!(matches!(picker.request, Some(Request::Delete(ref name)) if name == "WoW"));
        picker.request = None;
        picker.deleting = false;
        picker.index = 0;
        click(&mut picker, 500.0, 265.0);
        assert!(!picker.deleting && picker.request.is_none());
    }
    #[test]
    fn new_profile_ignores_blank_space_beside_its_label() {
        let mut picker = Picker {
            open: true,
            ..Default::default()
        };
        for point in [
            (300.0, 265.0),
            (40.0, 265.0),
            (100.0, 250.0),
            (100.0, 275.0),
        ] {
            picker.input(Some(point), true, &[]);
            picker.input(None, false, &[]);
            assert!(!picker.creating);
        }
        picker.input(Some((100.0, 265.0)), true, &[]);
        assert!(picker.creating);
    }
    #[test]
    fn dropdown_marks_current_and_applies_only_after_confirmation() {
        let mut picker = Picker {
            active: "WoW".into(),
            names: vec!["Default".into(), "WoW".into()],
            ..Default::default()
        };
        fn click(p: &mut Picker, x: f32, y: f32) {
            p.input(Some((x, y)), true, &[]);
            p.input(None, false, &[]);
        }
        click(&mut picker, 500.0, 375.0);
        assert_eq!(picker.label("WoW"), "WOW (current)");
        click(&mut picker, 500.0, 130.0);
        click(&mut picker, 100.0, 160.0);
        assert_eq!(picker.index, 0);
        assert_eq!(picker.active, "WoW");
        assert!(picker.request.is_none());
        click(&mut picker, 600.0, 333.0);
        assert!(matches!(picker.request, Some(Request::Select(ref name)) if name == "Default"));
    }
    #[test]
    fn dropdown_can_reach_profiles_beyond_visible_rows() {
        let mut picker = Picker {
            open: true,
            dropdown: true,
            names: (0..15).map(|n| format!("Profile {n}")).collect(),
            ..Default::default()
        };
        picker.input(None, false, &[Key::End]);
        assert_eq!(picker.index, 14);
        assert_eq!(picker.list_start, 9);
        picker.input(Some((100.0, 160.0)), true, &[]);
        assert_eq!(picker.index, 9);
        assert!(!picker.dropdown);
    }
    #[test]
    fn migration_preserves_all_geometry_and_only_runs_once() {
        let dir = std::env::temp_dir().join(format!("backlight-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut original = sampling::Profile::preset(3440, 1440, 120, 240, true);
        original
            .parts
            .insert(49, sampling::Rect(9800, 100, 10000, 400));
        sampling::save(&dir.join("backlight-sampling-wow.ini"), &original).unwrap();
        assert!(migrate(&dir).unwrap());
        assert_eq!(
            sampling::load(&path(&dir, "WoW").unwrap()).unwrap(),
            Some(original.clone())
        );
        assert_eq!(selected(&dir, None).unwrap(), "WoW");
        let clean = sampling::Profile::preset(3440, 1440, 120, 0, false);
        sampling::save(&path(&dir, "WoW").unwrap(), &clean).unwrap();
        remember(&dir, "Default").unwrap();
        assert!(!migrate(&dir).unwrap());
        assert_eq!(selected(&dir, None).unwrap(), "Default");
        assert_eq!(
            sampling::load(&path(&dir, "WoW").unwrap()).unwrap(),
            Some(clean)
        );
        assert_eq!(
            sampling::load(&dir.join("backlight-sampling-wow.ini")).unwrap(),
            Some(original)
        );
        assert_eq!(names(&dir).unwrap(), vec!["Default", "WoW"]);
        std::fs::remove_file(path(&dir, "WoW").unwrap()).unwrap();
        assert!(!migrate(&dir).unwrap());
        assert!(!path(&dir, "WoW").unwrap().exists());
        for item in std::fs::read_dir(&dir).unwrap() {
            std::fs::remove_file(item.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn profile_names_cannot_escape_settings_directory() {
        for name in [
            "",
            "..",
            "../other",
            "C:\\other",
            " bad",
            "bad ",
            "a/b",
            "a.b",
        ] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("Movie night-2"));
        assert!(!valid_name(&"a".repeat(25)));
    }
}
