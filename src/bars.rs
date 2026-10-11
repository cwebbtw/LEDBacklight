//! Conservative, sparse BGRA black-bar detection, separate from capture and layout.
use crate::{capture::Frame, layout::Region};
use anyhow::{Result, ensure};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crop {
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
    pub left: usize,
}
impl Crop {
    pub fn bounds(self, w: usize, h: usize) -> Result<Region> {
        let horizontal = self.left.checked_add(self.right);
        let vertical = self.top.checked_add(self.bottom);
        ensure!(
            horizontal.is_some_and(|n| n < w) && vertical.is_some_and(|n| n < h),
            "crop removes the entire image"
        );
        Ok(Region {
            x: self.left,
            y: self.top,
            width: w - horizontal.unwrap(),
            height: h - vertical.unwrap(),
        })
    }
}

/// Probe 64 evenly spaced lines, excluding outer 5% to tolerate window chrome.
/// At least 90% must agree; dark picture content may extend a few runs inward.
pub fn detect(frame: &Frame, threshold: u8) -> Crop {
    fn pair(f: &Frame, threshold: u8, horizontal: bool) -> (usize, usize) {
        let (length, across) = if horizontal {
            (f.width, f.height)
        } else {
            (f.height, f.width)
        };
        let limit = length / 4;
        if limit < 2 || across < 2 {
            return (0, 0);
        }
        let black = |position: usize, line: usize| {
            let (x, y) = if horizontal {
                (position, line)
            } else {
                (line, position)
            };
            let offset = y * f.pitch + x * 4;
            f.data[offset..offset + 3].iter().all(|&v| v <= threshold)
        };
        // A window title strip may precede a genuine letterbox bar. Only bypass
        // up to 48 rows, and only to reach three nearly full-width black rows.
        let offset = if horizontal {
            0
        } else {
            let row_black = |row| {
                (0..64)
                    .filter(|&i| {
                        let line =
                            ((across - 1) * (640 + 90 * (2 * i + 1)) / 12800).min(across - 1);
                        black(row, line)
                    })
                    .count()
                    >= 61
            };
            (0..=48.min(length / 20))
                .find(|&row| row + 2 < length && (row..row + 3).all(row_black))
                .unwrap_or(0)
        };
        let mut starts = Vec::with_capacity(64);
        let mut ends = Vec::with_capacity(64);
        for i in 0..64 {
            let line = ((across - 1) * (5 * 128 + 90 * (2 * i + 1)) / (100 * 128)).min(across - 1);
            let start = (0..=limit).find(|&p| !black(p + offset, line));
            let end = (0..=limit).find(|&p| !black(length - 1 - p, line));
            starts.push(start.unwrap_or(limit + 1));
            ends.push(end.unwrap_or(limit + 1));
        }
        starts.sort_unstable();
        ends.sort_unstable();
        let (a, b) = (starts[32], ends[32]);
        let consistent = |values: &[usize], median: usize| {
            values.iter().filter(|&&n| n.abs_diff(median) <= 2).count() >= 58
                // Bright intrusions into a bar are not dark picture extensions.
                && values.iter().filter(|&&n| n.saturating_add(2) >= median).count() >= 61
        };
        if a < 2
            || b < 2
            || a > limit
            || b > limit
            || a.abs_diff(b) > 2.max(a.min(b) / 10)
            || !consistent(&starts, a)
            || !consistent(&ends, b)
        {
            return (0, 0);
        }
        // Require visibly nonblack content just inside both boundaries; dark scenes stay uncropped.
        let visible = |position: usize| {
            (0..64)
                .filter(|&i| {
                    let line = (across - 1) * (i * 2 + 1) / 128;
                    let (x, y) = if horizontal {
                        (position, line)
                    } else {
                        (line, position)
                    };
                    let offset = y * f.pitch + x * 4;
                    f.data[offset..offset + 3]
                        .iter()
                        .any(|&v| v > threshold.saturating_add(16))
                })
                .count()
                >= 32
        };
        if !visible(a + offset) || !visible(length - 1 - b) {
            return (0, 0);
        }
        (a + offset, b)
    }
    let (left, right) = pair(frame, threshold, true);
    let (top, bottom) = pair(frame, threshold, false);
    Crop {
        top,
        right,
        bottom,
        left,
    }
}

pub struct Tracker {
    delay: Duration,
    candidate: Crop,
    age: Duration,
    active: Crop,
}
impl Tracker {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            candidate: Crop::default(),
            age: Duration::ZERO,
            active: Crop::default(),
        }
    }
    pub fn update(&mut self, candidate: Crop, elapsed: Duration) -> Crop {
        if candidate != self.candidate {
            self.candidate = candidate;
            self.age = Duration::ZERO;
        } else {
            self.age = self.age.saturating_add(elapsed);
        }
        if self.age >= self.delay {
            self.active = self.candidate;
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(crop: Crop) -> Frame {
        let mut f = Frame {
            data: vec![255; 408 * 80],
            width: 100,
            height: 80,
            pitch: 408,
        };
        let r = crop.bounds(100, 80).unwrap();
        for y in 0..80 {
            for x in 0..100 {
                let v = if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
                    100
                } else {
                    0
                };
                f.data[y * 408 + x * 4..y * 408 + x * 4 + 3].fill(v);
            }
        }
        f
    }
    #[test]
    fn symmetric_bars_with_padding() {
        for crop in [
            Crop {
                left: 12,
                right: 12,
                ..Crop::default()
            },
            Crop {
                top: 8,
                bottom: 8,
                ..Crop::default()
            },
        ] {
            assert_eq!(detect(&image(crop), 8), crop);
        }
    }
    #[test]
    fn rejects_uneven_asymmetric_and_dark() {
        assert_eq!(
            detect(
                &image(Crop {
                    left: 8,
                    right: 15,
                    ..Crop::default()
                }),
                8
            ),
            Crop::default()
        );
        let mut f = image(Crop {
            left: 12,
            right: 12,
            ..Crop::default()
        });
        for y in 20..40 {
            f.data[y * 408 + 5 * 4..y * 408 + 12 * 4].fill(100);
        }
        assert_eq!(detect(&f, 8), Crop::default());
        f.data.fill(0);
        assert_eq!(detect(&f, 8), Crop::default());
        assert_eq!(detect(&image(Crop::default()), 8), Crop::default());
    }
    #[test]
    fn dark_artwork_can_extend_past_a_uniform_bar() {
        let c = Crop {
            left: 12,
            right: 12,
            ..Crop::default()
        };
        let mut f = image(c);
        // Four of the 64 probes encounter dark artwork beyond the real boundary.
        for y in 4..9 {
            f.data[y * 408 + 12 * 4..y * 408 + 20 * 4].fill(0);
        }
        assert_eq!(detect(&f, 8), c);
    }
    #[test]
    #[ignore = "requires the supplied screenshot exported as BGRA via BACKLIGHT_SCREENSHOT_BGRA"]
    fn supplied_wow_screenshot() {
        let data = std::fs::read(
            std::env::var("BACKLIGHT_SCREENSHOT_BGRA").expect("BGRA screenshot path"),
        )
        .unwrap();
        let f = Frame {
            data,
            width: 3436,
            height: 1390,
            pitch: 13744,
        };
        let crop = detect(&f, 8);
        assert_eq!(
            crop,
            Crop {
                left: 450,
                right: 452,
                ..Crop::default()
            }
        );
        let bounds = crop.bounds(f.width, f.height).unwrap();
        let mut regions =
            crate::layout::regions(bounds.width, bounds.height, [32, 18, 32, 18], 120).unwrap();
        for r in &mut regions {
            r.x += bounds.x;
            r.y += bounds.y;
        }
        let colours =
            crate::colour::average(&f.data, f.width, f.height, f.pitch, &regions).unwrap();
        assert!(colours[32..50].iter().any(|c| c.0 > 30));
        assert!(colours[82..100].iter().any(|c| c.0 > 30));
    }
    #[test]
    fn letterbox_after_title_strip() {
        let mut f = image(Crop {
            top: 11,
            bottom: 8,
            ..Crop::default()
        });
        for y in 0..3 {
            f.data[y * 408..y * 408 + 400].fill(35);
        }
        assert_eq!(
            detect(&f, 8),
            Crop {
                top: 11,
                bottom: 8,
                ..Crop::default()
            }
        );
        // A title strip alone must not create a crop.
        let mut f = image(Crop::default());
        for y in 0..3 {
            f.data[y * 408..y * 408 + 400].fill(35);
        }
        assert_eq!(detect(&f, 8), Crop::default());
    }
    #[test]
    #[ignore = "requires the supplied letterbox screenshot as BACKLIGHT_TOP_BGRA"]
    fn supplied_letterbox_screenshot() {
        let data =
            std::fs::read(std::env::var("BACKLIGHT_TOP_BGRA").expect("BGRA screenshot path"))
                .unwrap();
        let f = Frame {
            data,
            width: 3439,
            height: 1387,
            pitch: 13756,
        };
        assert_eq!(
            detect(&f, 8),
            Crop {
                top: 94,
                bottom: 66,
                ..Crop::default()
            }
        );
    }
    #[test]
    fn stability_and_removal() {
        let mut t = Tracker::new(Duration::from_millis(500));
        let c = Crop {
            left: 12,
            right: 12,
            ..Crop::default()
        };
        assert_eq!(t.update(c, Duration::from_secs(1)), Crop::default());
        assert_eq!(t.update(c, Duration::from_millis(499)), Crop::default());
        assert_eq!(t.update(c, Duration::from_millis(1)), c);
        assert_eq!(t.update(Crop::default(), Duration::ZERO), c);
        assert_eq!(
            t.update(Crop::default(), Duration::from_millis(500)),
            Crop::default()
        );
    }
    #[test]
    fn manual_bounds() {
        assert!(
            Crop {
                left: usize::MAX,
                right: 1,
                ..Crop::default()
            }
            .bounds(100, 80)
            .is_err()
        );
        assert!(
            Crop {
                top: 80,
                ..Crop::default()
            }
            .bounds(100, 80)
            .is_err()
        );
    }
}
