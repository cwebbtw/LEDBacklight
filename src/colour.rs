use crate::layout::Region;
use anyhow::{Result, ensure};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Convert encoded screen colours to LED PWM, then apply the brightness ceiling.
/// Gamma 1 preserves the original linear byte scaling; >1 dims midtones.
pub fn led_output(input: &[Rgb], gamma: f64, brightness: f64) -> Vec<Rgb> {
    input
        .iter()
        .map(|c| {
            let channel = |v: u8| {
                ((f64::from(v) / 255.0).powf(gamma) * 255.0 * brightness)
                    .clamp(0.0, 255.0)
                    .round() as u8
            };
            Rgb(channel(c.0), channel(c.1), channel(c.2))
        })
        .collect()
}

/// Display encoding for the preview only. LED PWM values must not be gamma-
/// darkened a second time by a monitor. This is an approximate visualisation.
pub fn preview_output(drive: &[Rgb], gamma: f64) -> Vec<Rgb> {
    led_output(drive, 1.0 / gamma, 1.0)
}

// #FFE0A0 in the user's WLED colour picker is gamma-encoded. Their controller
// uses colour gamma 2.8 for effects and bypasses it for DDP (no-gc=true).
// Rounded PWM reference: 255 * (channel / 255)^2.8 = [255, 177, 69].
const WHITE_PWM_REFERENCE: Rgb = Rgb(255, 177, 69);

/// Match a narrow corridor from neutral grey to the captured grey-scene tint.
/// Normalising by peak makes this independent of scene brightness. The axis
/// is RGB 106,156,172 from the user's ghost scene, not a general cyan/pale mask.
fn white_correction_weight(source: Rgb) -> f64 {
    let max = f64::from(source.0.max(source.1).max(source.2));
    if max == 0.0 {
        return 0.0;
    }
    let colour = [source.0, source.1, source.2].map(|v| f64::from(v) / max);
    let axis = [106.0 / 172.0 - 1.0, 156.0 / 172.0 - 1.0, 0.0];
    let offset = colour.map(|v| v - 1.0);
    let dot = offset.iter().zip(axis).map(|(a, b)| a * b).sum::<f64>();
    let length_squared = axis.iter().map(|v| v * v).sum::<f64>();
    // Allow modest variation in tint strength, but reject different hue directions.
    let position = (dot / length_squared).clamp(0.0, 1.25);
    let distance = offset
        .iter()
        .zip(axis)
        .map(|(v, a)| (v - position * a).powi(2))
        .sum::<f64>()
        .sqrt();
    let fade = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let hue_match = fade((0.085 - distance) / 0.05);
    let saturation = 1.0 - colour.into_iter().fold(1.0, f64::min);
    hue_match * fade((0.8 - saturation) / 0.3)
}

/// Map neutral grey and colours matching the calibrated tint to #FFE0A0.
/// Apply to final PWM (after brightness clipping), so high brightness does not
/// clip all three calibrated channels back to equal white. Match before gamma
/// and clipping so brightness boosts cannot bleach unrelated pastel colours.
/// This is an intentional neutralising effect, not a measured colour profile.
pub fn corrected_led_output(input: &[Rgb], gamma: f64, brightness: f64, enabled: bool) -> Vec<Rgb> {
    input
        .iter()
        .map(|&source| corrected_led_colour(source, gamma, brightness, enabled))
        .collect()
}

/// Single-region output for independent brightness without temporary vectors.
pub fn corrected_led_colour(source: Rgb, gamma: f64, brightness: f64, enabled: bool) -> Rgb {
    let pwm = |v: u8| {
        ((f64::from(v) / 255.0).powf(gamma) * 255.0 * brightness)
            .clamp(0.0, 255.0)
            .round() as u8
    };
    let mut output = Rgb(pwm(source.0), pwm(source.1), pwm(source.2));
    if enabled {
        let weight = white_correction_weight(source);
        let peak = f64::from(output.0.max(output.1).max(output.2));
        let channel = |value: u8, reference: u8| {
            let target = peak * f64::from(reference) / 255.0;
            (f64::from(value) + weight * (target - f64::from(value))).round() as u8
        };
        output.0 = channel(output.0, WHITE_PWM_REFERENCE.0);
        output.1 = channel(output.1, WHITE_PWM_REFERENCE.1);
        output.2 = channel(output.2, WHITE_PWM_REFERENCE.2);
    }
    output
}

/// Circular neighbour blend follows the clockwise perimeter, including corners.
pub fn ambient(input: &[Rgb], blend: f64, brightness: f64, saturation: f64) -> Vec<Rgb> {
    input
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let previous = input[(i + input.len() - 1) % input.len()];
            let next = input[(i + 1) % input.len()];
            let channels = |c: Rgb| [f64::from(c.0), f64::from(c.1), f64::from(c.2)];
            let mut values = channels(*c);
            for ((v, p), n) in values
                .iter_mut()
                .zip(channels(previous))
                .zip(channels(next))
            {
                *v = *v * (1.0 - blend) + (p + n) * 0.5 * blend;
            }
            let grey = values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
            let v = values.map(|v| {
                ((grey + (v - grey) * saturation) * brightness)
                    .clamp(0.0, 255.0)
                    .round() as u8
            });
            Rgb(v[0], v[1], v[2])
        })
        .collect()
}

/// DXGI BGRA8, with an explicit byte pitch. Alpha is ignored.
pub fn average(
    data: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    regions: &[Region],
) -> Result<Vec<Rgb>> {
    Ok(sample_impl(data, width, height, pitch, regions, false)?
        .into_iter()
        .map(|s| s.average)
        .collect())
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub average: Rgb,
    pub highlight: Rgb,
}

/// Average plus brightest 5% mean from spatially supported bright patches.
/// Reject stroke-like support: each 16x16 tile needs broad coverage and a solid
/// 8x8 interior. Average colour is unaffected.
pub fn sample(
    data: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    regions: &[Region],
) -> Result<Vec<Sample>> {
    sample_impl(data, width, height, pitch, regions, true)
}

fn sample_impl(
    data: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    regions: &[Region],
    highlights: bool,
) -> Result<Vec<Sample>> {
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| anyhow::anyhow!("width overflow"))?;
    ensure!(pitch >= row_bytes, "row pitch too small");
    let size = pitch
        .checked_mul(height)
        .ok_or_else(|| anyhow::anyhow!("frame overflow"))?;
    ensure!(data.len() >= size, "truncated frame");
    regions
        .iter()
        .map(|r| {
            ensure!(
                r.width > 0
                    && r.height > 0
                    && r.x.checked_add(r.width).is_some_and(|x| x <= width)
                    && r.y.checked_add(r.height).is_some_and(|y| y <= height),
                "invalid region"
            );
            let mut sums = [0u64; 3];
            // count, R sum, G sum, B sum; no sorting or per-pixel allocation.
            let mut histogram = [[0u64; 4]; 64];
            if highlights {
                // Visit pixels once, tile by tile. A light needs broad bright
                // support; yellow text's colour alone must not qualify it.
                for ty in (r.y..r.y + r.height).step_by(16) {
                    for tx in (r.x..r.x + r.width).step_by(16) {
                        let end_x = (tx + 16).min(r.x + r.width);
                        let end_y = (ty + 16).min(r.y + r.height);
                        let area = ((end_x - tx) * (end_y - ty)) as u64;
                        let mut tile = [[0u64; 4]; 64];
                        let mut bright = 0;
                        let mut mask = [0u16; 16];
                        for y in ty..end_y {
                            let row = &data[y * pitch + tx * 4..y * pitch + end_x * 4];
                            for (x, p) in row.as_chunks::<4>().0.iter().enumerate() {
                                sums[0] += u64::from(p[2]);
                                sums[1] += u64::from(p[1]);
                                sums[2] += u64::from(p[0]);
                                let luma = (54 * u32::from(p[2])
                                    + 183 * u32::from(p[1])
                                    + 19 * u32::from(p[0]))
                                    >> 8;
                                if luma < 96 {
                                    continue;
                                }
                                bright += 1;
                                mask[y - ty] |= 1 << x;
                                let bin = &mut tile[(luma / 4) as usize];
                                bin[0] += 1;
                                bin[1] += u64::from(p[2]);
                                bin[2] += u64::from(p[1]);
                                bin[3] += u64::from(p[0]);
                            }
                        }
                        let interior =
                            if bright * 5 >= area * 3 && end_x - tx >= 8 && end_y - ty >= 8 {
                                (0..=end_y - ty - 8).any(|y| {
                                    (0..=end_x - tx - 8).any(|x| {
                                        let square = 0xffu16 << x;
                                        mask[y..y + 8].iter().all(|row| row & square == square)
                                    })
                                })
                            } else {
                                false
                            };
                        if interior {
                            for (total, bin) in histogram.iter_mut().zip(tile) {
                                for (sum, value) in total.iter_mut().zip(bin) {
                                    *sum += value;
                                }
                            }
                        }
                    }
                }
            } else {
                for y in r.y..r.y + r.height {
                    let row = &data[y * pitch + r.x * 4..y * pitch + (r.x + r.width) * 4];
                    for p in row.as_chunks::<4>().0 {
                        sums[0] += u64::from(p[2]);
                        sums[1] += u64::from(p[1]);
                        sums[2] += u64::from(p[0]);
                    }
                }
            }
            let n = (r.width * r.height) as u64;
            let average = Rgb(
                ((sums[0] + n / 2) / n) as u8,
                ((sums[1] + n / 2) / n) as u8,
                ((sums[2] + n / 2) / n) as u8,
            );
            let supported = histogram.iter().map(|bin| bin[0]).sum::<u64>();
            let highlight = if supported >= 16 {
                let count = n.div_ceil(20).max(16).min(supported);
                let mut remaining = count;
                let mut bright_sums = [0.0; 3];
                for bin in histogram.iter().rev() {
                    let take = remaining.min(bin[0]);
                    if take > 0 {
                        for (sum, channel) in bright_sums.iter_mut().zip(&bin[1..]) {
                            *sum += *channel as f64 * take as f64 / bin[0] as f64;
                        }
                        remaining -= take;
                    }
                    if remaining == 0 {
                        break;
                    }
                }
                let values = bright_sums.map(|v| (v / count as f64).round() as u8);
                Rgb(values[0], values[1], values[2])
            } else if highlights {
                // Zero means no qualifying source. Returning the region average
                // here would still trigger the above-scene boost for large labels.
                Rgb::default()
            } else {
                average
            };
            Ok(Sample { average, highlight })
        })
        .collect()
}

pub struct Smoother {
    tau: f64,
    state: Vec<[f64; 3]>,
}
impl Smoother {
    pub fn new(tau: Duration) -> Self {
        Self {
            tau: tau.as_secs_f64(),
            state: Vec::new(),
        }
    }
    pub fn set_duration(&mut self, tau: Duration) {
        self.tau = tau.as_secs_f64();
    }
    pub fn update(&mut self, input: &[Rgb], elapsed: Duration) -> Vec<Rgb> {
        if self.state.len() != input.len() {
            self.state = input
                .iter()
                .map(|c| [f64::from(c.0), f64::from(c.1), f64::from(c.2)])
                .collect();
        }
        let alpha = if self.tau == 0.0 {
            1.0
        } else {
            -(-elapsed.as_secs_f64() / self.tau).exp_m1()
        };
        self.state
            .iter_mut()
            .zip(input)
            .map(|(s, c)| {
                for (value, target) in s.iter_mut().zip([c.0, c.1, c.2]) {
                    *value += alpha * (f64::from(target) - *value);
                }
                Rgb(s[0].round() as u8, s[1].round() as u8, s[2].round() as u8)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_drive_smoothing_bounds_local_boost_and_calibration_transitions() {
        let averages = vec![Rgb(10, 10, 10); 20];
        let absent = vec![Rgb::default(); 20];
        let mut lamp = absent.clone();
        lamp[0] = Rgb(255, 255, 255);
        let drive = |highlights: &[Rgb], white: bool| {
            crate::adaptive::local_highlights(&averages, highlights, 0.15, true)
                .into_iter()
                .map(|(source, scale)| corrected_led_colour(source, 2.2, scale, white))
                .collect::<Vec<_>>()
        };
        let dark = drive(&absent, true);
        let lit = drive(&lamp, true);
        assert_eq!(lit[0], Rgb(255, 177, 69));
        let mut final_smoother = Smoother::new(Duration::from_millis(150));
        let mut previous = final_smoother.update(&dark, Duration::ZERO);
        // Light appearance, correction toggle, and light disappearance all
        // travel through the same final-output smoother used by the app.
        for goal in [&lit, &drive(&lamp, false), &dark] {
            for _ in 0..30 {
                let next = final_smoother.update(goal, Duration::from_millis(10));
                for (a, b) in previous.iter().zip(&next) {
                    assert!(
                        a.0.abs_diff(b.0) <= 17
                            && a.1.abs_diff(b.1) <= 17
                            && a.2.abs_diff(b.2) <= 17
                    );
                }
                previous = next;
            }
        }
        final_smoother.set_duration(Duration::ZERO);
        assert_eq!(final_smoother.update(&lit, Duration::ZERO), lit);
    }
    #[test]
    fn large_bold_labels_cannot_reenter_through_region_average() {
        for stroke in [2, 4, 6] {
            let mut bytes = vec![0; 128 * 32 * 4];
            for y in 0..32 {
                for x in 0..128 {
                    let glyph = x < 96
                        && (x % 24 < stroke
                            || x % 24 < 20
                                && (y < stroke
                                    || (14..14 + stroke).contains(&y)
                                    || y >= 32 - stroke));
                    let lamp = x >= 96;
                    let pixel = if glyph || lamp {
                        [0, 255, 255, 255]
                    } else {
                        [10, 10, 10, 255]
                    };
                    bytes[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4].copy_from_slice(&pixel);
                }
            }
            let regions = [
                Region {
                    x: 0,
                    y: 0,
                    width: 96,
                    height: 32,
                },
                Region {
                    x: 96,
                    y: 0,
                    width: 32,
                    height: 32,
                },
            ];
            let samples = sample(&bytes, 128, 32, 512, &regions).unwrap();
            assert_eq!(samples[0].highlight, Rgb::default(), "stroke {stroke}");
            assert_eq!(samples[1].highlight, Rgb(255, 255, 0));
            let mut averages = vec![Rgb(10, 10, 10); 20];
            let mut highlights = vec![Rgb::default(); 20];
            for i in 0..2 {
                averages[i] = samples[i].average;
                highlights[i] = samples[i].highlight;
            }
            let output = crate::adaptive::local_highlights(&averages, &highlights, 0.15, true);
            assert_eq!(
                output[0],
                (averages[0], 0.15),
                "rejected label still boosted at stroke {stroke}"
            );
            assert!(output[1].1 > 1.5);
            if stroke == 6 {
                let mut old_fallback = highlights.clone();
                old_fallback[0] = averages[0];
                assert!(
                    crate::adaptive::local_highlights(&averages, &old_fallback, 0.15, true)[0].1
                        > 0.15,
                    "fixture must reproduce the old average-fallback boost"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires supplied 567x766 screenshot as BACKLIGHT_LAMP_LABEL_BGRA"]
    fn supplied_lantern_and_name_label_screenshot() {
        let bytes = std::fs::read(std::env::var("BACKLIGHT_LAMP_LABEL_BGRA").unwrap()).unwrap();
        let regions = [
            Region {
                x: 380,
                y: 24,
                width: 128,
                height: 176,
            },
            Region {
                x: 104,
                y: 448,
                width: 200,
                height: 48,
            },
        ];
        let samples = sample(&bytes, 567, 766, 567 * 4, &regions).unwrap();
        eprintln!("Lantern {:?}, label {:?}", samples[0], samples[1]);
        assert_eq!(samples[1].highlight, Rgb::default());
        assert!(samples[0].highlight.0 > 150 && samples[0].highlight.1 > 100);
        let averages: Vec<_> = samples
            .iter()
            .map(|s| s.average)
            .chain(std::iter::repeat_n(Rgb(10, 10, 10), 18))
            .collect();
        let highlights: Vec<_> = samples
            .iter()
            .map(|s| s.highlight)
            .chain(std::iter::repeat_n(Rgb(10, 10, 10), 18))
            .collect();
        let output = crate::adaptive::local_highlights(&averages, &highlights, 0.15, true);
        assert_eq!(output[1], (averages[1], 0.15));
        assert!(output[0].1 > 1.5);
    }
    #[test]
    fn thin_yellow_labels_do_not_boost_but_same_colour_lantern_does() {
        let pitch = 528;
        let mut bytes = vec![255; pitch * 32];
        for y in 0..32 {
            for x in 0..128 {
                // Repeated two-pixel glyph strokes, with the same RGB as the lamp.
                let label = x < 96
                    && (12..24).contains(&y)
                    && (x % 8 < 2 || ((12..14).contains(&y) || (17..19).contains(&y)) && x % 8 < 6);
                let lamp = (96..128).contains(&x);
                let pixel = if label || lamp {
                    [40, 200, 255, 0]
                } else {
                    [10, 10, 10, 255]
                };
                bytes[y * pitch + x * 4..y * pitch + x * 4 + 4].copy_from_slice(&pixel);
            }
        }
        let regions = [
            Region {
                x: 0,
                y: 0,
                width: 96,
                height: 32,
            },
            Region {
                x: 96,
                y: 0,
                width: 32,
                height: 32,
            },
        ];
        let samples = sample(&bytes, 128, 32, pitch, &regions).unwrap();
        assert_eq!(samples[0].highlight, Rgb::default());
        assert_eq!(samples[1].highlight, Rgb(255, 200, 40));
        let averages: Vec<_> = samples.iter().map(|s| s.average).collect();
        let highlights: Vec<_> = samples.iter().map(|s| s.highlight).collect();
        let scene_averages: Vec<_> = averages
            .iter()
            .copied()
            .chain(std::iter::repeat_n(Rgb(10, 10, 10), 18))
            .collect();
        let scene_highlights: Vec<_> = highlights
            .iter()
            .copied()
            .chain(std::iter::repeat_n(Rgb::default(), 18))
            .collect();
        let output =
            crate::adaptive::local_highlights(&scene_averages, &scene_highlights, 0.15, true);
        assert_eq!(output[0], (averages[0], 0.15));
        assert!(output[1].1 > 1.0);
        // Ordinary colour averages still include text and match the manual path.
        assert_eq!(averages, average(&bytes, 128, 32, pitch, &regions).unwrap());
    }
    #[test]
    fn highlight_sampling_keeps_lamp_colour_local_and_ignores_padding() {
        let pitch = 416;
        let mut bytes = vec![255; pitch * 32];
        for y in 0..32 {
            for x in 0..100 {
                let pixel = if y < 16 && x < 16 {
                    [40, 190, 255, 0]
                } else {
                    [10, 10, 10, 255]
                };
                bytes[y * pitch + x * 4..y * pitch + x * 4 + 4].copy_from_slice(&pixel);
            }
        }
        let regions = [
            Region {
                x: 0,
                y: 0,
                width: 50,
                height: 32,
            },
            Region {
                x: 50,
                y: 0,
                width: 50,
                height: 32,
            },
        ];
        let samples = sample(&bytes, 100, 32, pitch, &regions).unwrap();
        assert_eq!(samples[0].average, Rgb(49, 39, 15));
        assert_eq!(samples[0].highlight, Rgb(255, 190, 40));
        assert_eq!(samples[1].average, Rgb(10, 10, 10));
        assert_eq!(samples[1].highlight, Rgb::default());
        assert_eq!(
            average(&bytes, 100, 32, pitch, &regions).unwrap(),
            samples.iter().map(|s| s.average).collect::<Vec<_>>()
        );
        // One hot pixel is diluted over the brightest 5%, not treated as a lamp.
        bytes.fill(0);
        bytes[..4].copy_from_slice(&[255, 255, 255, 255]);
        let samples = sample(&bytes, 100, 32, pitch, &regions).unwrap();
        assert_eq!(samples[0].highlight, Rgb::default());
        // Small regions use all their pixels up to the 16-pixel support minimum.
        let small = sample(
            &bytes,
            100,
            10,
            pitch,
            &[Region {
                x: 0,
                y: 0,
                width: 4,
                height: 1,
            }],
        )
        .unwrap();
        assert_eq!(small[0].highlight, Rgb::default());
    }
    #[test]
    fn whiteness_reference_black_greys_and_brightness_boost() {
        for brightness in [1.0, 2.0] {
            assert_eq!(
                corrected_led_output(&[Rgb(255, 255, 255)], 2.2, brightness, true),
                vec![Rgb(255, 177, 69)]
            );
        }
        assert_eq!(
            corrected_led_output(&[Rgb(0, 0, 0)], 2.2, 2.0, true),
            vec![Rgb(0, 0, 0)]
        );
        assert_eq!(
            corrected_led_output(&[Rgb(128, 128, 128)], 1.0, 1.0, true),
            vec![Rgb(128, 89, 35)]
        );
        assert_eq!(
            corrected_led_output(&[Rgb(255, 255, 255)], 1.0, 0.0, true),
            vec![Rgb(0, 0, 0)]
        );
    }
    #[test]
    fn whiteness_preserves_pure_colours_and_disabled_is_unchanged() {
        let colours = [
            Rgb(255, 0, 0),
            Rgb(0, 255, 0),
            Rgb(0, 0, 255),
            Rgb(255, 255, 0),
            Rgb(0, 255, 255),
            Rgb(255, 0, 255),
        ];
        assert_eq!(
            corrected_led_output(&colours, 2.2, 2.0, true),
            led_output(&colours, 2.2, 2.0)
        );
        let near_grey = [Rgb(200, 198, 200)];
        let raw = led_output(&near_grey, 1.0, 1.0);
        assert_eq!(corrected_led_output(&near_grey, 1.0, 1.0, false), raw);
        let corrected = corrected_led_output(&near_grey, 1.0, 1.0, true);
        assert_eq!(corrected[0].0, raw[0].0);
        assert!(corrected[0].1 < raw[0].1 && corrected[0].2 < raw[0].2);
        assert_eq!(corrected[0], Rgb(200, 139, 54));
    }
    #[test]
    fn pale_cyan_uses_full_warm_reference_including_measured_wow_samples() {
        assert_eq!(
            corrected_led_output(&[Rgb(120, 180, 200)], 1.0, 1.0, true),
            vec![Rgb(200, 139, 54)]
        );
        // Actual background capture from the user's ghost scene.
        let source = [Rgb(106, 156, 172)];
        let peak = led_output(&source, 2.2, 1.53)[0].2;
        assert_eq!(
            corrected_led_output(&source, 2.2, 1.53, true),
            led_output(&[WHITE_PWM_REFERENCE], 1.0, f64::from(peak) / 255.0)
        );
        // Neutral values stay bounded even at boosted output.
        for grey in 0..=255 {
            let input = [Rgb(grey, grey, grey)];
            let raw = led_output(&input, 2.2, 2.0)[0];
            let calibrated = corrected_led_output(&input, 2.2, 2.0, true)[0];
            assert_eq!(calibrated.0, raw.0);
            assert!(calibrated.1 <= raw.1 && calibrated.2 <= raw.2);
        }
    }
    #[test]
    fn correction_preserves_lavender_pink_and_other_unmatched_hues() {
        let colours = [
            Rgb(180, 130, 220),
            Rgb(220, 160, 240),
            Rgb(230, 170, 200),
            Rgb(200, 190, 180),
            Rgb(100, 200, 200),
            Rgb(10, 120, 200),
            Rgb(200, 10, 80),
        ];
        for brightness in [0.35, 1.0, 2.0] {
            assert_eq!(
                corrected_led_output(&colours, 2.2, brightness, true),
                led_output(&colours, 2.2, brightness)
            );
        }
    }
    #[test]
    fn tint_matching_is_brightness_independent_and_has_a_smooth_boundary() {
        for sample in [Rgb(53, 78, 86), Rgb(106, 156, 172), Rgb(79, 117, 129)] {
            assert!(white_correction_weight(sample) > 0.99);
        }
        // Move away from the calibrated axis toward purple, with no hard switch.
        let weights: Vec<_> = (180..=226)
            .rev()
            .map(|green| white_correction_weight(Rgb(156, green, 250)))
            .collect();
        assert_eq!(weights[0], 1.0);
        assert_eq!(*weights.last().unwrap(), 0.0);
        assert!(
            weights
                .windows(2)
                .all(|w| w[1] <= w[0] && w[0] - w[1] < 0.13)
        );
    }
    #[test]
    fn calibrated_white_reaches_every_installed_led_as_rgb_in_ddp() {
        let drive = corrected_led_output(&vec![Rgb(255, 255, 255); 137], 2.2, 2.0, true);
        let physical = crate::strip::installed(&drive).unwrap();
        let packets = crate::ddp::packets(&physical, 1).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].len(), 10 + 140 * 3);
        assert!(
            packets[0][10..]
                .as_chunks::<3>()
                .0
                .iter()
                .all(|rgb| *rgb == [255, 177, 69])
        );
    }
    #[test]
    fn changing_smoothing_preserves_state_and_zero_is_immediate() {
        let mut smoother = Smoother::new(Duration::from_secs(1));
        smoother.update(&[Rgb(0, 0, 0)], Duration::ZERO);
        let before = smoother.update(&[Rgb(255, 0, 0)], Duration::from_millis(100));
        smoother.set_duration(Duration::from_millis(500));
        assert_eq!(smoother.update(&[Rgb(255, 0, 0)], Duration::ZERO), before);
        smoother.set_duration(Duration::ZERO);
        assert_eq!(
            smoother.update(&[Rgb(255, 2, 3)], Duration::ZERO),
            vec![Rgb(255, 2, 3)]
        );
    }
    #[test]
    fn preview_encoding_does_not_change_led_drive() {
        let drive = led_output(&[Rgb(128, 64, 0)], 2.2, 0.35);
        let saved = drive.clone();
        let preview = preview_output(&drive, 2.2);
        assert!(preview[0].0 > drive[0].0 && preview[0].1 > drive[0].1);
        assert!(preview[0].0 > preview[0].1);
        assert_eq!(preview[0].2, 0);
        assert_eq!(drive, saved);
        assert_eq!(preview_output(&drive, 1.0), drive);
        assert_eq!(preview_output(&[Rgb(0, 0, 0)], 2.2), vec![Rgb(0, 0, 0)]);
    }
    #[test]
    fn led_gamma_and_brightness_order() {
        let input = [Rgb(0, 128, 255), Rgb(100, 100, 100)];
        assert_eq!(led_output(&input, 1.0, 1.0), input);
        let output = led_output(&input, 2.2, 0.35);
        assert_eq!(output[0], Rgb(0, 20, 89));
        assert_eq!(output[1], Rgb(11, 11, 11));
        assert_eq!(led_output(&input, 2.2, 0.0), vec![Rgb(0, 0, 0); 2]);
        assert_eq!(
            led_output(&[Rgb(255, 0, 0)], 2.2, 0.35),
            vec![Rgb(89, 0, 0)]
        );
    }
    #[test]
    fn ambient_controls_and_perimeter_wrap() {
        let input = [Rgb(255, 0, 0), Rgb(0, 0, 0), Rgb(0, 0, 0), Rgb(0, 0, 255)];
        assert_eq!(ambient(&input, 0.0, 1.0, 1.0), input);
        assert_eq!(ambient(&input, 1.0, 1.0, 1.0)[0], Rgb(0, 0, 128));
        assert_eq!(ambient(&input, 0.0, 0.0, 2.0), vec![Rgb(0, 0, 0); 4]);
        assert_eq!(
            ambient(&[Rgb(100, 50, 0)], 0.0, 1.0, 0.0),
            vec![Rgb(57, 57, 57)]
        );
        assert!(ambient(&[], 0.5, 1.0, 1.0).is_empty());
        assert_eq!(
            ambient(&[Rgb(255, 255, 255)], 0.5, 2.0, 2.0),
            vec![Rgb(255, 255, 255)]
        );
    }
    #[test]
    fn bgra_padding_and_average() {
        let bytes = [
            10, 20, 30, 0, 30, 40, 50, 255, 255, 255, 255, 255, 50, 60, 70, 1, 70, 80, 90, 2, 255,
            255, 255, 255,
        ];
        assert_eq!(
            average(
                &bytes,
                2,
                2,
                12,
                &[Region {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2
                }]
            )
            .unwrap(),
            vec![Rgb(60, 50, 40)]
        );
        assert!(average(&bytes[..20], 2, 2, 12, &[]).is_err());
        assert!(average(&bytes, 2, 2, 7, &[]).is_err());
    }
    #[test]
    fn subregion_excludes_surrounding_pixels() {
        let bytes = [0, 0, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255];
        assert_eq!(
            average(
                &bytes,
                3,
                1,
                12,
                &[Region {
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1
                }]
            )
            .unwrap(),
            vec![Rgb(0, 0, 255)]
        );
        assert!(
            average(
                &bytes,
                3,
                1,
                12,
                &[Region {
                    x: 3,
                    y: 0,
                    width: 1,
                    height: 1
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn smoothing_is_time_based() {
        let mut a = Smoother::new(Duration::from_secs(1));
        let mut b = Smoother::new(Duration::from_secs(1));
        for s in [&mut a, &mut b] {
            assert_eq!(
                s.update(&[Rgb(0, 0, 0)], Duration::ZERO),
                vec![Rgb(0, 0, 0)]
            );
        }
        let full = a.update(&[Rgb(255, 100, 0)], Duration::from_secs(1));
        b.update(&[Rgb(255, 100, 0)], Duration::from_millis(500));
        assert_eq!(
            full,
            b.update(&[Rgb(255, 100, 0)], Duration::from_millis(500))
        );
        assert_eq!(full, vec![Rgb(161, 63, 0)]);
        assert_eq!(a.update(&[Rgb(0, 0, 0)], Duration::ZERO), full);
        assert_eq!(
            Smoother::new(Duration::ZERO).update(&[Rgb(1, 2, 3)], Duration::ZERO),
            vec![Rgb(1, 2, 3)]
        );
    }
}
