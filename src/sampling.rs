//! Saved sampling geometry, expressed in 1/10000 of the active picture.
use crate::{layout::Region, ui};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect(pub usize, pub usize, pub usize, pub usize);
impl Rect {
    pub fn pixels(self, w: usize, h: usize) -> Region {
        let x = (self.0 * w / 10000).min(w - 1);
        let y = (self.1 * h / 10000).min(h - 1);
        Region {
            x,
            y,
            width: (self.2 * w / 10000).max(x + 1).min(w) - x,
            height: (self.3 * h / 10000).max(y + 1).min(h) - y,
        }
    }
    fn valid(self) -> bool {
        self.0 < self.2 && self.1 < self.3 && self.2 <= 10000 && self.3 <= 10000
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub bands: [Rect; 4],
    pub exclusions: Vec<Rect>,
    pub parts: BTreeMap<usize, Rect>,
}
impl Profile {
    pub fn preset(w: usize, h: usize, depth: usize, inset: usize, wow: bool) -> Self {
        let dx = (depth.min(w) * 10000 / w).max(1);
        let dy = (depth.min(h - inset) * 10000 / h).max(1);
        let bottom = 10000 - inset * 10000 / h;
        Self {
            parts: BTreeMap::new(),
            bands: [
                Rect(0, 0, 10000, dy),
                Rect(10000 - dx, 0, 10000, 10000),
                Rect(0, bottom - dy, 10000, bottom),
                Rect(0, 0, dx, 10000),
            ],
            exclusions: if wow {
                ui::wow_zones(10000, 10000)
                    .iter()
                    .map(|r| Rect(r.x, r.y, r.x + r.width, r.y + r.height))
                    .collect()
            } else {
                vec![]
            },
        }
    }
    /// Clockwise subdivision retains physical LED ordering. Exclusions relocate
    /// readings to nearby visible pixels; they cannot reveal the world behind UI.
    pub fn regions(&self, w: usize, h: usize, counts: [usize; 4]) -> (Vec<Region>, Vec<bool>) {
        self.regions_with_zones(w, h, counts, &[])
    }
    pub fn regions_with_zones(
        &self,
        w: usize,
        h: usize,
        counts: [usize; 4],
        extra: &[Region],
    ) -> (Vec<Region>, Vec<bool>) {
        let mut zones: Vec<_> = self.exclusions.iter().map(|r| r.pixels(w, h)).collect();
        zones.extend_from_slice(extra);
        let mut result = Vec::new();
        let mut blocked = Vec::new();
        for (edge, &count) in counts.iter().enumerate() {
            let band = self.bands[edge].pixels(w, h);
            let length = if edge % 2 == 0 {
                band.width
            } else {
                band.height
            };
            for i in 0..count {
                let index = if edge >= 2 { count - 1 - i } else { i };
                let start = index * length / count;
                let end = ((index + 1) * length / count).max(start + 1).min(length);
                let mut r = if edge % 2 == 0 {
                    Region {
                        x: band.x + start,
                        y: band.y,
                        width: end - start,
                        height: band.height,
                    }
                } else {
                    Region {
                        x: band.x,
                        y: band.y + start,
                        width: band.width,
                        height: end - start,
                    }
                };
                if let Some(rect) = self.parts.get(&result.len()) {
                    r = rect.pixels(w, h);
                }
                // Bottom readings keep their horizontal span even when depth
                // must shrink. Never jump sideways to another LED's position.
                loop {
                    if edge == 2 {
                        ui::avoid_bottom(std::slice::from_mut(&mut r), &zones);
                    } else {
                        ui::avoid(std::slice::from_mut(&mut r), &zones, w, h);
                    }
                    let hit = zones.iter().any(|z| overlaps(r, *z));
                    if !hit || (r.height == 1 && (edge == 2 || r.width == 1)) {
                        blocked.push(hit);
                        break;
                    }
                    if edge != 2 {
                        r.width = r.width.div_ceil(2);
                    }
                    r.height = r.height.div_ceil(2);
                }
                result.push(r);
            }
        }
        (result, blocked)
    }
}
pub fn overlaps(a: Region, b: Region) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
pub fn encode(p: &Profile) -> String {
    let mut text = String::from("version=2\n");
    for (name, r) in ["top", "right", "bottom", "left"]
        .into_iter()
        .zip(&p.bands)
        .chain(p.exclusions.iter().map(|r| ("exclude", r)))
    {
        text.push_str(&format!("{name}={},{},{},{}\n", r.0, r.1, r.2, r.3));
    }
    for (index, r) in &p.parts {
        text.push_str(&format!("part_{index}={},{},{},{}\n", r.0, r.1, r.2, r.3));
    }
    text
}
pub fn decode(text: &str) -> Result<Profile> {
    let mut lines = text.lines();
    let version = lines.next();
    ensure!(
        matches!(version, Some("version=1" | "version=2")),
        "unsupported sampling layout"
    );
    let mut bands = [None; 4];
    let mut exclusions = Vec::new();
    let mut parts = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once('=').context("invalid layout field")?;
        let values: Vec<usize> = value
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        ensure!(values.len() == 4, "expected four coordinates");
        let rect = Rect(values[0], values[1], values[2], values[3]);
        ensure!(rect.valid(), "sampling box outside picture or empty");
        if key == "exclude" {
            ensure!(exclusions.len() < 32, "too many exclusion boxes");
            exclusions.push(rect);
        } else if let Some(index) = key.strip_prefix("part_") {
            ensure!(
                version == Some("version=2"),
                "parts require layout version 2"
            );
            let index: usize = index.parse()?;
            ensure!(
                index < 10000 && parts.len() < 10000,
                "invalid reading index"
            );
            ensure!(
                parts.insert(index, rect).is_none(),
                "duplicate reading override"
            );
        } else {
            let i = ["top", "right", "bottom", "left"]
                .iter()
                .position(|n| *n == key)
                .context("unknown layout field")?;
            ensure!(bands[i].is_none(), "duplicate sampling band");
            bands[i] = Some(rect);
        }
    }
    ensure!(bands.iter().all(Option::is_some), "missing sampling band");
    Ok(Profile {
        bands: bands.map(Option::unwrap),
        exclusions,
        parts,
    })
}
pub fn load(path: &Path) -> Result<Option<Profile>> {
    match std::fs::read_to_string(path) {
        Ok(text) => decode(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn save(path: &Path, p: &Profile) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&temp)?;
    use std::io::Write;
    file.write_all(encode(p).as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temp, path)?;
    Ok(())
}
pub fn save_optional(path: &Path, p: Option<&Profile>) -> Result<()> {
    if let Some(p) = p {
        save(path, p)
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_override_keeps_horizontal_span_and_shrinks_depth_if_needed() {
        let mut p = Profile::preset(1000, 500, 20, 0, false);
        p.parts.insert(12, Rect(1000, 9600, 2000, 10000));
        p.exclusions.push(Rect(900, 100, 2100, 10000));
        let (r, blocked) = p.regions(1000, 500, [10, 2, 10, 2]);
        assert_eq!((r[12].x, r[12].width), (100, 100));
        assert!(r[12].y + r[12].height <= 5);
        assert!(!blocked[12]);
        p.exclusions[0].1 = 0;
        let (r, blocked) = p.regions(1000, 500, [10, 2, 10, 2]);
        assert_eq!((r[12].x, r[12].width), (100, 100));
        assert!(blocked[12]);
    }
    #[test]
    fn saved_layout_round_trips_and_rejects_bad_boxes() {
        let p = Profile::preset(3440, 1440, 120, 240, true);
        let text = encode(&p);
        assert_eq!(decode(&text).unwrap(), p);
        for bad in [
            text.replace("version=2", "version=3"),
            format!("{text}top=0,0,1,1\n"),
            format!("{text}exclude=0,0,10001,10000\n"),
            format!("{text}exclude=5,0,5,100\n"),
        ] {
            assert!(decode(&bad).is_err());
        }
    }
    #[test]
    fn bright_icons_excluded_but_world_lamp_retained() {
        let mut p = Profile::preset(1000, 500, 60, 80, false);
        p.exclusions = vec![Rect(7000, 0, 8000, 1500), Rect(9500, 3000, 10000, 10000)];
        let (regions, blocked) = p.regions(1000, 500, [48, 20, 50, 19]);
        assert_eq!(regions.len(), 137);
        assert!(!blocked.contains(&true));
        let mut data = vec![10; 1000 * 500 * 4];
        for rect in p
            .exclusions
            .iter()
            .copied()
            .chain([Rect(2000, 0, 2400, 1000)])
        {
            let r = rect.pixels(1000, 500);
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    data[(y * 1000 + x) * 4..(y * 1000 + x) * 4 + 4]
                        .copy_from_slice(&[0, 220, 255, 255]);
                }
            }
        }
        let samples = crate::colour::sample(&data, 1000, 500, 4000, &regions).unwrap();
        assert!(samples[..48].iter().any(|s| s.highlight.0 > 200));
        for (i, s) in samples.iter().enumerate() {
            if !(9..=11).contains(&i) {
                assert_eq!(s.average, crate::colour::Rgb(10, 10, 10));
            }
        }
    }
    #[test]
    fn tiny_bands_keep_every_led_in_bounds_and_clockwise() {
        let mut p = Profile::preset(3440, 1440, 120, 240, false);
        p.bands = [Rect(4900, 4900, 5000, 5000); 4];
        let (rs, blocked) = p.regions(100, 50, [48, 20, 50, 19]);
        assert_eq!(rs.len(), 137);
        assert!(!blocked.contains(&true));
        assert!(
            rs.iter().all(|r| r.width > 0
                && r.height > 0
                && r.x + r.width <= 100
                && r.y + r.height <= 50)
        );
        p.exclusions = vec![Rect(0, 0, 10000, 10000)];
        assert!(p.regions(100, 50, [48, 20, 50, 19]).1.iter().all(|b| *b));
    }
    #[test]
    fn save_reload_replace_and_reset_layout_file() {
        let path =
            std::env::temp_dir().join(format!("backlight-layout-{}.ini", std::process::id()));
        let mut p = Profile::preset(3440, 1440, 120, 240, true);
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), Some(p.clone()));
        p.bands[0] = Rect(0, 1500, 10000, 2000);
        p.exclusions.push(Rect(200, 200, 500, 500));
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), Some(p));
        save_optional(&path, None).unwrap();
        assert_eq!(load(&path).unwrap(), None);
    }
    #[test]
    fn visible_taskbar_and_saved_exclusions_apply_together_to_individual_parts() {
        let mut p = Profile::preset(1000, 500, 60, 0, false);
        let bar = Region {
            x: 0,
            y: 450,
            width: 1000,
            height: 50,
        };
        p.exclusions = vec![Rect(4000, 7000, 8000, 8000)];
        p.parts.insert(10, Rect(5000, 9100, 5200, 10000));
        let hidden = p.regions(1000, 500, crate::strip::COUNTS).0;
        assert!(overlaps(hidden[10], bar));
        let (visible, blocked) = p.regions_with_zones(1000, 500, crate::strip::COUNTS, &[bar]);
        assert_eq!(visible.len(), 137);
        assert!(!blocked.contains(&true));
        let excluded = p.exclusions[0].pixels(1000, 500);
        assert!(
            visible
                .iter()
                .all(|r| !overlaps(*r, bar) && !overlaps(*r, excluded))
        );
        let mut bytes = vec![0; 1000 * 500 * 4];
        for y in 450..500 {
            for x in 0..1000 {
                bytes[(y * 1000 + x) * 4 + 1] = 255;
            }
        }
        let colours = crate::colour::average(&bytes, 1000, 500, 4000, &visible).unwrap();
        assert!(colours.iter().all(|c| *c == crate::colour::Rgb::default()));
    }
    #[test]
    fn moving_one_part_preserves_neighbors_order_and_old_saved_layouts() {
        let mut p = Profile::preset(1000, 500, 60, 80, false);
        let original = p.regions(1000, 500, [48, 20, 50, 19]).0;
        let old = encode(&p).replace("version=2", "version=1");
        assert_eq!(decode(&old).unwrap(), p);
        p.parts.insert(10, Rect(2200, 3000, 2400, 4500));
        let changed = p.regions(1000, 500, [48, 20, 50, 19]).0;
        assert_eq!(
            changed[10],
            Region {
                x: 220,
                y: 150,
                width: 20,
                height: 75
            }
        );
        for i in 0..137 {
            if i != 10 {
                assert_eq!(changed[i], original[i]);
            }
        }
        assert_eq!(decode(&encode(&p)).unwrap(), p);
        assert!(decode(&format!("{}part_10=0,0,100,100\n", encode(&p))).is_err());
        // The moved top part still feeds the same physical strip position.
        let mut bytes = vec![0; 1000 * 500 * 4];
        for y in 150..225 {
            for x in 220..240 {
                bytes[(y * 1000 + x) * 4 + 2] = 255;
            }
        }
        let colours = crate::colour::average(&bytes, 1000, 500, 4000, &changed).unwrap();
        assert_eq!(colours[10], crate::colour::Rgb(255, 0, 0));
        let physical = crate::strip::installed(&colours).unwrap();
        assert_eq!(physical[54], [255, 0, 0]);
        assert_eq!(physical[53], [0, 0, 0]);
        assert_eq!(physical[55], [0, 0, 0]);
        p.parts.remove(&10);
        assert_eq!(p.regions(1000, 500, [48, 20, 50, 19]).0, original);
    }
}
