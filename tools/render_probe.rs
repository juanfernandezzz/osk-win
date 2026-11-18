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
