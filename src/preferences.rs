use crate::controls::Settings;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

pub fn load(path: &std::path::Path) -> Result<Option<Settings>> {
    match fs::read_to_string(path) {
        Ok(text) => decode(&text)
            .map(Some)
            .context("invalid saved lighting settings"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("cannot read saved lighting settings"),
    }
}

fn encode(s: Settings) -> String {
    format!(
        "version=2\nbrightness={}\nadaptive={}\nfps={}\nsmoothing_ms={}\nwhiteness_correction={}\n",
        s.brightness, s.adaptive, s.fps, s.smoothing_ms, s.whiteness_correction
    )
}
/// Explicit CLI values win; launcher defaults no longer mask saved controls.
pub fn restore(defaults: Settings, saved: Option<Settings>, explicit: [bool; 5]) -> Settings {
    let mut s = saved.unwrap_or(defaults);
    if explicit[0] {
        s.brightness = defaults.brightness;
    }
    if explicit[1] {
        s.adaptive = defaults.adaptive;
    }
    if explicit[2] {
        s.fps = defaults.fps;
    }
    if explicit[3] {
        s.smoothing_ms = defaults.smoothing_ms;
    }
    if explicit[4] {
        s.whiteness_correction = defaults.whiteness_correction;
    }
    s
}
fn decode(text: &str) -> Result<Settings> {
    let mut fields = std::collections::HashMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (key, value) = line.split_once('=').context("expected key=value")?;
        ensure!(
            fields.insert(key.trim(), value.trim()).is_none(),
            "duplicate setting"
        );
    }
    let field = |key| fields.get(key).copied().context("missing setting");
    let version = field("version")?;
    ensure!(
        version == "1" || version == "2",
        "unsupported settings version"
    );
    ensure!(
        fields.len() == if version == "1" { 5 } else { 6 },
        "missing or unknown setting"
    );
    let s = Settings {
        brightness: field("brightness")?.parse()?,
        adaptive: field("adaptive")?.parse()?,
        whiteness_correction: if version == "1" {
            false
        } else {
            field("whiteness_correction")?.parse()?
        },
        fps: field("fps")?.parse()?,
        smoothing_ms: field("smoothing_ms")?.parse()?,
    };
    ensure!(
        s.brightness.is_finite() && (0.0..=2.0).contains(&s.brightness),
        "brightness must be 0..2"
    );
    ensure!((1..=240).contains(&s.fps), "fps must be 1..240");
    Ok(s)
}

pub struct Store {
    path: PathBuf,
    pending: Option<Settings>,
    due: Instant,
}
impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            pending: None,
            due: Instant::now(),
        }
    }
    pub fn schedule(&mut self, settings: Settings) {
        self.pending = Some(settings);
        self.due = Instant::now() + Duration::from_millis(300);
    }
    pub fn flush(&mut self, force: bool) -> Result<()> {
        if !force && Instant::now() < self.due {
            return Ok(());
        }
        let Some(settings) = self.pending else {
            return Ok(());
        };
        // Write beside the destination, then replace it atomically. A failed
        // write never truncates the last successfully saved settings file.
        let temp = self
            .path
            .with_extension(format!("{}.tmp", std::process::id()));
        let result = (|| -> Result<()> {
            let mut file = fs::File::create(&temp)?;
            file.write_all(encode(settings).as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_ok() {
            self.pending = None;
        } else {
            self.due = Instant::now() + Duration::from_secs(5);
        }
        result.context("cannot save lighting settings")
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.flush(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previous_settings_migrate_without_resetting_controls() {
        let old = "version=1\nbrightness=1.77\nadaptive=true\nfps=100\nsmoothing_ms=200\n";
        let mut s = decode(old).unwrap();
        assert_eq!(s.brightness, 1.77);
        assert!(s.adaptive);
        assert_eq!(s.fps, 100);
        assert_eq!(s.smoothing_ms, 200);
        assert!(!s.whiteness_correction);
        s.whiteness_correction = true;
        assert_eq!(decode(&encode(s)).unwrap(), s);
    }
    #[test]
    fn saved_values_win_over_defaults_but_explicit_cli_wins() {
        let defaults = Settings {
            brightness: 0.75,
            adaptive: false,
            whiteness_correction: false,
            fps: 100,
            smoothing_ms: 0,
        };
        let saved = Settings {
            brightness: 1.4,
            adaptive: true,
            whiteness_correction: true,
            fps: 144,
            smoothing_ms: 50,
        };
        assert_eq!(restore(defaults, None, [false; 5]), defaults);
        assert_eq!(restore(defaults, Some(saved), [false; 5]), saved);
        assert_eq!(restore(defaults, Some(saved), [true; 5]), defaults);
        let mixed = restore(defaults, Some(saved), [false, false, true, false, false]);
        assert_eq!(mixed.fps, 100);
        assert_eq!(mixed.brightness, 1.4);
        assert!(mixed.adaptive);
        assert_eq!(mixed.smoothing_ms, 50);
    }
    #[test]
    fn rejects_invalid_settings_and_round_trips_all_controls() {
        let s = Settings {
            brightness: 1.73,
            adaptive: true,
            whiteness_correction: true,
            fps: 144,
            smoothing_ms: 27,
        };
        let encoded = encode(s);
        assert_eq!(decode(&encoded).unwrap(), s);
        for invalid in [
            encoded.replace("fps=144", "fps=0"),
            encoded.replace("fps=144", "fps=241"),
            encoded.replace("brightness=1.73", "brightness=NaN"),
            encoded.replace("brightness=1.73", "brightness=2.01"),
            encoded.replace("adaptive=true", "adaptive=maybe"),
            encoded.replace("version=2", "version=3"),
            encoded.replace("whiteness_correction=true", "whiteness_correction=maybe"),
            format!("{encoded}fps=100\n"),
            "brightness=1.0\n".into(),
        ] {
            assert!(decode(&invalid).is_err());
        }
    }
    #[test]
    fn saves_replaces_and_reloads_without_losing_manual_value() -> Result<()> {
        let path = std::env::temp_dir().join(format!(
            "backlight-settings-{}-{}.ini",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        assert!(load(&path)?.is_none());
        let mut store = Store::new(path.clone());
        let mut s = Settings {
            brightness: 0.75,
            adaptive: true,
            whiteness_correction: true,
            fps: 100,
            smoothing_ms: 0,
        };
        store.schedule(s);
        store.flush(false)?;
        assert!(!path.exists()); // Debounced, rather than writing on each mouse event.
        store.flush(true)?;
        assert_eq!(load(&path)?, Some(s));
        s.fps = 60;
        s.adaptive = false;
        store.schedule(s);
        store.flush(true)?;
        assert_eq!(load(&path)?, Some(s));
        fs::write(&path, "broken")?;
        assert!(load(&path).is_err());
        fs::remove_file(path)?;
        Ok(())
    }
}
