use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Move only bottom regions inward, relative to the active picture's bottom.
/// Preserve horizontal placement and clockwise ordering; depth clamps safely.
pub fn sampling_regions(
    w: usize,
    h: usize,
    counts: [usize; 4],
    depth: usize,
    bottom_inset: usize,
) -> Result<Vec<Region>> {
    ensure!(
        bottom_inset < h,
        "bottom inset must leave at least one picture row"
    );
    let mut result = regions(w, h, counts, depth)?;
    let bottom_depth = depth.min(h - bottom_inset);
    for r in &mut result[counts[0] + counts[1]..counts[0] + counts[1] + counts[2]] {
        r.y = h - bottom_inset - bottom_depth;
        r.height = bottom_depth;
    }
    Ok(result)
}

/// Half-open rectangles; corners intentionally contribute to both adjacent edges.
pub fn regions(w: usize, h: usize, counts: [usize; 4], depth: usize) -> Result<Vec<Region>> {
    ensure!(
        w > 0 && h > 0 && depth > 0,
        "dimensions and depth must be positive"
    );
    ensure!(
        counts.iter().all(|&n| n > 0),
        "edge counts must be positive"
    );
    ensure!(
        counts[0] <= w && counts[2] <= w && counts[1] <= h && counts[3] <= h,
        "more regions than edge pixels"
    );
    let mut result = Vec::new();
    for (edge, &count) in counts.iter().enumerate() {
        let length = if edge % 2 == 0 { w } else { h };
        for i in 0..count {
            let index = if edge >= 2 { count - 1 - i } else { i };
            let start = index * length / count;
            let end = (index + 1) * length / count;
            result.push(match edge {
                0 => Region {
                    x: start,
                    y: 0,
                    width: end - start,
                    height: depth.min(h),
                },
                1 => Region {
                    x: w - depth.min(w),
                    y: start,
                    width: depth.min(w),
                    height: end - start,
                },
                2 => Region {
                    x: start,
                    y: h - depth.min(h),
                    width: end - start,
                    height: depth.min(h),
                },
                _ => Region {
                    x: 0,
                    y: start,
                    width: depth.min(w),
                    height: end - start,
                },
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_inset_preserves_other_edges_and_order() {
        let c = [3, 2, 3, 2];
        let original = regions(11, 10, c, 2).unwrap();
        let inset = sampling_regions(11, 10, c, 2, 3).unwrap();
        assert_eq!(original[..5], inset[..5]);
        assert_eq!(original[8..], inset[8..]);
        assert_eq!(
            inset[5..8]
                .iter()
                .map(|r| (r.x, r.y, r.height))
                .collect::<Vec<_>>(),
            vec![(7, 5, 2), (3, 5, 2), (0, 5, 2)]
        );
        assert_eq!(sampling_regions(11, 10, c, 2, 0).unwrap(), original);
        assert_eq!(sampling_regions(11, 10, c, 100, 9).unwrap()[5].height, 1);
        assert!(sampling_regions(11, 10, c, 2, 10).is_err());
    }
    #[test]
    fn sampling_above_action_bar_uses_world_colours() {
        let mut data = vec![0; 8 * 10 * 4];
        for y in 0..7 {
            for x in 0..8 {
                let i = (y * 8 + x) * 4;
                data[i..i + 4].copy_from_slice(&[20, 180, 100, 255]);
            }
        }
        let c = [2; 4];
        let old = crate::colour::average(&data, 8, 10, 32, &regions(8, 10, c, 2).unwrap()).unwrap();
        let new =
            crate::colour::average(&data, 8, 10, 32, &sampling_regions(8, 10, c, 2, 3).unwrap())
                .unwrap();
        assert_eq!(&old[4..6], &[crate::colour::Rgb(0, 0, 0); 2]);
        assert_eq!(&new[4..6], &[crate::colour::Rgb(100, 180, 20); 2]);
    }
    #[test]
    fn clockwise_and_remainders() {
        let r = regions(11, 7, [3, 2, 3, 2], 2).unwrap();
        assert_eq!(
            r.iter().map(|r| (r.x, r.y)).collect::<Vec<_>>(),
            vec![
                (0, 0),
                (3, 0),
                (7, 0),
                (9, 0),
                (9, 3),
                (7, 5),
                (3, 5),
                (0, 5),
                (0, 3),
                (0, 0)
            ]
        );
        assert_eq!(r[..3].iter().map(|r| r.width).sum::<usize>(), 11);
        assert_eq!(r[3..5].iter().map(|r| r.height).sum::<usize>(), 7);
    }
    #[test]
    fn bounds_and_validation() {
        for r in regions(5, 3, [5, 3, 5, 3], 100).unwrap() {
            assert!(r.width > 0 && r.height > 0 && r.x + r.width <= 5 && r.y + r.height <= 3);
        }
        assert!(regions(5, 3, [6, 1, 1, 1], 1).is_err());
        assert!(regions(5, 3, [0, 1, 1, 1], 1).is_err());
        assert!(regions(5, 3, [1; 4], 0).is_err());
    }
}
