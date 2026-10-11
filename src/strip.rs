//! Installed strip order, as viewed from the front. Public labels are one-based.
use crate::colour::Rgb;
use anyhow::{Result, ensure};

pub const COUNTS: [usize; 4] = [48, 20, 50, 19];
pub const LEDS: usize = 140;
pub fn installed(colours: &[Rgb]) -> Result<Vec<[u8; 3]>> {
    ensure!(
        colours.len() == 137,
        "installed layout requires 48 top,20 right,50 bottom,19 left regions"
    );
    let rgb = |i: usize| {
        let c = colours[i];
        [c.0, c.1, c.2]
    };
    let corner = |a: usize, b: usize| {
        let a = rgb(a);
        let b = rgb(b);
        std::array::from_fn(|i| ((u16::from(a[i]) + u16::from(b[i])).div_ceil(2)) as u8)
    };
    let mut out = Vec::with_capacity(LEDS);
    // Bottom runs clockwise right-to-left; physical strip starts partway across.
    out.extend((95..118).map(rgb)); // 1..23: bottom split to bottom-left
    out.push(corner(117, 118)); // 24: bottom-left
    out.extend((118..137).map(rgb)); // 25..43: left, bottom-to-top
    out.push(corner(136, 0)); // 44: top-left
    out.extend((0..48).map(rgb)); // 45..92: top, left-to-right
    out.push(corner(47, 48)); // 93: top-right
    out.extend((48..68).map(rgb)); // 94..113: right, top-to-bottom
    out.extend((68..95).map(rgb)); // 114..140: bottom-right back to split
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_edge_sample_once_and_split_in_order() {
        let c: Vec<_> = (0..137).map(|i| Rgb(i, 0, 0)).collect();
        let out = installed(&c).unwrap();
        assert_eq!(out.len(), 140);
        for (led, region) in [
            (1, 95),
            (23, 117),
            (25, 118),
            (43, 136),
            (45, 0),
            (92, 47),
            (94, 48),
            (113, 67),
            (114, 68),
            (140, 94),
        ] {
            assert_eq!(out[led - 1], [region, 0, 0]);
        }
        let mut edge_values: Vec<_> = out
            .iter()
            .enumerate()
            .filter(|(i, _)| ![23, 43, 92].contains(i))
            .map(|(_, v)| v[0])
            .collect();
        edge_values.sort_unstable();
        assert_eq!(edge_values, (0..137).collect::<Vec<u8>>());
        assert!(installed(&c[..136]).is_err());
    }
    #[test]
    fn corner_channels_average_without_overflow() {
        let mut c = vec![Rgb(255, 0, 100); 137];
        c[118] = Rgb(1, 200, 0);
        assert_eq!(installed(&c).unwrap()[23], [128, 100, 50]);
    }
}
