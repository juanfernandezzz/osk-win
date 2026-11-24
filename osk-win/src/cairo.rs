//! The subset of the cairo API that `render.rs` draws with, powered by
//! tiny-skia and fontdue with a GDI backbuffer for blitting to the window HDC.
//!
//! Vector paths (arcs, rounded rects, lines, strokes, fills, clips) and text
//! are rendered with subpixel anti-aliasing and alpha blending into a tiny-skia
//! `Pixmap`. When `blit_to_hdc` is called, the pixels are transferred to the
//! GDI DIB section and blitted to the window device context.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::ffi::c_void;
use std::io::Write;
use std::ptr;
use std::rc::Rc;
use std::sync::LazyLock;

use fontdue::{Font, FontSettings};
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, LineJoin,
    Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

type Hdc = *mut c_void;
type Hgdiobj = *mut c_void;

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

const SRCCOPY: u32 = 0x00CC_0020;

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    fn DeleteDC(hdc: Hdc) -> i32;
    fn CreateDIBSection(
        hdc: Hdc,
        info: *const BitmapInfoHeader,
        usage: u32,
        bits: *mut *mut c_void,
        section: *mut c_void,
        offset: u32,
    ) -> Hgdiobj;
    fn SelectObject(hdc: Hdc, obj: Hgdiobj) -> Hgdiobj;
    fn DeleteObject(obj: Hgdiobj) -> i32;
    fn BitBlt(
        dst: Hdc,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: Hdc,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
    fn GdiFlush() -> i32;
}

static EMBEDDED_FONT: &[u8] = include_bytes!("../assets/AdwaitaSans-Regular.ttf");
static FONT: LazyLock<Font> = LazyLock::new(|| {
    Font::from_bytes(EMBEDDED_FONT, FontSettings::default()).expect("invalid embedded font")
});

// One UI thread, one bounded cache. Color and simulated bold are applied after
// rasterization; the exact pixel-size bits and character identify the bitmap.
const GLYPH_CACHE_BYTES: usize = 64 * 1024;
const GLYPH_CACHE_ENTRIES: usize = 256;

#[derive(Default)]
struct GlyphCache {
    glyphs: HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>,
    bytes: usize,
    #[cfg(test)]
    rasterizations: usize,
}

impl GlyphCache {
    fn with_glyph<R>(&mut self, ch: char, size: f32, draw: impl FnOnce(&fontdue::Metrics, &[u8]) -> R) -> R {
        let key = (ch, size.to_bits());
        if let Some((metrics, bitmap)) = self.glyphs.get(&key) {
            return draw(metrics, bitmap);
        }
        #[cfg(test)]
        { self.rasterizations += 1; }
        let (metrics, bitmap) = FONT.rasterize(ch, size);
        let result = draw(&metrics, &bitmap);
        // Oversized glyphs are drawn once without being retained. Resizing can
        // produce arbitrarily many sizes, so cap bytes and entry metadata.
        if bitmap.len() <= GLYPH_CACHE_BYTES {
            if self.bytes + bitmap.len() > GLYPH_CACHE_BYTES || self.glyphs.len() == GLYPH_CACHE_ENTRIES {
                self.glyphs.clear();
                self.bytes = 0;
            }
            self.bytes += bitmap.len();
            self.glyphs.insert(key, (metrics, bitmap));
        }
        result
    }
}

thread_local! {
    static GLYPHS: RefCell<GlyphCache> = RefCell::new(GlyphCache::default());
}

#[derive(Debug, Clone, Copy)]
pub struct Error;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Drawing failed")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Format {
    ARgb32,
}

#[derive(Clone, Copy, Debug)]
pub enum FontSlant {
    Normal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Source,
    Over,
}

pub use tiny_skia::LineCap;

#[derive(Clone, Copy, Debug)]
pub struct TextExtents {
    width: f64,
    height: f64,
    y_bearing: f64,
}

impl TextExtents {
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }
    pub fn x_bearing(&self) -> f64 {
        0.0
    }
    pub fn y_bearing(&self) -> f64 {
        self.y_bearing
    }
}

struct Surface {
    pixmap: RefCell<Pixmap>,
    dc: Hdc,
    bitmap: Hgdiobj,
    old_bitmap: Hgdiobj,
    pixels: *mut u8,
    w: i32,
    h: i32,
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.dc.is_null() {
            return;
        }
        unsafe {
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

#[derive(Clone)]
pub struct ImageSurface(Rc<Surface>);

impl ImageSurface {
    /// A pixel-only surface: cached layers never need a GDI presentation buffer.
    pub fn create_offscreen(_format: Format, w: i32, h: i32) -> Result<Self, Error> {
        let (w, h) = (w.max(1), h.max(1));
        let pixmap = Pixmap::new(w as u32, h as u32).ok_or(Error)?;
        Ok(Self(Rc::new(Surface {
            pixmap: RefCell::new(pixmap),
            dc: ptr::null_mut(),
            bitmap: ptr::null_mut(),
            old_bitmap: ptr::null_mut(),
            pixels: ptr::null_mut(),
            w,
            h,
        })))
    }

    pub fn create(format: Format, w: i32, h: i32) -> Result<Self, Error> {
        let mut surface = Self::create_offscreen(format, w, h)?;
        let s = Rc::get_mut(&mut surface.0).ok_or(Error)?;
        let (w, h) = (s.w, s.h);

        let info = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w,
            height: -h,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        };
        let mut pixels: *mut c_void = ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(ptr::null_mut(), &info, 0, &mut pixels, ptr::null_mut(), 0)
        };
        if bitmap.is_null() || pixels.is_null() {
            return Err(Error);
        }
        let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
        if dc.is_null() {
            unsafe { DeleteObject(bitmap) };
            return Err(Error);
        }
        let old_bitmap = unsafe { SelectObject(dc, bitmap) };

        s.dc = dc;
        s.bitmap = bitmap;
        s.old_bitmap = old_bitmap;
        s.pixels = pixels as *mut u8;
        Ok(surface)
    }

    pub fn width(&self) -> i32 {
        self.0.w
    }

    pub fn height(&self) -> i32 {
        self.0.h
    }

    pub fn flush(&self) {
        if self.0.pixels.is_null() {
            return;
        }
        let pm = self.0.pixmap.borrow();
        let src = pm.data();
        let dst = unsafe {
            std::slice::from_raw_parts_mut(self.0.pixels, (self.0.w * self.0.h * 4) as usize)
        };
        let (src_chunks, _) = src.as_chunks::<4>();
        let (dst_chunks, _) = dst.as_chunks_mut::<4>();
        for (s, d) in src_chunks.iter().zip(dst_chunks.iter_mut()) {
            d[0] = s[2]; // B
            d[1] = s[1]; // G
            d[2] = s[0]; // R
            d[3] = s[3]; // A
        }
        unsafe { GdiFlush() };
    }

    pub fn blit_to_hdc(&self, hdc: *mut c_void) {
        self.flush();
        unsafe { BitBlt(hdc, 0, 0, self.0.w, self.0.h, self.0.dc, 0, 0, SRCCOPY) };
    }

    /// Raw-copy the rows `src` shares with this surface (same width, no blending).
    pub fn copy_top_from(&self, src: &ImageSurface) {
        if Rc::ptr_eq(&self.0, &src.0) || self.0.w != src.0.w {
            return;
        }
        let src = src.0.pixmap.borrow();
        let mut dst = self.0.pixmap.borrow_mut();
        let n = dst.data().len().min(src.data().len());
        dst.data_mut()[..n].copy_from_slice(&src.data()[..n]);
    }

    pub fn write_to_png<W: Write>(&self, out: &mut W) -> Result<(), Error> {
        let pm = self.0.pixmap.borrow();
        let png = pm.encode_png().map_err(|_| Error)?;
        out.write_all(&png).map_err(|_| Error)
    }
}

#[derive(Clone)]
struct Gstate {
    color: Color,
    line_width: f32,
    line_cap: LineCap,
    blend: BlendMode,
    transform: Transform,
    font_size: f32,
    bold: bool,
    clip_mask: Option<Mask>,
    source_surface: Option<ImageSurface>,
}

struct State {
    g: Gstate,
    stack: Vec<Gstate>,
    pb: PathBuilder,
    cur: Option<(f64, f64)>,
}

pub struct Context {
    surface: ImageSurface,
    st: RefCell<State>,
}

impl Context {
    pub fn new(surface: &ImageSurface) -> Result<Self, Error> {
        Ok(Self {
            surface: surface.clone(),
            st: RefCell::new(State {
                g: Gstate {
                    color: Color::from_rgba8(0, 0, 0, 255),
                    line_width: 2.0,
                    line_cap: LineCap::Butt,
                    blend: BlendMode::SourceOver,
                    transform: Transform::identity(),
                    font_size: 10.0,
                    bold: false,
                    clip_mask: None,
                    source_surface: None,
                },
                stack: Vec::new(),
                pb: PathBuilder::new(),
                cur: None,
            }),
        })
    }

    pub fn target(&self) -> &ImageSurface {
        &self.surface
    }

    pub fn save(&self) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        let g = st.g.clone();
        st.stack.push(g);
        Ok(())
    }

    pub fn restore(&self) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        if let Some(g) = st.stack.pop() {
            st.g = g;
        }
        Ok(())
    }

    pub fn identity_matrix(&self) {
        self.st.borrow_mut().g.transform = Transform::identity();
    }

    pub fn scale(&self, sx: f64, sy: f64) {
        let mut st = self.st.borrow_mut();
        st.g.transform = st.g.transform.pre_scale(sx as f32, sy as f32);
    }

    pub fn set_operator(&self, op: Operator) {
        self.st.borrow_mut().g.blend = match op {
            Operator::Source => BlendMode::Source,
            Operator::Over => BlendMode::SourceOver,
        };
    }

    pub fn set_source_rgba(&self, r: f64, g: f64, b: f64, a: f64) {
        let mut st = self.st.borrow_mut();
        st.g.color = Color::from_rgba(r as f32, g as f32, b as f32, a as f32).unwrap_or(Color::BLACK);
        st.g.source_surface = None;
    }

    pub fn set_source_surface(&self, surface: &ImageSurface, _x: f64, _y: f64) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        st.g.source_surface = Some(surface.clone());
        Ok(())
    }

    pub fn set_line_width(&self, w: f64) {
        self.st.borrow_mut().g.line_width = w as f32;
    }

    pub fn set_line_cap(&self, cap: LineCap) {
        self.st.borrow_mut().g.line_cap = cap;
    }

    pub fn new_path(&self) {
        let mut st = self.st.borrow_mut();
        st.pb.clear();
        st.cur = None;
    }

    pub fn new_sub_path(&self) {
        self.st.borrow_mut().cur = None;
    }

    pub fn move_to(&self, x: f64, y: f64) {
        let mut st = self.st.borrow_mut();
        st.pb.move_to(x as f32, y as f32);
        st.cur = Some((x, y));
    }

    pub fn line_to(&self, x: f64, y: f64) {
        let mut st = self.st.borrow_mut();
        if st.cur.is_none() {
            st.pb.move_to(x as f32, y as f32);
        } else {
            st.pb.line_to(x as f32, y as f32);
        }
        st.cur = Some((x, y));
    }

    pub fn arc(&self, cx: f64, cy: f64, r: f64, a1: f64, a2: f64) {
        let mut a2 = a2;
        while a2 < a1 {
            a2 += 2.0 * PI;
        }
        // line_to starts the subpath itself when there is no current point.
        let (mut sin1, mut cos1) = a1.sin_cos();
        self.line_to(cx + r * cos1, cy + r * sin1);

        let mut cur_angle = a1;
        while cur_angle < a2 {
            let sweep = (a2 - cur_angle).min(PI / 2.0);
            let next_angle = cur_angle + sweep;
            let k = 4.0 / 3.0 * (sweep / 4.0).tan();

            let (sin2, cos2) = next_angle.sin_cos();

            let cp0x = cx + r * cos1;
            let cp0y = cy + r * sin1;
            let p1x = cp0x - k * r * sin1;
            let p1y = cp0y + k * r * cos1;

            let p3x = cx + r * cos2;
            let p3y = cy + r * sin2;
            let p2x = p3x + k * r * sin2;
            let p2y = p3y - k * r * cos2;

            self.st.borrow_mut().pb.cubic_to(
                p1x as f32, p1y as f32,
                p2x as f32, p2y as f32,
                p3x as f32, p3y as f32,
            );
            cur_angle = next_angle;
            (sin1, cos1) = (sin2, cos2);
        }
        self.st.borrow_mut().cur = Some((cx + r * a2.cos(), cy + r * a2.sin()));
    }

    pub fn close_path(&self) {
        let mut st = self.st.borrow_mut();
        st.pb.close();
    }

    pub fn rectangle(&self, x: f64, y: f64, w: f64, h: f64) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close_path();
    }

    pub fn fill(&self) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        let pb = std::mem::replace(&mut st.pb, PathBuilder::new());
        st.cur = None;
        let Some(path) = pb.finish() else {
            return Ok(());
        };
        let mut paint = Paint::default();
        paint.set_color(st.g.color);
        paint.blend_mode = st.g.blend;
        paint.anti_alias = true;
        let mut pm = self.surface.0.pixmap.borrow_mut();
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            st.g.transform,
            st.g.clip_mask.as_ref(),
        );
        st.pb = path.clear();
        Ok(())
    }

    pub fn stroke(&self) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        let pb = std::mem::replace(&mut st.pb, PathBuilder::new());
        st.cur = None;
        let Some(path) = pb.finish() else {
            return Ok(());
        };
        let mut paint = Paint::default();
        paint.set_color(st.g.color);
        paint.blend_mode = st.g.blend;
        paint.anti_alias = true;
        let stroke = Stroke {
            width: st.g.line_width,
            miter_limit: 4.0,
            line_cap: st.g.line_cap,
            line_join: LineJoin::Round,
            dash: None,
        };
        let mut pm = self.surface.0.pixmap.borrow_mut();
        pm.stroke_path(
            &path,
            &paint,
            &stroke,
            st.g.transform,
            st.g.clip_mask.as_ref(),
        );
        st.pb = path.clear();
        Ok(())
    }

    pub fn clip(&self) {
        let mut st = self.st.borrow_mut();
        let pb = std::mem::replace(&mut st.pb, PathBuilder::new());
        st.cur = None;
        let Some(path) = pb.finish() else {
            return;
        };
        let (w, h) = (self.surface.0.w as u32, self.surface.0.h as u32);
        let ts = st.g.transform;
        let has_curves = path.segments().any(|seg| matches!(seg, tiny_skia::PathSegment::CubicTo(..) | tiny_skia::PathSegment::QuadTo(..)));
        if let Some(ref mut mask) = st.g.clip_mask {
            mask.intersect_path(&path, FillRule::Winding, has_curves, ts);
        } else {
            let mut mask = match Mask::new(w, h) {
                Some(m) => m,
                None => return,
            };
            mask.fill_path(&path, FillRule::Winding, has_curves, ts);
            st.g.clip_mask = Some(mask);
        }
        st.pb = path.clear();
    }

    pub fn reset_clip(&self) {
        self.st.borrow_mut().g.clip_mask = None;
    }

    pub fn paint(&self) -> Result<(), Error> {
        let mut st = self.st.borrow_mut();
        let (w, h) = (self.surface.0.w as u32, self.surface.0.h as u32);
        if let Some(src) = st.g.source_surface.take() {
            if st.g.transform.is_identity() && st.g.clip_mask.is_none() && st.g.blend == BlendMode::Source && self.surface.0.w == src.0.w {
                self.surface.copy_top_from(&src);
                return Ok(());
            }
            let src_pm = src.0.pixmap.borrow();
            let mut pm = self.surface.0.pixmap.borrow_mut();
            let paint = PixmapPaint {
                opacity: 1.0,
                blend_mode: st.g.blend,
                quality: FilterQuality::Nearest,
            };
            pm.draw_pixmap(0, 0, src_pm.as_ref(), &paint, st.g.transform, st.g.clip_mask.as_ref());
            return Ok(());
        }

        let Some(rect) = Rect::from_xywh(0.0, 0.0, w as f32, h as f32) else {
            return Ok(());
        };
        let mut paint = Paint::default();
        paint.set_color(st.g.color);
        paint.blend_mode = st.g.blend;
        paint.anti_alias = false;
        let mut pm = self.surface.0.pixmap.borrow_mut();
        pm.fill_rect(rect, &paint, Transform::identity(), st.g.clip_mask.as_ref());
        Ok(())
    }

    pub fn select_font_face(&self, _family: &str, _slant: FontSlant, weight: FontWeight) {
        self.st.borrow_mut().g.bold = weight == FontWeight::Bold;
    }

    pub fn set_font_size(&self, size: f64) {
        self.st.borrow_mut().g.font_size = size as f32;
    }

    pub fn text_extents(&self, s: &str) -> Result<TextExtents, Error> {
        let st = self.st.borrow();
        let font_size = st.g.font_size;
        let mut total_w = 0.0f32;
        for ch in s.chars() {
            let metrics = FONT.metrics(ch, font_size);
            total_w += metrics.advance_width;
        }
        let line = FONT.horizontal_line_metrics(font_size).unwrap_or(fontdue::LineMetrics {
            ascent: font_size * 0.8,
            descent: -font_size * 0.2,
            line_gap: 0.0,
            new_line_size: font_size * 1.2,
        });
        let (sx, sy) = (st.g.transform.sx as f64, st.g.transform.sy as f64);
        Ok(TextExtents {
            width: total_w as f64 / sx.max(0.001),
            height: (line.ascent - line.descent) as f64 / sy.max(0.001),
            y_bearing: -line.ascent as f64 / sy.max(0.001),
        })
    }

    pub fn show_text(&self, s: &str) -> Result<(), Error> {
        let st = self.st.borrow();
        let (cur_x, cur_y) = st.cur.unwrap_or((0.0, 0.0));
        let mut pt = tiny_skia::Point::from_xy(cur_x as f32, cur_y as f32);
        st.g.transform.map_point(&mut pt);
        let mut base_x = pt.x;
        let base_y = pt.y;
        let px_size = st.g.font_size * st.g.transform.sy;
        let mut pm = self.surface.0.pixmap.borrow_mut();
        let (pw, ph) = (pm.width() as i64, pm.height() as i64);
        let color = st.g.color;
        let src = [color.red(), color.green(), color.blue(), 1.0];
        let data = pm.data_mut();

        GLYPHS.with(|cache| {
            let mut cache = cache.borrow_mut();
            for ch in s.chars() {
                cache.with_glyph(ch, px_size, |metrics, bitmap| {
                    let gx = i64::from((base_x + metrics.xmin as f32).round() as i32);
                    let gy = i64::from((base_y - (metrics.ymin + metrics.height as i32) as f32).round() as i32);
                    let (gw, gh) = (metrics.width as i64, metrics.height as i64);
                    // Visible glyph rows; columns are clipped per pass below.
                    let (ly0, ly1) = ((-gy).max(0), gh.min(ph - gy));

                    for pass in 0..1 + st.g.bold as i64 {
                        let x0 = gx + pass;
                        let (lx0, lx1) = ((-x0).max(0), gw.min(pw - x0));
                        for ly in ly0..ly1 {
                            let row = (gy + ly) * pw + x0;
                            for lx in lx0..lx1 {
                                let cov = bitmap[(ly * gw + lx) as usize];
                                if cov == 0 {
                                    continue;
                                }
                                let idx = (row + lx) as usize;
                                let mut alpha_f = (cov as f32 / 255.0) * color.alpha();
                                if let Some(ref mask) = st.g.clip_mask {
                                    let mask_cov = mask.data()[idx];
                                    if mask_cov == 0 {
                                        continue;
                                    }
                                    alpha_f *= mask_cov as f32 / 255.0;
                                }
                                let inv_a = 1.0 - alpha_f;
                                for (d, s) in data[idx * 4..idx * 4 + 4].iter_mut().zip(src) {
                                    *d = ((s * alpha_f + *d as f32 / 255.0 * inv_a).clamp(0.0, 1.0) * 255.0).round() as u8;
                                }
                            }
                        }
                    }
                    base_x += metrics.advance_width * st.g.transform.sx;
                });
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reused_paths_discard_geometry_across_operations() {
        fn draw(cr: &Context, i: usize) {
            cr.new_path();
            cr.rectangle(0.0, 0.0, 32.0, 24.0);
            cr.new_path(); // abandon a populated path
            cr.move_to(1.0, 1.0);
            cr.fill().unwrap(); // a single point produces no finished path
            cr.stroke().unwrap(); // an empty path also produces no geometry
            cr.set_source_rgba(0.2, 0.4, 0.7, 0.2);
            cr.rectangle((i % 13) as f64, (i % 7) as f64, 5.0, 4.0);
            cr.fill().unwrap();
            cr.set_line_width(0.7);
            cr.move_to(-2.0, (i % 23) as f64);
            cr.line_to(25.0, (i % 17) as f64);
            cr.stroke().unwrap();
            cr.rectangle(3.0, 2.0, 7.0, 11.0);
            cr.clip();
            cr.paint().unwrap();
            cr.reset_clip();
        }
        let actual = ImageSurface::create_offscreen(Format::ARgb32, 32, 24).unwrap();
        let expected = ImageSurface::create_offscreen(Format::ARgb32, 32, 24).unwrap();
        let reused = Context::new(&actual).unwrap();
        for i in 0..128 {
            draw(&reused, i);
            draw(&Context::new(&expected).unwrap(), i);
            assert_eq!(actual.0.pixmap.borrow().data(), expected.0.pixmap.borrow().data(), "operation sequence {i}");
        }
    }

    #[test]
    fn native_blit_preserves_bgra_pixels_on_tiny_and_regular_surfaces() {
        for (w, h) in [(1, 1), (2, 3), (97, 65), (800, 300)] {
            let source = ImageSurface::create(Format::ARgb32, w, h).unwrap();
            let destination = ImageSurface::create(Format::ARgb32, w, h).unwrap();
            {
                let mut pixmap = source.0.pixmap.borrow_mut();
                for (i, pixel) in pixmap.data_mut().chunks_exact_mut(4).enumerate() {
                    pixel.copy_from_slice(&[(i % 251) as u8, (i % 127) as u8, (i % 61) as u8, 255]);
                }
            }
            source.blit_to_hdc(destination.0.dc);
            // GDI writes are batched; read DIB memory only after synchronization.
            unsafe { GdiFlush(); }
            let native = unsafe { std::slice::from_raw_parts(destination.0.pixels, (w * h * 4) as usize) };
            let pixmap = source.0.pixmap.borrow();
            for (rgba, bgra) in pixmap.data().chunks_exact(4).zip(native.chunks_exact(4)) {
                assert_eq!(bgra, &[rgba[2], rgba[1], rgba[0], rgba[3]]);
            }
        }
    }

    #[test]
    fn glyph_hits_evictions_oversize_and_revisits() {
        let mut cache = GlyphCache::default();
        let check = |cache: &mut GlyphCache, ch, size| {
            let (expected, pixels) = FONT.rasterize(ch, size);
            cache.with_glyph(ch, size, |metrics, bitmap| {
                assert_eq!(*metrics, expected);
                assert_eq!(bitmap, pixels);
            });
            assert_eq!(cache.bytes, cache.glyphs.values().map(|(_, b)| b.len()).sum::<usize>());
            assert!(cache.bytes <= GLYPH_CACHE_BYTES);
            assert!(cache.glyphs.len() <= GLYPH_CACHE_ENTRIES);
        };
        check(&mut cache, 'A', 12.0);
        check(&mut cache, 'A', 12.0);
        assert_eq!(cache.rasterizations, 1, "a hit must avoid rasterizing");
        check(&mut cache, 'A', f32::from_bits(12.0f32.to_bits() + 1));
        assert_eq!(cache.rasterizations, 2, "exact size bits identify a glyph");

        // Empty payloads isolate the metadata/entry limit.
        cache = GlyphCache::default();
        for i in 1..=GLYPH_CACHE_ENTRIES {
            check(&mut cache, ' ', i as f32);
        }
        assert_eq!(cache.glyphs.len(), GLYPH_CACHE_ENTRIES);
        check(&mut cache, ' ', 257.0);
        assert_eq!(cache.glyphs.len(), 1);
        check(&mut cache, ' ', 1.0);
        assert_eq!(cache.rasterizations, 258, "evicted entries must be rebuilt");

        cache = GlyphCache::default();
        let mut byte_evictions = 0;
        for size in 200..220 {
            let previous = cache.bytes;
            check(&mut cache, 'W', size as f32);
            if cache.bytes < previous { byte_evictions += 1; }
        }
        assert!(byte_evictions > 0, "exercise the byte limit before the entry limit");
        let before = cache.rasterizations;
        check(&mut cache, 'W', 200.0);
        assert_eq!(cache.rasterizations, before + 1);
        let retained = (cache.bytes, cache.glyphs.len());
        let (_, huge) = FONT.rasterize('W', 800.0);
        assert!(huge.len() > GLYPH_CACHE_BYTES);
        for _ in 0..2 { check(&mut cache, 'W', 800.0); }
        assert_eq!((cache.bytes, cache.glyphs.len()), retained);
        assert_eq!(cache.rasterizations, before + 3);
    }

    #[test]
    fn nested_clips_restore_pixels_on_tiny_surfaces() {
        for (w, h) in [(1, 1), (2, 3), (7, 5), (96, 64)] {
            let nested = ImageSurface::create_offscreen(Format::ARgb32, w, h).unwrap();
            let reference = ImageSurface::create_offscreen(Format::ARgb32, w, h).unwrap();
            for (surface, nesting) in [(&nested, true), (&reference, false)] {
                let cr = Context::new(surface).unwrap();
                cr.rectangle(0.0, 0.0, w as f64, h as f64);
                cr.clip();
                if nesting {
                    cr.save().unwrap();
                    cr.rectangle(0.0, 0.0, 0.5, 0.5);
                    cr.clip();
                    cr.save().unwrap();
                    cr.scale(1.25, 1.5);
                    cr.reset_clip();
                    cr.restore().unwrap();
                    cr.restore().unwrap();
                }
                cr.set_source_rgba(0.2, 0.4, 0.8, 0.7);
                cr.paint().unwrap();
                cr.set_font_size(9.125);
                cr.move_to(-2.0, 5.0);
                cr.show_text("ÑAg").unwrap();
            }
            assert_eq!(nested.0.pixmap.borrow().data(), reference.0.pixmap.borrow().data(), "{w}x{h}");
        }
    }

    #[test]
    fn cached_glyphs_match_font_rasterization_across_sizes_and_eviction() {
        let mut cache = GlyphCache::default();
        for size in [0.0, 8.0, 8.125, 12.0, 24.0, 64.0, 400.0] {
            for ch in "AaÑñßİáÉ0123!? ⇧←✓\n".chars() {
                let (expected, pixels) = FONT.rasterize(ch, size);
                for _ in 0..2 {
                    cache.with_glyph(ch, size, |metrics, bitmap| {
                        assert_eq!(bitmap, pixels);
                        assert_eq!((metrics.width, metrics.height, metrics.xmin, metrics.ymin),
                                   (expected.width, expected.height, expected.xmin, expected.ymin));
                        assert_eq!(metrics.advance_width.to_bits(), expected.advance_width.to_bits());
                    });
                }
                assert!(cache.bytes <= GLYPH_CACHE_BYTES);
                assert!(cache.glyphs.len() <= GLYPH_CACHE_ENTRIES);
            }
        }
        // Zero-sized bitmaps still need a metadata bound.
        for n in 1..=600 {
            cache.with_glyph(' ', n as f32 / 16.0, |_, _| ());
            assert!(cache.glyphs.len() <= GLYPH_CACHE_ENTRIES);
        }
    }

    #[test]
    fn offscreen_layers_have_identical_pixels_without_native_buffers() {
        let offscreen = ImageSurface::create_offscreen(Format::ARgb32, 96, 64).unwrap();
        let presented = ImageSurface::create(Format::ARgb32, 96, 64).unwrap();
        assert!(offscreen.0.dc.is_null());
        assert!(offscreen.0.pixels.is_null());
        assert!(!presented.0.dc.is_null());
        for surface in [&offscreen, &presented] {
            let cr = Context::new(surface).unwrap();
            cr.set_source_rgba(0.2, 0.4, 0.6, 1.0);
            cr.paint().unwrap();
            cr.rectangle(5.0, 5.0, 80.0, 50.0);
            cr.clip();
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.6);
            cr.set_font_size(20.0);
            cr.move_to(8.0, 30.0);
            cr.show_text("Ña✓").unwrap();
            surface.flush();
        }
        assert_eq!(offscreen.0.pixmap.borrow().data(), presented.0.pixmap.borrow().data());
    }
}

