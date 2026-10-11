//! Explicit WoW UI avoidance profile. It cannot recover pixels hidden by the UI.
use crate::layout::Region;

/// Normalised rectangles based on the supplied UI layout; active-picture coordinates.
pub fn wow_zones(w: usize, h: usize) -> Vec<Region> {
    // Minimap, objectives, chat, action bars, damage panel, player unit frame.
    [
        (85, 0, 100, 35),
        (85, 35, 100, 80),
        (0, 58, 24, 88),
        (14, 86, 86, 100),
        (0, 20, 20, 36),
        (22, 58, 36, 72),
        (70, 0, 80, 15),    // Buff icons, including the row below the top edge.
        (95, 30, 100, 100), // Right-side vertical action bars.
    ]
    .into_iter()
    .map(|(l, t, r, b)| {
        let x = w * l / 100;
        let y = h * t / 100;
        Region {
            x,
            y,
            width: (w * r / 100).saturating_sub(x).max(1),
            height: (h * b / 100).saturating_sub(y).max(1),
        }
    })
    .collect()
}
fn overlaps(a: Region, b: Region) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Bottom readings stay aligned horizontally with their LEDs. Search only
/// upwards, including above stacked exclusions, and leave clean boxes alone.
/// If no full-size box fits, the caller can reduce depth or mark it blocked.
pub fn avoid_bottom(regions: &mut [Region], zones: &[Region]) -> usize {
    let mut shifted = 0;
    for region in regions {
        if !zones.iter().any(|&z| overlaps(*region, z)) {
            continue;
        }
        let best = zones
            .iter()
            .filter_map(|z| {
                let y = z.y.checked_sub(region.height)?;
                let candidate = Region { y, ..*region };
                (y <= region.y && !zones.iter().any(|&z| overlaps(candidate, z)))
                    .then_some(candidate)
            })
            .max_by_key(|r| r.y);
        if let Some(candidate) = best {
            *region = candidate;
            shifted += 1;
        }
    }
    shifted
}

pub fn avoid_edges(
    regions: &mut [Region],
    counts: [usize; 4],
    zones: &[Region],
    w: usize,
    h: usize,
) -> usize {
    let start = counts[0] + counts[1];
    let (before, rest) = regions.split_at_mut(start);
    let (bottom, after) = rest.split_at_mut(counts[2]);
    avoid(before, zones, w, h) + avoid_bottom(bottom, zones) + avoid(after, zones, w, h)
}

/// Shift a contaminated region to the nearest clear rectangle of the same size.
/// Leave clean regions untouched. No filtering by brightness: dark scenery stays dark.
pub fn avoid(regions: &mut [Region], zones: &[Region], w: usize, h: usize) -> usize {
    let mut shifted = 0;
    for region in regions {
        if !zones.iter().any(|&z| overlaps(*region, z)) {
            continue;
        }
        let original = *region;
        let mut xs = vec![original.x];
        let mut ys = vec![original.y];
        for z in zones {
            xs.push(z.x.saturating_sub(original.width));
            xs.push((z.x + z.width).min(w - original.width));
            ys.push(z.y.saturating_sub(original.height));
            ys.push((z.y + z.height).min(h - original.height));
        }
        let mut best = None;
        for &x in &xs {
            for &y in &ys {
                let candidate = Region { x, y, ..original };
                if zones.iter().any(|&z| overlaps(candidate, z)) {
                    continue;
                }
                let distance =
                    (x.abs_diff(original.x) as u64).pow(2) + (y.abs_diff(original.y) as u64).pow(2);
                if best.is_none_or(|(old, _)| distance < old) {
                    best = Some((distance, candidate));
                }
            }
        }
        // Very large sampling rectangles may have nowhere to fit; preserve valid geometry.
        if let Some((_, candidate)) = best {
            *region = candidate;
            shifted += 1;
        }
    }
    shifted
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_boxes_lift_without_sideways_gaps_even_at_exclusion_ends() {
        let mut readings = crate::layout::regions(1000, 500, [10, 2, 10, 2], 20).unwrap();
        let original = readings.clone();
        let zones = [Region {
            x: 195,
            y: 400,
            width: 610,
            height: 100,
        }];
        avoid_edges(&mut readings, [10, 2, 10, 2], &zones, 1000, 500);
        for (old, new) in original[12..22].iter().zip(&readings[12..22]) {
            assert_eq!(
                (new.x, new.width, new.height),
                (old.x, old.width, old.height)
            );
            assert_eq!(new.y, if overlaps(*old, zones[0]) { 380 } else { old.y });
            assert!(!overlaps(*new, zones[0]));
        }
    }
    #[test]
    fn bottom_lifts_above_stacked_exclusions() {
        let mut r = [Region {
            x: 100,
            y: 480,
            width: 100,
            height: 20,
        }];
        let zones = [
            Region {
                x: 90,
                y: 400,
                width: 120,
                height: 100,
            },
            Region {
                x: 95,
                y: 370,
                width: 110,
                height: 30,
            },
        ];
        assert_eq!(avoid_bottom(&mut r, &zones), 1);
        assert_eq!(r[0].y, 350);
        assert_eq!(r[0].x, 100);
    }
    #[test]
    fn affected_samples_move_and_clean_samples_stay() {
        let zones = wow_zones(500, 300);
        let mut regions =
            crate::layout::sampling_regions(500, 300, [10, 6, 10, 6], 20, 50).unwrap();
        let original = regions.clone();
        assert!(avoid(&mut regions, &zones, 500, 300) > 0);
        for (old, r) in original.iter().zip(&regions) {
            assert_eq!((old.width, old.height), (r.width, r.height));
            assert!(r.x + r.width <= 500 && r.y + r.height <= 300);
            assert!(!zones.iter().any(|&z| overlaps(*r, z)));
            if !zones.iter().any(|&z| overlaps(*old, z)) {
                assert_eq!(old, r);
            }
        }
    }
    #[test]
    fn synthetic_ui_does_not_contribute_to_world_samples() {
        let (w, h) = (500, 300);
        let zones = wow_zones(w, h);
        let mut bytes = vec![0; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let p = Region {
                    x,
                    y,
                    width: 1,
                    height: 1,
                };
                let c = if zones.iter().any(|&z| overlaps(p, z)) {
                    [20, 20, 20, 255]
                } else {
                    [20, 180, 100, 255]
                };
                bytes[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        let mut regions = crate::layout::sampling_regions(w, h, [10, 6, 10, 6], 20, 50).unwrap();
        avoid(&mut regions, &zones, w, h);
        assert!(
            crate::colour::average(&bytes, w, h, w * 4, &regions)
                .unwrap()
                .iter()
                .all(|&c| c == crate::colour::Rgb(100, 180, 20))
        );
    }
    #[test]
    fn no_space_preserves_region() {
        let r = Region {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let mut regions = [r];
        assert_eq!(avoid(&mut regions, &[r], 10, 10), 0);
        assert_eq!(regions, [r]);
    }
}
