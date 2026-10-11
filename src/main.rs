#![windows_subsystem = "windows"]

mod adaptive;
mod bars;
mod capture;
mod colour;
mod controls;
mod ddp;
mod editor;
mod layout;
mod output;
mod preferences;
mod profiles;
mod sampling;
mod setup;
mod strip;
mod taskbar;
mod ui;

use anyhow::{Result, ensure};
use clap::{CommandFactory, FromArgMatches, Parser};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    about = "DXGI screen-edge colour sampler (Windows)",
    args_override_self = true
)]
struct Config {
    /// Use the existing installation as a fallback until hardware Setup is saved.
    #[arg(long)]
    installed_strip: bool,
    /// Stream RGB output to a DDP receiver, e.g. 192.168.4.23:4048.
    #[arg(long)]
    ddp: Option<std::net::SocketAddr>,
    #[arg(long)]
    list_monitors: bool,
    #[arg(long, default_value_t = 0)]
    monitor: usize,
    #[arg(long, default_value_t = 32)]
    top: usize,
    #[arg(long, default_value_t = 18)]
    right: usize,
    #[arg(long, default_value_t = 32)]
    bottom: usize,
    #[arg(long, default_value_t = 18)]
    left: usize,
    #[arg(long, default_value_t = 120)]
    depth: usize,
    /// Compatibility alias for --profile WoW.
    #[arg(long, hide = true)]
    wow: bool,
    /// Select a saved sampling profile (otherwise restore the last selected profile).
    #[arg(long)]
    profile: Option<String>,
    /// Physical pixels to skip below bottom samples when no saved layout is selected.
    #[arg(long)]
    bottom_inset: Option<usize>,
    /// Neighbour blending strength, 0..1.
    #[arg(long, default_value_t = 0.35)]
    blend: f64,
    /// Output brightness multiplier, 0..2.
    #[arg(long, default_value_t = 1.0)]
    brightness: f64,
    /// Automatically scale global brightness from the sampled scene (GUI checkbox).
    #[arg(long)]
    adaptive_brightness: bool,
    /// Warm whites/near-greys using calibrated #FFE0A0, preserving saturated colours.
    #[arg(long)]
    whiteness_correction: bool,
    /// LED gamma exponent, 1..3. Default: 2.2 for installed strip, 1 otherwise.
    /// Disable receiver-side gamma to avoid applying it twice.
    #[arg(long)]
    gamma: Option<f64>,
    /// Saturation multiplier, 0..2; zero is greyscale.
    #[arg(long, default_value_t = 1.15)]
    saturation: f64,
    /// Show solid region blocks (the default; retained for compatibility).
    #[arg(long)]
    raw_preview: bool,
    /// Show a simulated diffuse wall glow instead of individual output colours.
    #[arg(long, conflicts_with = "raw_preview")]
    glow_preview: bool,
    /// Exponential smoothing time constant; zero disables smoothing.
    #[arg(long, default_value_t = 40)]
    smoothing_ms: u64,
    #[arg(long, default_value_t = 60)]
    fps: u32,
    #[arg(long)]
    debug_rgb: bool,
    #[arg(long)]
    no_preview: bool,
    /// Ignore saved GUI controls and do not save changes (also used by diagnostics).
    #[arg(long)]
    no_saved_settings: bool,
    /// Exit after this many updates (useful for bounded diagnostics).
    #[arg(long)]
    updates: Option<u64>,
    /// Disable automatic symmetric black-bar detection.
    #[arg(long)]
    no_auto_crop: bool,
    /// Maximum channel value considered black (0..255).
    #[arg(long, default_value_t = 8)]
    black_threshold: u8,
    #[arg(long, default_value_t = 500)]
    bar_delay_ms: u64,
    /// Manual crop in physical pixels: top,right,bottom,left. Overrides automatic detection.
    #[arg(long, value_delimiter = ',')]
    crop: Option<Vec<usize>>,
}
fn main() {
    let console_mode = std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--no-preview" | "--debug-rgb" | "--list-monitors" | "--help" | "-h" | "--updates"
        )
    });
    if console_mode {
        unsafe {
            let _ = windows::Win32::System::Console::AttachConsole(
                windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    if let Err(error) = run() {
        if console_mode {
            eprintln!("{error:#}");
        } else {
            let message: Vec<u16> = format!("{error:#}").encode_utf16().chain(Some(0)).collect();
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    None,
                    windows::core::PCWSTR(message.as_ptr()),
                    windows::core::w!("Backlight"),
                    windows::Win32::UI::WindowsAndMessaging::MB_OK
                        | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                );
            }
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = Config::command().get_matches();
    let mut config = Config::from_arg_matches(&matches)?;
    let explicit =
        |name| matches.value_source(name) == Some(clap::parser::ValueSource::CommandLine);
    // Launcher fallback values; explicit options and saved settings can override them.
    if config.installed_strip {
        if !explicit("brightness") {
            config.brightness = 0.35;
        }
        if !explicit("smoothing_ms") {
            config.smoothing_ms = 0;
        }
    }
    let monitors = capture::monitors()?;
    if config.list_monitors {
        for (i, m) in monitors.iter().enumerate() {
            println!("{i}: {} ({}×{})", m.name, m.width, m.height);
        }
        return Ok(());
    }
    ensure!((1..=240).contains(&config.fps), "fps must be 1..240");
    ensure!(
        (0.0..=1.0).contains(&config.blend)
            && (0.0..=2.0).contains(&config.brightness)
            && (0.0..=2.0).contains(&config.saturation),
        "blend must be 0..1; brightness and saturation must be 0..2"
    );
    let hardware_path = std::env::current_exe()?.with_file_name("backlight-hardware.ini");
    let mut hardware = if config.no_saved_settings {
        None
    } else {
        setup::load(&hardware_path)?
    };
    if let Some(h) = &mut hardware {
        config.ddp = Some(h.address);
        if explicit("monitor") {
            h.monitor = config.monitor;
        } else {
            config.monitor = h.monitor;
        }
    }
    let mut mapping = hardware
        .as_ref()
        .map(setup::Hardware::mapping)
        .transpose()?;
    let mut counts = if let Some(m) = &mapping {
        m.counts
    } else if config.installed_strip {
        strip::COUNTS
    } else {
        [config.top, config.right, config.bottom, config.left]
    };
    let mut gamma = config
        .gamma
        .unwrap_or(if config.installed_strip || hardware.is_some() {
            2.2
        } else {
            1.0
        });
    ensure!((1.0..=3.0).contains(&gamma), "gamma must be 1..3");
    let mut sender = config.ddp.map(ddp::Sender::new).transpose()?;
    ensure!(
        config.crop.as_ref().is_none_or(|v| v.len() == 4),
        "--crop requires four comma-separated values: top,right,bottom,left"
    );
    let manual = config.crop.as_ref().map(|v| bars::Crop {
        top: v[0],
        right: v[1],
        bottom: v[2],
        left: v[3],
    });
    let mut monitor = monitors
        .get(config.monitor)
        .ok_or_else(|| anyhow::anyhow!("invalid monitor index; use --list-monitors"))?;
    let initial_bounds = manual
        .unwrap_or_default()
        .bounds(monitor.width as usize, monitor.height as usize)?;
    let inset_for = |height: usize| {
        config
            .bottom_inset
            .unwrap_or(0)
            .min(height.saturating_sub(1))
    };
    layout::sampling_regions(
        manual
            .unwrap_or_default()
            .bounds(monitor.width as usize, monitor.height as usize)?
            .width,
        manual
            .unwrap_or_default()
            .bounds(monitor.width as usize, monitor.height as usize)?
            .height,
        counts,
        config.depth,
        inset_for(initial_bounds.height),
    )?;
    // The preview uses a fixed logical size; reject counts it cannot represent.
    let mut preview = if config.no_preview {
        None
    } else {
        layout::regions(800, 400, counts, 30)?;
        let draft = hardware.clone().unwrap_or(setup::Hardware {
            address: config
                .ddp
                .unwrap_or_else(|| "192.168.4.23:4048".parse().unwrap()),
            monitor: config.monitor,
            leds: if config.installed_strip {
                strip::LEDS
            } else {
                counts.iter().sum()
            },
            clockwise: true,
            corners: if config.installed_strip {
                [44, 93, 113, 24]
            } else {
                let n: usize = counts.iter().sum();
                [1, 1 + n / 4, 1 + n / 2, 1 + 3 * n / 4]
            },
            between: if config.installed_strip {
                [false, false, true, false]
            } else {
                [false; 4]
            },
        });
        let wizard = setup::Wizard::new(
            draft,
            monitors
                .iter()
                .enumerate()
                .map(|(i, m)| format!("{} - {} X {}", i + 1, m.width, m.height))
                .collect(),
        );
        Some(output::Preview::new(counts, !config.glow_preview, wizard)?)
    };
    let mut capture = None;
    let mut retry_at = Instant::now();
    let mut smoother = colour::Smoother::new(Duration::from_millis(config.smoothing_ms));
    let mut target = vec![colour::Rgb::default(); counts.iter().sum()];
    let mut highlight_target = target.clone();
    let mut has_frame = false;
    let mut last = Instant::now();
    let mut settings = controls::Settings {
        brightness: config.brightness,
        adaptive: config.adaptive_brightness,
        whiteness_correction: config.whiteness_correction,
        fps: config.fps,
        smoothing_ms: config.smoothing_ms,
    };
    let mut settings_warning = String::new();
    let mut preferences =
        if !config.no_preview && config.updates.is_none() && !config.no_saved_settings {
            let path = std::env::current_exe()?.with_file_name("backlight-settings.ini");
            let saved = match preferences::load(&path) {
                Ok(saved) => saved,
                Err(e) => {
                    eprintln!("{e:#}");
                    settings_warning = format!(" — settings warning: {e}");
                    None
                }
            };
            settings = preferences::restore(
                settings,
                saved,
                [
                    explicit("brightness"),
                    explicit("adaptive_brightness"),
                    explicit("fps"),
                    explicit("smoothing_ms"),
                    explicit("whiteness_correction"),
                ],
            );
            let mut store = preferences::Store::new(path);
            store.schedule(settings);
            Some(store)
        } else {
            None
        };
    let mut adaptive = adaptive::Adaptive::new(settings.brightness);
    let executable = std::env::current_exe()?;
    let profile_dir = executable.parent().unwrap();
    if !config.no_saved_settings {
        profiles::migrate(profile_dir)?;
    }
    let requested = config
        .profile
        .as_deref()
        .or(if config.wow { Some("WoW") } else { None });
    let mut active_profile = if config.no_saved_settings {
        "Default".to_string()
    } else {
        profiles::selected(profile_dir, requested)?
    };
    let mut layout_path = profiles::path(profile_dir, &active_profile)?;
    let mut custom_layout = if !config.no_saved_settings {
        match sampling::load(&layout_path) {
            Ok(p) => p,
            Err(e) => {
                settings_warning = format!(" — layout warning: {e}");
                None
            }
        }
    } else {
        None
    };
    if !config.no_saved_settings {
        profiles::remember(profile_dir, &active_profile)?;
    }
    if let Some(p) = &mut preview {
        p.editor.profile = custom_layout.clone();
        p.profiles.active = active_profile.clone();
        p.profiles.names = profiles::names(profile_dir)?;
    }
    let mut effective_brightness = settings.brightness;
    let mut next_update = Instant::now();
    let mut preview_colours = target.clone();
    let mut status = String::from("waiting for capture");
    let mut updates = 0u64;
    let mut captured_frames = 0u64;
    let mut tracker = bars::Tracker::new(Duration::from_millis(config.bar_delay_ms));
    let mut candidate = bars::Crop::default();
    let mut cached: Option<capture::Frame> = None;
    let mut sampling_key = None;
    let mut sample_regions = Vec::new();
    let mut blocked_regions = Vec::new();
    let mut ui_shifted = 0;
    let run_started = Instant::now();
    loop {
        if config.updates.is_some_and(|limit| updates >= limit) {
            break;
        }
        let start = Instant::now();
        if preview.as_ref().is_some_and(|p| !p.open()) {
            break;
        }
        if let Some(p) = &mut preview {
            let old_settings = settings;
            p.interact(&mut settings);
            if let Some(candidate) = p.setup.apply.take() {
                let applied = (|| -> Result<_> {
                    let selected = monitors
                        .get(candidate.monitor)
                        .ok_or_else(|| anyhow::anyhow!("Invalid screen"))?;
                    let new_mapping = candidate.mapping()?;
                    let bounds = manual
                        .unwrap_or_default()
                        .bounds(selected.width as usize, selected.height as usize)?;
                    layout::sampling_regions(
                        bounds.width,
                        bounds.height,
                        new_mapping.counts,
                        config.depth,
                        inset_for(bounds.height),
                    )?;
                    let new_sender = ddp::Sender::new(candidate.address)?;
                    ensure!(
                        !config.no_saved_settings,
                        "Restart without --no-saved-settings to save Setup"
                    );
                    setup::save(&hardware_path, &candidate)?;
                    Ok((new_mapping, new_sender))
                })();
                match applied {
                    Ok((new_mapping, new_sender)) => {
                        // Clear the previous controller/strip before changing its length.
                        if let Some(old) = &mut sender {
                            let n = hardware.as_ref().map(|h| h.leds).unwrap_or(
                                if config.installed_strip {
                                    strip::LEDS
                                } else {
                                    counts.iter().sum()
                                },
                            );
                            let _ = old.send(&vec![[0; 3]; n]);
                        }
                        counts = new_mapping.counts;
                        if !explicit("gamma") {
                            gamma = 2.2;
                        }
                        mapping = Some(new_mapping);
                        monitor = &monitors[candidate.monitor];
                        sender = Some(new_sender);
                        hardware = Some(candidate.clone());
                        target = vec![colour::Rgb::default(); counts.iter().sum()];
                        highlight_target = target.clone();
                        preview_colours = target.clone();
                        smoother =
                            colour::Smoother::new(Duration::from_millis(settings.smoothing_ms));
                        capture = None;
                        cached = None;
                        has_frame = false;
                        retry_at = start;
                        sampling_key = None;
                        sample_regions.clear();
                        blocked_regions.clear();
                        tracker = bars::Tracker::new(Duration::from_millis(config.bar_delay_ms));
                        if let Some(profile) = &mut custom_layout {
                            profile.parts.clear();
                        }
                        if let Err(e) =
                            sampling::save_optional(&layout_path, custom_layout.as_ref())
                        {
                            settings_warning = format!(" — layout save warning: {e}");
                        }
                        p.reconfigure(counts, custom_layout.clone());
                        p.setup.accepted(candidate);
                        next_update = start;
                        last = start;
                    }
                    Err(e) => p.setup.message = e.to_string(),
                }
            }
            if p.editor.profile != custom_layout {
                custom_layout = p.editor.profile.clone();
                sampling_key = None;
                next_update = start;
            }
            if p.editor.dirty && p.editor.released() && !config.no_saved_settings {
                let saved = sampling::save_optional(&layout_path, custom_layout.as_ref());
                match saved {
                    Ok(()) => p.editor.dirty = false,
                    Err(e) => {
                        settings_warning = format!(" — cannot save layout: {e}");
                    }
                }
            }
            if let Some(request) = p.profiles.request.take() {
                let result = (|| -> Result<(String, Option<sampling::Profile>)> {
                    ensure!(
                        !config.no_saved_settings,
                        "Restart without --no-saved-settings to use profiles"
                    );
                    sampling::save_optional(&layout_path, custom_layout.as_ref())?;
                    let (name, create) = match request {
                        profiles::Request::Select(name) => (name, false),
                        profiles::Request::Create(name) => (name, true),
                    };
                    let path = profiles::path(profile_dir, &name)?;
                    if create {
                        ensure!(
                            !name.eq_ignore_ascii_case("default") && !path.exists(),
                            "That profile already exists"
                        );
                        let profile = custom_layout.clone().unwrap_or_else(|| {
                            sampling::Profile::preset(
                                initial_bounds.width,
                                initial_bounds.height,
                                config.depth,
                                0,
                                false,
                            )
                        });
                        sampling::save(&path, &profile)?;
                    }
                    let profile = sampling::load(&path)?;
                    ensure!(
                        name.eq_ignore_ascii_case("default") || profile.is_some(),
                        "Profile does not exist"
                    );
                    profiles::remember(profile_dir, &name)?;
                    Ok((name, profile))
                })();
                match result {
                    Ok((name, profile)) => {
                        active_profile = name;
                        layout_path = profiles::path(profile_dir, &active_profile)?;
                        custom_layout = profile;
                        p.reconfigure(counts, custom_layout.clone());
                        p.profiles.active = active_profile.clone();
                        p.profiles.names = profiles::names(profile_dir)?;
                        p.profiles.open = false;
                        sampling_key = None;
                        next_update = start;
                    }
                    Err(e) => p.profiles.message = e.to_string(),
                }
            }
            if settings != old_settings {
                next_update = start;
                if let Some(store) = &mut preferences {
                    store.schedule(settings);
                }
            }
        }
        if let Some(store) = &mut preferences
            && let Err(e) = store.flush(false)
        {
            eprintln!("{e:#}");
            settings_warning = format!(" — settings warning: {e}");
        }
        if let Some(p) = &mut preview
            && p.setup.open
        {
            p.setup.tick();
            p.draw(&preview_colours, &status, settings, effective_brightness)?;
            std::thread::sleep(Duration::from_millis(16));
            // Calibration replaces normal lighting only while Setup is open.
            last = Instant::now();
            next_update = last;
            continue;
        }
        // Service mouse/window events at least 60 Hz even with a slow LED rate.
        if start < next_update {
            if let Some(p) = &mut preview {
                p.draw(
                    &preview_colours,
                    &format!("{status}{settings_warning}"),
                    settings,
                    effective_brightness,
                )?;
            }
            std::thread::sleep((next_update - start).min(Duration::from_millis(16)));
            continue;
        }
        smoother.set_duration(Duration::from_millis(settings.smoothing_ms));
        if capture.is_none() && start >= retry_at {
            match capture::Capture::new(&monitor.name) {
                Ok(c) => {
                    capture = Some(c);
                    status = "waiting for frame".into();
                }
                Err(e) => {
                    status = format!("capture unavailable: {e}");
                    eprintln!("{status}");
                    retry_at = start + Duration::from_secs(1);
                }
            }
        }
        if let Some(c) = &mut capture {
            match c.next() {
                Ok(Some(frame)) => {
                    captured_frames += 1;
                    if cached
                        .as_ref()
                        .is_some_and(|old| (old.width, old.height) != (frame.width, frame.height))
                    {
                        tracker = bars::Tracker::new(Duration::from_millis(config.bar_delay_ms));
                    }
                    candidate = if manual.is_none() && !config.no_auto_crop {
                        bars::detect(&frame, config.black_threshold)
                    } else {
                        bars::Crop::default()
                    };
                    cached = Some(frame);
                }
                Ok(None) => (), // Desktop is unchanged: retain target and continue smoothing.
                Err(e) => {
                    status = format!("capture lost: {e}");
                    eprintln!("{status}");
                    capture = None;
                    cached = None;
                    tracker = bars::Tracker::new(Duration::from_millis(config.bar_delay_ms));
                    retry_at = start + Duration::from_secs(1);
                }
            }
        }
        let elapsed = start.duration_since(last);
        last = start;
        if capture.is_some()
            && let Some(frame) = &cached
        {
            // An unchanged DXGI desktop is still evidence for the cached candidate.
            let mut active = manual.unwrap_or_else(|| tracker.update(candidate, elapsed));
            let mut bounds = active.bounds(frame.width, frame.height)?;
            if manual.is_none()
                && (counts[0] > bounds.width
                    || counts[2] > bounds.width
                    || counts[1] > bounds.height
                    || counts[3] > bounds.height)
            {
                active = bars::Crop::default();
                bounds = active.bounds(frame.width, frame.height)?;
            }
            let bottom_inset = inset_for(bounds.height);
            let mut runtime_zones = taskbar::visible_regions(monitor.origin, bounds);
            let preview_zone = preview
                .as_ref()
                .and_then(|p| p.sampling_exclusion(monitor.origin, bounds));
            runtime_zones.extend(preview_zone);
            let key = (
                frame.width,
                frame.height,
                active,
                bottom_inset,
                runtime_zones.clone(),
            );
            if sampling_key.as_ref() != Some(&key) {
                if let Some(p) = &mut preview {
                    p.editor.set_preset(sampling::Profile::preset(
                        bounds.width,
                        bounds.height,
                        config.depth,
                        bottom_inset,
                        false,
                    ));
                }
                sample_regions = layout::sampling_regions(
                    bounds.width,
                    bounds.height,
                    counts,
                    config.depth,
                    bottom_inset,
                )?;
                let zones = runtime_zones;
                ui_shifted = ui::avoid_edges(
                    &mut sample_regions,
                    counts,
                    &zones,
                    bounds.width,
                    bounds.height,
                );
                blocked_regions = vec![false; sample_regions.len()];
                for i in counts[0] + counts[1]..counts[0] + counts[1] + counts[2] {
                    blocked_regions[i] = zones
                        .iter()
                        .any(|&z| sampling::overlaps(sample_regions[i], z));
                }
                // If no clear same-sized reading fits, never sample our own UI.
                if let Some(zone) = preview_zone {
                    for (r, blocked) in sample_regions.iter().zip(&mut blocked_regions) {
                        *blocked |= r.x < zone.x + zone.width
                            && zone.x < r.x + r.width
                            && r.y < zone.y + zone.height
                            && zone.y < r.y + r.height;
                    }
                }
                if let Some(profile) = &custom_layout {
                    (sample_regions, blocked_regions) = if key.4.is_empty() {
                        profile.regions(bounds.width, bounds.height, counts)
                    } else {
                        profile.regions_with_zones(bounds.width, bounds.height, counts, &key.4)
                    };
                }
                for r in &mut sample_regions {
                    r.x += bounds.x;
                    r.y += bounds.y;
                }
                sampling_key = Some(key);
            }
            if let Some(p) = &mut preview {
                p.editor
                    .capture(frame, bounds, &sample_regions, &blocked_regions);
            }
            if settings.adaptive {
                let samples = colour::sample(
                    &frame.data,
                    frame.width,
                    frame.height,
                    frame.pitch,
                    &sample_regions,
                )?;
                target = samples.iter().map(|s| s.average).collect();
                highlight_target = samples.iter().map(|s| s.highlight).collect();
                // Keep highlights local; neighbour blending applies only to averages.
                highlight_target = colour::ambient(&highlight_target, 0.0, 1.0, config.saturation);
            } else {
                target = colour::average(
                    &frame.data,
                    frame.width,
                    frame.height,
                    frame.pitch,
                    &sample_regions,
                )?;
                highlight_target = target.clone();
            }
            for (i, blocked) in blocked_regions.iter().enumerate() {
                if *blocked {
                    target[i] = colour::Rgb::default();
                    highlight_target[i] = colour::Rgb::default();
                }
            }
            target = colour::ambient(&target, config.blend, 1.0, config.saturation);
            has_frame = true;
            status = format!(
                "{} — {}×{} — crop {},{},{},{} — bottom inset {} — UI shifts {}",
                monitor.name,
                frame.width,
                frame.height,
                active.top,
                active.right,
                active.bottom,
                active.left,
                bottom_inset,
                ui_shifted
            );
        }
        effective_brightness = adaptive.update(
            if has_frame { &target } else { &[] },
            elapsed,
            settings.adaptive,
            settings.brightness,
        );
        let regions = adaptive::local_highlights(
            &target,
            &highlight_target,
            effective_brightness,
            settings.adaptive,
        );
        let mut colours: Vec<_> = if !settings.adaptive {
            colour::corrected_led_output(
                &target,
                gamma,
                effective_brightness,
                settings.whiteness_correction,
            )
        } else {
            regions
                .into_iter()
                .map(|(source, brightness)| {
                    colour::corrected_led_colour(
                        source,
                        gamma,
                        brightness,
                        settings.whiteness_correction,
                    )
                })
                .collect()
        };
        // Smooth the actual LED drive after all nonlinear processing: local
        // boost, gamma, clipping and white correction must also fade smoothly.
        if has_frame {
            colours = smoother.update(&colours, elapsed);
        }
        for (i, blocked) in blocked_regions.iter().enumerate() {
            if *blocked {
                colours[i] = colour::Rgb::default();
            }
        }
        if config.debug_rgb {
            output::debug(&colours);
        }
        if let Some(sender) = &mut sender {
            let physical = if let Some(m) = &mapping {
                m.output(&colours)
            } else if config.installed_strip {
                strip::installed(&colours)?
            } else {
                colours.iter().map(|c| [c.0, c.1, c.2]).collect()
            };
            sender.send(&physical)?;
        }
        if let Some(p) = &mut preview {
            preview_colours = colour::preview_output(&colours, gamma);
            p.draw(
                &preview_colours,
                &format!("{status}{settings_warning}"),
                settings,
                effective_brightness,
            )?;
        }
        updates += 1;
        next_update = start + settings.period();
        if let Some(remaining) = settings.period().checked_sub(start.elapsed()) {
            std::thread::sleep(if preview.is_some() {
                remaining.min(Duration::from_millis(16))
            } else {
                remaining
            });
        }
    }
    if !config.no_saved_settings
        && let Some(p) = &preview
        && p.editor.dirty
        && let Err(e) = sampling::save_optional(&layout_path, p.editor.profile.as_ref())
    {
        eprintln!("Cannot save layout on exit: {e}");
    }
    if let Some(store) = &mut preferences
        && let Err(e) = store.flush(true)
    {
        eprintln!("{e:#}");
    }
    if let Some(sender) = &mut sender {
        let count = if let Some(h) = &hardware {
            h.leds
        } else if config.installed_strip {
            strip::LEDS
        } else {
            counts.iter().sum()
        };
        sender.send(&vec![[0; 3]; count])?;
    }
    if config.updates.is_some() {
        eprintln!(
            "Completed {updates} updates at {:.1} updates/s; acquired {captured_frames} frames; {status}",
            updates as f64 / run_started.elapsed().as_secs_f64()
        );
        ensure!(
            has_frame,
            "no desktop frame was captured during the diagnostic run"
        );
    }
    Ok(())
}
