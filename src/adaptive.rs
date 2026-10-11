use crate::colour::Rgb;
use std::time::Duration;

fn luminance(c: Rgb) -> f64 {
    (0.2126 * f64::from(c.0) + 0.7152 * f64::from(c.1) + 0.0722 * f64::from(c.2)) / 255.0
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Preserve global dark adaptation but let genuinely bright local sources shine.
/// Returns region source colours and scales; dark/non-highlight regions retain
/// exactly the original global output. Highlight data never feeds global scale.
pub fn local_highlights(
    averages: &[Rgb],
    highlights: &[Rgb],
    global: f64,
    enabled: bool,
) -> Vec<(Rgb, f64)> {
    let scene = if averages.is_empty() {
        0.0
    } else {
        averages.iter().copied().map(luminance).sum::<f64>() / averages.len() as f64
    };
    let dark = 1.0 - smoothstep((scene - 0.10) / 0.25);
    averages
        .iter()
        .copied()
        .enumerate()
        .map(|(i, average)| {
            let Some(&highlight) = highlights.get(i).filter(|_| enabled) else {
                return (average, global);
            };
            let light = luminance(highlight);
            let within_region = smoothstep((light - luminance(average) - 0.06) / 0.24);
            let above_scene = smoothstep((luminance(average) - scene - 0.08) / 0.30);
            let strength =
                dark * smoothstep((light - 0.18) / 0.42) * within_region.max(above_scene);
            let mix = strength * within_region;
            let blend =
                |a: u8, b: u8| (f64::from(a) + mix * (f64::from(b) - f64::from(a))).round() as u8;
            (
                Rgb(
                    blend(average.0, highlight.0),
                    blend(average.1, highlight.1),
                    blend(average.2, highlight.2),
                ),
                global + (2.0 - global).max(0.0) * strength,
            )
        })
        .collect()
}

/// A single scene-wide scale retains the relative colours of all LED regions.
/// Existing cropped/UI-adjusted edge samples act as a scene brightness proxy.
fn scene_scale(colours: &[Rgb]) -> Option<f64> {
    if colours.is_empty() {
        return None;
    }
    let luminance = colours.iter().copied().map(luminance).sum::<f64>() / colours.len() as f64;
    let t = ((luminance - 0.08) / (0.70 - 0.08)).clamp(0.0, 1.0);
    Some(0.15 + 1.85 * t * t * (3.0 - 2.0 * t))
}

pub struct Adaptive {
    scale: f64,
    enabled: bool,
}
impl Adaptive {
    pub fn new(manual: f64) -> Self {
        Self {
            scale: manual,
            enabled: false,
        }
    }
    pub fn update(
        &mut self,
        colours: &[Rgb],
        elapsed: Duration,
        enabled: bool,
        manual: f64,
    ) -> f64 {
        if !enabled {
            self.enabled = false;
            self.scale = manual;
            return manual;
        }
        if !self.enabled {
            self.scale = manual.clamp(0.0, 2.0);
            self.enabled = true;
        }
        if let Some(target) = scene_scale(colours) {
            // Separate 600 ms time constant prevents global brightness pumping,
            // even when per-region colour smoothing is disabled.
            let alpha = -(-elapsed.as_secs_f64() / 0.6).exp_m1();
            self.scale += alpha * (target - self.scale);
        }
        self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_lamp_boosts_only_its_region_while_scene_stays_dark() {
        let mut averages = vec![Rgb(10, 10, 10); 40];
        let mut highlights = averages.clone();
        averages[3] = Rgb(22, 19, 12);
        highlights[3] = Rgb(255, 190, 40);
        let global = scene_scale(&averages).unwrap();
        assert_eq!(global, 0.15);
        let regions = local_highlights(&averages, &highlights, global, true);
        assert!(regions[3].1 > 1.5 && regions[3].1 <= 2.0);
        assert!(
            regions[3].0.0 > 220
                && regions[3].0.0 > regions[3].0.1
                && regions[3].0.1 > regions[3].0.2
        );
        for (i, region) in regions.iter().enumerate() {
            if i != 3 {
                assert_eq!(*region, (averages[i], global));
            }
        }
        assert_eq!(scene_scale(&averages).unwrap(), global);
        let base = crate::colour::corrected_led_colour(averages[3], 2.2, global, false);
        let lit = crate::colour::corrected_led_colour(regions[3].0, 2.2, regions[3].1, false);
        assert!(lit.0 > base.0 + 100);
    }
    #[test]
    fn highlight_smoothing_releases_to_dark_and_zero_is_immediate() {
        let averages = vec![Rgb(10, 10, 10); 20];
        let mut lamp = averages.clone();
        lamp[0] = Rgb(255, 190, 40);
        let mut smoother = crate::colour::Smoother::new(Duration::from_millis(200));
        let lit = smoother.update(&lamp, Duration::ZERO);
        assert!(local_highlights(&averages, &lit, 0.15, true)[0].1 > 1.5);
        let fading = smoother.update(&averages, Duration::from_millis(100));
        let old_scale = local_highlights(&averages, &lit, 0.15, true)[0].1;
        let fading_scale = local_highlights(&averages, &fading, 0.15, true)[0].1;
        assert!(fading_scale > 0.15 && fading_scale < old_scale);
        // Missing frames retain the last highlight until a new source arrives.
        assert_eq!(smoother.update(&averages, Duration::ZERO), fading);
        smoother.set_duration(Duration::ZERO);
        let dark = smoother.update(&averages, Duration::ZERO);
        assert!(
            local_highlights(&averages, &dark, 0.15, true)
                .iter()
                .all(|&r| r == (Rgb(10, 10, 10), 0.15))
        );
    }
    #[test]
    fn local_highlights_respect_manual_mode_uniform_scenes_and_missing_samples() {
        for value in [0, 10, 120, 255] {
            let colours = vec![Rgb(value, value, value); 10];
            assert!(
                local_highlights(&colours, &colours, 0.35, true)
                    .iter()
                    .all(|&(c, scale)| c == colours[0] && scale == 0.35)
            );
        }
        let colours = [Rgb(10, 10, 10), Rgb(30, 20, 10)];
        let highlights = [Rgb(255, 255, 255); 2];
        assert_eq!(
            local_highlights(&colours, &highlights, 0.75, false),
            vec![(colours[0], 0.75), (colours[1], 0.75)]
        );
        assert_eq!(
            local_highlights(&colours, &[], 0.75, true),
            vec![(colours[0], 0.75), (colours[1], 0.75)]
        );
        assert!(local_highlights(&[], &[], 0.35, true).is_empty());
        // Noise and diluted hot pixels remain too dim to qualify.
        assert_eq!(
            local_highlights(&[Rgb(0, 0, 0)], &[Rgb(10, 10, 10)], 0.15, true),
            vec![(Rgb(0, 0, 0), 0.15)]
        );
    }
    #[test]
    fn bright_region_can_override_dark_global_scale_without_brightening_neighbours() {
        let mut colours = vec![Rgb(10, 10, 10); 20];
        colours[0] = Rgb(240, 240, 240);
        let regions = local_highlights(&colours, &colours, 0.15, true);
        assert_eq!(regions[0], (colours[0], 2.0));
        assert!(regions[1..].iter().all(|r| *r == (Rgb(10, 10, 10), 0.15)));
        let output = crate::colour::corrected_led_colour(regions[0].0, 2.2, regions[0].1, true);
        assert_eq!(output, Rgb(255, 177, 69));
    }
    #[test]
    fn dark_to_bright_is_monotonic_and_bounded() {
        let values: Vec<_> = (0..=255)
            .map(|v| scene_scale(&[Rgb(v, v, v)]).unwrap())
            .collect();
        assert_eq!(values[0], 0.15);
        assert_eq!(values[255], 2.0);
        assert!(values.windows(2).all(|w| w[0] <= w[1]));
        assert!(values.iter().all(|v| (0.15..=2.0).contains(v)));
        assert!(scene_scale(&[Rgb(0, 255, 0)]) > scene_scale(&[Rgb(0, 0, 255)]));
    }
    #[test]
    fn adaptation_is_time_based_and_manual_is_restored() {
        let mut a = Adaptive::new(0.75);
        let mut b = Adaptive::new(0.75);
        let bright = [Rgb(255, 255, 255)];
        let full = a.update(&bright, Duration::from_millis(600), true, 0.75);
        b.update(&bright, Duration::from_millis(300), true, 0.75);
        let halves = b.update(&bright, Duration::from_millis(300), true, 0.75);
        assert!((full - halves).abs() < 1e-12);
        assert!(full > 1.0 && full < 2.0);
        assert_eq!(a.update(&[], Duration::from_secs(1), true, 0.75), full);
        assert_eq!(a.update(&bright, Duration::ZERO, false, 0.35), 0.35);
        assert_eq!(a.update(&bright, Duration::ZERO, true, 0.35), 0.35);
        a.update(&bright, Duration::ZERO, false, 2.0);
        assert_eq!(a.update(&bright, Duration::ZERO, true, 2.0), 2.0);
    }
}
