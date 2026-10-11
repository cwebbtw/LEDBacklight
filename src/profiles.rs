//! Named sampling layouts and their in-window selector.
use crate::{controls::text, sampling};
use anyhow::{Result, ensure};
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
    let wow = path(dir, "WoW")?;
    if !wow.exists()
        && let Some(p) = sampling::load(&dir.join("backlight-sampling-wow.ini"))?
    {
        sampling::save(&wow, &p)?;
        if !dir.join("backlight-active-profile.txt").exists() {
            remember(dir, "WoW")?;
        }
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
    name: String,
    was_down: bool,
}
impl Picker {
    pub fn input(&mut self, mouse: Option<(f32, f32)>, down: bool, keys: &[Key]) -> bool {
        let fresh = down && !self.was_down;
        self.was_down = down;
        if !self.open {
            if fresh
                && mouse.is_some_and(|(x, y)| {
                    (280.0..=620.0).contains(&x) && (362.0..=390.0).contains(&y)
                })
            {
                self.open = true;
                self.creating = false;
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
        if fresh && let Some((x, y)) = mouse {
            if (323.0..=349.0).contains(&y) {
                if (45.0..=200.0).contains(&x) {
                    self.open = false;
                } else if (580.0..=760.0).contains(&x) {
                    if self.creating {
                        self.request = Some(Request::Create(self.name.trim().to_string()));
                    } else if let Some(name) = self.names.get(self.index) {
                        self.request = Some(Request::Select(name.clone()));
                    }
                }
            } else if !self.creating && (175.0..=203.0).contains(&y) && !self.names.is_empty() {
                if x < 350.0 {
                    self.index = (self.index + self.names.len() - 1) % self.names.len();
                } else {
                    self.index = (self.index + 1) % self.names.len();
                }
            } else if !self.creating && (245.0..=278.0).contains(&y) {
                self.creating = true;
                self.name.clear();
                self.message.clear();
            }
        }
        true
    }
    pub fn draw(&self, buffer: &mut [u32]) {
        if !self.open {
            let label: String = format!("PROFILE: {}", self.active)
                .chars()
                .take(28)
                .collect();
            text(buffer, 285, 370, &label.to_uppercase(), 0x44cfff);
            return;
        }
        for y in 30..360 {
            buffer[y * 800 + 30..y * 800 + 770].fill(0x151515);
        }
        text(buffer, 45, 65, "SAMPLING PROFILES", 0xe4eaf2);
        if self.creating {
            text(buffer, 45, 105, "NEW PROFILE NAME", 0xc7d2e2);
            text(
                buffer,
                45,
                145,
                &format!("{}|", self.name.to_uppercase()),
                0xffffff,
            );
            text(
                buffer,
                45,
                205,
                "COPIES YOUR CURRENT SAMPLING BOXES",
                0xc7d2e2,
            );
        } else {
            if let Some(name) = self.names.get(self.index) {
                text(buffer, 45, 130, &name.to_uppercase(), 0xffffff);
            }
            text(buffer, 45, 185, "PREVIOUS", 0x44cfff);
            text(buffer, 575, 185, "NEXT", 0x44cfff);
            text(buffer, 45, 259, "NEW PROFILE", 0x44cfff);
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
            if self.creating {
                "CREATE"
            } else {
                "USE PROFILE"
            },
            0x44cfff,
        );
    }
}
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
