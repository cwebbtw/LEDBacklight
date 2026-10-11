//! Offline layout inspection: no native window, capture API or lighting packets.
use super::*;
use crate::{controls::Settings, output::View};
use std::collections::HashMap;

fn fixture() -> View {
    let hardware = crate::setup::Hardware {
        address: "127.0.0.1:4048".parse().unwrap(),
        monitor: 0,
        leds: 140,
        clockwise: true,
        corners: [44, 93, 113, 24],
        between: [false, false, true, false],
    };
    let counts = crate::strip::COUNTS;
    let mut view = View {
        editor: crate::editor::Editor::new(counts),
        setup: crate::setup::Wizard::new(
            hardware,
            vec!["1 - 3440 X 1440".into(), "2 - 2560 X 1440".into()],
        ),
        profiles: crate::profiles::Picker::default(),
        settings: Some(Settings {
            brightness: 1.0,
            adaptive: true,
            whiteness_correction: true,
            fps: 100,
            smoothing_ms: 175,
        }),
        effective_brightness: 1.25,
        colours: (0..counts.iter().sum())
            .map(|i| {
                crate::colour::Rgb(
                    40 + (i % 50) as u8,
                    110 + (i % 90) as u8,
                    140 + (i % 100) as u8,
                )
            })
            .collect(),
        counts,
        raw: true,
        status: "Capture active".into(),
        alive: true,
        context: None,
        hwnd: 0,
        next_repaint: Instant::now(),
    };
    view.profiles.active = "WoW".into();
    view.profiles.names = vec!["Default".into(), "WoW".into(), "Movies".into()];
    view.editor
        .set_preset(crate::sampling::Profile::preset(3440, 1440, 120, 0, false));
    view
}
fn input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        ..Default::default()
    }
}
#[test]
fn repaint_does_not_overwrite_pending_control_changes() {
    let mut view = fixture();
    let original = view.settings.unwrap();
    view.settings.as_mut().unwrap().brightness = 1.7;
    view.draw(&[], "test", original, 1.2).unwrap();
    let mut engine_settings = original;
    view.interact(&mut engine_settings);
    assert_eq!(engine_settings.brightness, 1.7);
}
#[test]
fn rendering_all_pages_preserves_engine_and_sampling_settings() {
    let ctx = egui::Context::default();
    theme(&ctx);
    let mut view = fixture();
    let original = view.settings;
    let mut canvas = Canvas::default();
    let _ = ctx.run(input(), |ctx| screen(ctx, &mut view, &mut canvas));
    view.profiles.preview_mode(true, false);
    let _ = ctx.run(input(), |ctx| screen(ctx, &mut view, &mut canvas));
    view.profiles.open = false;
    for step in 0..7 {
        view.setup.preview_page(step);
        let _ = ctx.run(input(), |ctx| screen(ctx, &mut view, &mut canvas));
        assert!(view.setup.apply.is_none());
        assert_eq!(view.settings, original);
    }
}
#[test]
#[ignore = "offline modern UI visual review"]
fn render_modern_screens() {
    std::fs::create_dir_all("target/ui-review").unwrap();
    let ctx = egui::Context::default();
    theme(&ctx);
    let mut view = fixture();
    let mut canvas = Canvas::default();
    let mut textures = HashMap::new();
    render(&ctx, &mut view, &mut canvas, &mut textures, "lighting");
    view.profiles.preview_mode(false, false);
    render(&ctx, &mut view, &mut canvas, &mut textures, "profiles");
    view.profiles.preview_mode(true, false);
    render(&ctx, &mut view, &mut canvas, &mut textures, "new-profile");
    view.profiles.preview_mode(false, true);
    render(
        &ctx,
        &mut view,
        &mut canvas,
        &mut textures,
        "delete-profile",
    );
    view.profiles.open = false;
    for step in 0..7 {
        view.setup.preview_page(step);
        render(
            &ctx,
            &mut view,
            &mut canvas,
            &mut textures,
            &format!("setup-{step}"),
        );
    }
    view.setup.open = false;
    view.editor.open = true;
    let screenshot = std::env::var("BACKLIGHT_GUI_IMAGE")
        .ok()
        .and_then(|path| image::open(path).ok());
    if let Some(image) = screenshot {
        let rgba = image.to_rgba8();
        let (w, h) = rgba.dimensions();
        let data = rgba
            .pixels()
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        let frame = crate::capture::Frame {
            width: w as usize,
            height: h as usize,
            pitch: w as usize * 4,
            data,
        };
        let profile = crate::sampling::load(std::path::Path::new(
            "target/release/backlight-profile-wow.ini",
        ))
        .unwrap()
        .unwrap_or_else(|| crate::sampling::Profile::preset(w as usize, h as usize, 120, 0, false));
        let (readings, blocked) = profile.regions(w as usize, h as usize, view.counts);
        view.editor.profile = Some(profile);
        view.editor.capture(
            &frame,
            crate::layout::Region {
                x: 0,
                y: 0,
                width: w as usize,
                height: h as usize,
            },
            &readings,
            &blocked,
        );
    }
    render(&ctx, &mut view, &mut canvas, &mut textures, "sampling");
}
fn render(
    ctx: &egui::Context,
    view: &mut View,
    canvas: &mut Canvas,
    textures: &mut HashMap<egui::TextureId, egui::ColorImage>,
    name: &str,
) {
    for frame in 0..2 {
        let out = ctx.run(input(), |ctx| screen(ctx, view, canvas));
        for (id, delta) in &out.textures_delta.set {
            let egui::ImageData::Color(image) = &delta.image;
            if let Some([x, y]) = delta.pos {
                let target = textures.get_mut(id).unwrap();
                for row in 0..image.size[1] {
                    let dest = (y + row) * target.size[0] + x;
                    target.pixels[dest..dest + image.size[0]].copy_from_slice(
                        &image.pixels[row * image.size[0]..(row + 1) * image.size[0]],
                    );
                }
            } else {
                textures.insert(*id, (**image).clone());
            }
        }
        if frame == 0 {
            continue;
        }
        let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut pixels = vec![BACKGROUND.to_array(); 1200 * 800];
        for clipped in meshes {
            let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else {
                continue;
            };
            let Some(texture) = textures.get(&mesh.texture_id) else {
                continue;
            };
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let v = [
                    mesh.vertices[triangle[0] as usize],
                    mesh.vertices[triangle[1] as usize],
                    mesh.vertices[triangle[2] as usize],
                ];
                let minx = v
                    .iter()
                    .map(|v| v.pos.x)
                    .fold(f32::INFINITY, f32::min)
                    .max(clipped.clip_rect.left())
                    .floor()
                    .max(0.0) as usize;
                let maxx = v
                    .iter()
                    .map(|v| v.pos.x)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .min(clipped.clip_rect.right())
                    .ceil()
                    .min(1200.0) as usize;
                let miny = v
                    .iter()
                    .map(|v| v.pos.y)
                    .fold(f32::INFINITY, f32::min)
                    .max(clipped.clip_rect.top())
                    .floor()
                    .max(0.0) as usize;
                let maxy = v
                    .iter()
                    .map(|v| v.pos.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .min(clipped.clip_rect.bottom())
                    .ceil()
                    .min(800.0) as usize;
                let edge = |a: egui::Pos2, b: egui::Pos2, p: egui::Pos2| {
                    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
                };
                let area = edge(v[0].pos, v[1].pos, v[2].pos);
                if area.abs() < 0.0001 {
                    continue;
                }
                for y in miny..maxy {
                    for x in minx..maxx {
                        let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let w = [
                            edge(v[1].pos, v[2].pos, p) / area,
                            edge(v[2].pos, v[0].pos, p) / area,
                            edge(v[0].pos, v[1].pos, p) / area,
                        ];
                        if w.iter().any(|w| *w < -0.00001) {
                            continue;
                        }
                        let u = (0..3).map(|i| v[i].uv.x * w[i]).sum::<f32>();
                        let vv = (0..3).map(|i| v[i].uv.y * w[i]).sum::<f32>();
                        let tx = (u * texture.size[0] as f32) as usize;
                        let ty = (vv * texture.size[1] as f32) as usize;
                        let tex = texture.pixels[ty.min(texture.size[1] - 1) * texture.size[0]
                            + tx.min(texture.size[0] - 1)]
                        .to_array();
                        let mut src = [0.0; 4];
                        for c in 0..4 {
                            src[c] = (0..3)
                                .map(|i| v[i].color.to_array()[c] as f32 * w[i])
                                .sum::<f32>()
                                * tex[c] as f32
                                / 255.0;
                        }
                        let dst = &mut pixels[y * 1200 + x];
                        let alpha = src[3] / 255.0;
                        for c in 0..3 {
                            dst[c] =
                                (src[c] + dst[c] as f32 * (1.0 - alpha)).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
        let bytes: Vec<_> = pixels.into_iter().flatten().collect();
        image::save_buffer(
            format!("target/ui-review/{name}.png"),
            &bytes,
            1200,
            800,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
