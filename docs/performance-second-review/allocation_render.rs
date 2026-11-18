
#[test]
#[ignore = "allocation profile, not timing; run alone"]
fn probe_allocations() {
    use crate::allocation_probe::measure;
    let ctl = Controller::new();
    for (w, h) in [(800, 300), (1440, 540)] {
        let mut v = view(&ctl, w, h);
        v.theme_menu_open = true;
        let surface = ImageSurface::create(Format::ARgb32, w, h).unwrap();
        let mut cache = None;
        let cold = measure(|| frame(&v, &surface, &mut cache));
        let rebuild = measure(|| { cache = None; frame(&v, &surface, &mut cache); });
        let hot = measure(|| frame(&v, &surface, &mut cache));
        println!("allocations: {}", serde_json::json!({"size":[w,h], "cold":cold,"rebuild":rebuild,"hot":hot}));
        black_box(png(&surface));
    }
}
