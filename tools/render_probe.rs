// Included only by render.rs's test build; also portable to the pinned baselines.
use super::*;
use std::hint::black_box;

fn view(ctl: &Controller, width: i32, height: i32) -> View<'_> {
    View {
        width, height, buf_w: width, buf_h: height, theme: THEMES[0], theme_idx: 0,
        layout_idx: 0, grid_cols: grid_cols(layout_at(0)), interaction: Interaction::Idle,
        flash_key: -1, px: 0, py: 0, inside: false, handle_anim: 0.0,
        theme_menu_open: false, fn_on: false, held: &[], caps_on: false, ctl,
    }
}

fn png(surface: &ImageSurface) -> Vec<u8> {
    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).unwrap();
    bytes
}

fn frame(v: &View, surface: &ImageSurface, cache: &mut Option<KeyCache>) {
    let cr = Context::new(surface).unwrap();
    let bar = v.measure_bar(&cr);
    v.paint(&cr, &bar, cache);
    surface.flush();
}

#[test]
fn version_does_not_cover_keys_or_resize_handle() {
    let ctl = Controller::new();
    for layout in 0..ALL_LAYOUTS.len() {
        for (w, h) in [(200, 180), (400, 180), (800, 300), (1440, 540), (3840, 2160)] {
            let mut v = view(&ctl, w, h);
            v.layout_idx = layout;
            v.grid_cols = grid_cols(layout_at(layout));
            let r = v.version_rect();
            assert!(r.w >= 20.0 && r.h >= 9.0, "legible target: layout={layout}, {w}x{h}, {}x{}", r.w, r.h);
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= f64::from(w) && r.y + r.h <= f64::from(h));
            let overlaps = |x: f64, y: f64, width: f64, height: f64| {
                r.x < x + width && x < r.x + r.w && r.y < y + height && y < r.y + r.h
            };
            let hs = f64::from(v.handle_size());
            assert!(!overlaps(f64::from(w) - hs, f64::from(h) - hs, hs, hs), "handle: layout={layout}, {w}x{h}");
            for (row, keys) in v.layout().rows.iter().enumerate() {
                for (col, key) in keys.keys.iter().enumerate() {
                    if key.is_gap() { continue; }
                    let (x, y, width, height) = v.key_rect(row, col);
                    assert!(!overlaps(x, y, width, height), "key: layout={layout}, {w}x{h}, {row}/{col}");
                    if key.code_u16() == key::KEY_SCROLLLOCK { assert!(r.y >= y + height); }
                }
            }
        }
    }
}

#[test]
fn cache_sequences_match_fresh_reconstruction() {
    let mut ctl = Controller::new();
    let mut cache = None;
    let directory = std::env::var_os("OSK_SEQUENCE_OUT").map(PathBuf::from);
    if let Some(dir) = &directory { std::fs::create_dir_all(dir).unwrap(); }
    let sizes = [(1, 1), (2, 3), (7, 5), (200, 180), (721, 341), (800, 300)];
    let mut cases = 0;
    for step in 0..234 {
        let layout = step % ALL_LAYOUTS.len();
        let theme = (step / ALL_LAYOUTS.len()) % THEMES.len();
        let (w, h) = sizes[(step / 39) % sizes.len()];
        let scale = [1.0, 1.25, 1.5, 2.0][step % 4];
        if step % 5 == 0 {
            ctl.click_key(&Key::latch("shift", key::KEY_LEFTSHIFT, 20), step as u64 * 500, &mut DiscardSink);
        }
        let mut v = view(&ctl, w, h);
        v.layout_idx = layout;
        v.grid_cols = grid_cols(layout_at(layout));
        v.theme_idx = theme;
        v.theme = THEMES[theme];
        v.buf_w = (w as f64 * scale).round() as i32;
        v.buf_h = (h as f64 * scale).round() as i32;
        v.fn_on = step % 3 == 0;
        v.caps_on = step % 4 == 0;
        v.held = if step % 2 == 0 { &[key::KEY_RIGHTALT, key::KEY_A] } else { &[] };
        v.theme_menu_open = step % 2 != 0;
        v.handle_anim = (step % 5) as f64 / 4.0;
        let reused = ImageSurface::create(Format::ARgb32, v.buf_w, v.buf_h).unwrap();
        let fresh = ImageSurface::create(Format::ARgb32, v.buf_w, v.buf_h).unwrap();
        frame(&v, &reused, &mut cache);
        frame(&v, &fresh, &mut None);
        let expected = png(&fresh);
        assert_eq!(png(&reused), expected, "state transition {step}");
        // Exercise the warm cache after the transition/reconstruction check.
        frame(&v, &reused, &mut cache);
        assert_eq!(png(&reused), expected, "cache revisit {step}");
        if let Some(dir) = &directory { std::fs::write(dir.join(format!("{step:03}.png")), expected).unwrap(); }
        cases += 1;
    }
    assert_eq!(cases, 234);
}

#[test]
#[ignore = "explicit performance experiment; run in fresh alternating processes"]
fn probe_cold_rebuild_hot() {
    let ctl = Controller::new();
    let mut results = Vec::new();
    for (w, h) in [(800, 300), (1440, 540)] {
        let mut v = view(&ctl, w, h);
        v.theme_menu_open = true;
        let mut cache = None;
        let start = Instant::now();
        let surface = ImageSurface::create(Format::ARgb32, w, h).unwrap();
        frame(&v, &surface, &mut cache);
        let first_us = start.elapsed().as_secs_f64() * 1e6;
        let expected = png(&surface);
        results.push(serde_json::json!({"mode":"first", "size":[w,h], "us":[first_us], "frames":1}));
        for (mode, n) in [("rebuild", 80), ("hot", 500)] {
            let mut times = Vec::with_capacity(n);
            for _ in 0..n {
                let start = Instant::now();
                if mode == "rebuild" { cache = None; }
                frame(black_box(&v), black_box(&surface), black_box(&mut cache));
                times.push(start.elapsed().as_secs_f64() * 1e6);
            }
            assert_eq!(png(&surface), expected, "{mode} output");
            results.push(serde_json::json!({"mode":mode, "size":[w,h], "us":times, "frames":n}));
        }
    }
    println!("probe: {}", serde_json::to_string(&results).unwrap());
}


#[test]
fn toolbar_revisits_match_a_new_surface() {
    let ctl = Controller::new();
    let output = std::env::var_os("OSK_BAR_SEQUENCE_OUT").map(PathBuf::from);
    if let Some(dir) = &output { std::fs::create_dir_all(dir).unwrap(); }
    for (case, (w, h, bw, bh)) in [(1, 1, 1, 1), (7, 5, 11, 8), (201, 181, 251, 226),
                                   (721, 341, 1082, 512), (800, 300, 800, 300)].into_iter().enumerate() {
        let mut v = view(&ctl, w, h);
        v.buf_w = bw;
        v.buf_h = bh;
        let surface = ImageSurface::create(Format::ARgb32, bw, bh).unwrap();
        let mut cache = None;
        for (step, target) in ["theme", "layout", "min", "close", "version", "outside", "key",
                               "theme", "close", "min", "outside", "layout", "theme"].into_iter().enumerate() {
            v.theme_menu_open = (3..9).contains(&step);
            let cr = Context::new(&surface).unwrap();
            let bar = v.measure_bar(&cr);
            v.inside = target != "outside";
            if let Some(r) = bar_hover_target(&bar, target) {
                v.px = (r.x + r.w / 2.0) as i32;
                v.py = (r.y + r.h / 2.0) as i32;
            } else {
                v.px = w / 3;
                v.py = h / 2;
            }
            frame(&v, &surface, &mut cache);
            let fresh = ImageSurface::create(Format::ARgb32, bw, bh).unwrap();
            frame(&v, &fresh, &mut None);
            let image = png(&surface);
            assert!(image == png(&fresh), "toolbar case {case}, step {step}, {target}");
            if let Some(dir) = &output {
                std::fs::write(dir.join(format!("{case}-{step:02}.png")), image).unwrap();
            }
        }
    }
}



#[test]
fn source_copy_preserves_pixels_outside_the_source() {
    for (src_h, dst_h) in [(2, 5), (5, 2), (3, 3)] {
        let solid = |height, rgba: [f64; 4]| {
            let s = ImageSurface::create_offscreen(Format::ARgb32, 3, height).unwrap();
            let cr = Context::new(&s).unwrap();
            cr.set_operator(Operator::Source);
            cr.set_source_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
            cr.paint().unwrap();
            s
        };
        let src = solid(src_h, [0.0, 1.0, 0.0, 0.5]);
        let dst = solid(dst_h, [1.0, 0.0, 0.0, 1.0]);
        let expected = solid(dst_h, [1.0, 0.0, 0.0, 1.0]);
        let cr = Context::new(&dst).unwrap();
        cr.set_operator(Operator::Source);
        cr.set_source_surface(&src, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
        let oracle = Context::new(&expected).unwrap();
        oracle.set_operator(Operator::Source);
        oracle.set_source_rgba(0.0, 1.0, 0.0, 0.5);
        oracle.rectangle(0.0, 0.0, 3.0, src_h.min(dst_h) as f64);
        oracle.fill().unwrap();
        assert_eq!(png(&dst), png(&expected));
    }
}


#[test]
fn clipped_colored_glyphs_match_inside_a_larger_surface() {
    for (i, (bold, x, y)) in [(false, -6.0, 11.0), (true, 23.0, 6.0), (false, 0.0, -2.0), (true, 10.0, 20.0)].into_iter().enumerate() {
        let draw = |size, shift: f64| {
            let s = ImageSurface::create_offscreen(Format::ARgb32, size, size).unwrap();
            let cr = Context::new(&s).unwrap();
            cr.set_operator(Operator::Source);
            cr.set_source_rgba(0.25, 0.5, 0.125, 0.6);
            cr.paint().unwrap();
            cr.rectangle(shift + 1.0, shift + 1.0, 28.0, 16.0);
            cr.clip();
            cr.select_font_face(FONT, FontSlant::Normal, if bold { FontWeight::Bold } else { FontWeight::Normal });
            cr.set_font_size(13.25);
            cr.set_source_rgba(0.85, 0.2, 0.7, 0.65);
            cr.move_to(shift + x, shift + y);
            cr.show_text("ÁgW").unwrap();
            png(&s)
        };
        let small = draw(32, 0.0);
        let large = draw(64, 16.0);
        let small_pixels = tiny_skia::Pixmap::decode_png(&small).unwrap();
        let large_pixels = tiny_skia::Pixmap::decode_png(&large).unwrap();
        for yy in 0..32 {
            for xx in 0..32 {
                assert_eq!(small_pixels.pixel(xx, yy), large_pixels.pixel(xx + 16, yy + 16));
            }
        }
        if let Some(dir) = std::env::var_os("OSK_TEXT_SEQUENCE_OUT").map(PathBuf::from) {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{i}.png")), small).unwrap();
        }
    }
}
