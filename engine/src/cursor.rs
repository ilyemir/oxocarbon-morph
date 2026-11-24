//! Dynamic Oxocarbon cursor theme generator (v1.0).
//!
//! Every `theme set` regenerates the full cursor theme from the active
//! palette. Each shape is defined once as sharp vector geometry in a 24x24
//! space and rendered twice, fully in-process with zero external tools:
//!
//! - Hyprcursor `.hlc` archives (`hyprcursors/<shape>.hlc`, a STORE-method
//!   ZIP containing `meta.hl` + the shape SVG — exactly the distribution
//!   format libhyprcursor loads) for Hyprland's own hardware cursor, and
//! - classic XCursor binaries (`cursors/<shape>`, 24/32/48 px, premultiplied
//!   ARGB little-endian) so GTK, XWayland and X11 apps match exactly.
//!
//! Aliases (pointer, text, size_hor, ...) are declared as hyprcursor
//! `define_override` entries and as symlinks in `cursors/`.

use crate::{hypr, palette::Palette, paths};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Color + geometry model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn parse(s: &str) -> Rgb {
        let h = s.trim_start_matches('#');
        if h.len() == 6 {
            let v = u32::from_str_radix(h, 16).unwrap_or(0xffffff);
            Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
        } else {
            Rgb(0xff, 0xff, 0xff)
        }
    }

    fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone)]
enum Prim {
    /// Filled polygon.
    Poly { pts: Vec<(f64, f64)>, color: Rgb },
    /// Stroked segment (round caps, matches the SVG output).
    Line { a: (f64, f64), b: (f64, f64), w: f64, color: Rgb },
    /// Circle; `w == 0.0` fills it, otherwise strokes the ring.
    Circle { c: (f64, f64), r: f64, w: f64, color: Rgb },
    /// Stroked arc from a0 to a1 radians (y-down screen space).
    Arc { c: (f64, f64), r: f64, a0: f64, a1: f64, w: f64, color: Rgb },
}

impl Prim {
    fn color(&self) -> Rgb {
        match self {
            Prim::Poly { color, .. }
            | Prim::Line { color, .. }
            | Prim::Circle { color, .. }
            | Prim::Arc { color, .. } => *color,
        }
    }
}

/// One cursor shape: geometry, hotspot (in 24x24 space) and aliases.
struct Shape {
    name: &'static str,
    hotspot: (f64, f64),
    prims: Vec<Prim>,
    aliases: &'static [&'static str],
}

// --- geometry helpers -------------------------------------------------------

fn poly(pts: &[(f64, f64)], color: Rgb) -> Prim {
    Prim::Poly { pts: pts.to_vec(), color }
}

fn line(a: (f64, f64), b: (f64, f64), w: f64, color: Rgb) -> Prim {
    Prim::Line { a, b, w, color }
}

/// Filled polygon with an outline stroke around its edge.
fn outlined_poly(pts: &[(f64, f64)], body: Rgb, outline: Rgb, w: f64) -> Vec<Prim> {
    let mut v = vec![poly(pts, body)];
    let n = pts.len();
    for i in 0..n {
        v.push(line(pts[i], pts[(i + 1) % n], w, outline));
    }
    v
}

/// Stroke with a dark backing outline so it reads on any wallpaper.
fn backed_line(a: (f64, f64), b: (f64, f64), w: f64, color: Rgb, body: Rgb) -> Vec<Prim> {
    vec![line(a, b, w + 1.4, body), line(a, b, w, color)]
}

/// Scale + translate a point set (for the mini arrow on left_ptr_watch).
fn mapped(pts: &[(f64, f64)], s: f64, dx: f64, dy: f64) -> Vec<(f64, f64)> {
    pts.iter().map(|(x, y)| (x * s + dx, y * s + dy)).collect()
}

// ---------------------------------------------------------------------------
// Shape catalog — sharp Oxocarbon geometry, palette-driven colors
// ---------------------------------------------------------------------------

fn shapes(pal: &Palette) -> Vec<Shape> {
    let body = Rgb::parse(&pal.bg); // dark fill
    let outline = Rgb::parse(&pal.ink); // light rim
    let acc = Rgb::parse(&pal.accent); // palette accent

    // classic angular arrow polygon (tip top-left)
    let arrow: Vec<(f64, f64)> = vec![
        (2.0, 2.0),
        (2.0, 16.5),
        (6.1, 13.2),
        (8.8, 19.4),
        (11.2, 18.4),
        (8.6, 12.6),
        (13.2, 12.6),
    ];

    let mut v: Vec<Shape> = Vec::with_capacity(11);

    // --- left_ptr ------------------------------------------------------------
    let mut prims = outlined_poly(&arrow, body, outline, 1.5);
    prims.push(line((2.0, 2.6), (2.0, 14.5), 1.5, acc)); // accent left edge
    v.push(Shape {
        name: "left_ptr",
        hotspot: (2.0, 2.0),
        prims,
        aliases: &["default", "arrow", "top_left_arrow"],
    });

    // --- hand2 (pointing hand) ------------------------------------------------
    let finger = [(10.3, 3.5), (13.7, 3.5), (13.7, 14.2), (10.3, 14.2)];
    let palm = [(6.5, 12.6), (17.5, 12.6), (17.5, 19.5), (6.5, 19.5)];
    let mut prims = vec![poly(&finger[..], body), poly(&palm[..], body)];
    for seg in [
        ((10.3, 3.5), (13.7, 3.5)),
        ((13.7, 3.5), (13.7, 14.2)),
        ((13.7, 12.6), (13.7, 19.5)),
        ((10.3, 12.6), (10.3, 19.5)),
        ((6.5, 12.6), (6.5, 19.5)),
        ((17.5, 12.6), (17.5, 19.5)),
        ((6.5, 19.5), (17.5, 19.5)),
        ((6.5, 12.6), (10.3, 12.6)),
        ((13.7, 12.6), (17.5, 12.6)),
    ] {
        prims.push(line(seg.0, seg.1, 1.4, outline));
    }
    prims.push(line((10.3, 3.5), (13.7, 3.5), 1.6, acc)); // accent fingertip
    v.push(Shape {
        name: "hand2",
        hotspot: (12.0, 3.5),
        prims,
        aliases: &["pointer", "hand"],
    });

    // --- xterm (I-beam) --------------------------------------------------------
    let stem = [(10.9, 4.8), (13.1, 4.8), (13.1, 19.2), (10.9, 19.2)];
    let top = [(8.0, 3.6), (16.0, 3.6), (16.0, 5.4), (8.0, 5.4)];
    let bot = [(8.0, 18.6), (16.0, 18.6), (16.0, 20.4), (8.0, 20.4)];
    let mut prims = vec![
        poly(&stem[..], body),
        poly(&top[..], body),
        poly(&bot[..], body),
    ];
    prims.extend(outlined_poly(&stem, body, outline, 1.3));
    prims.extend(outlined_poly(&top, body, outline, 1.3));
    prims.extend(outlined_poly(&bot, body, outline, 1.3));
    prims.push(line((12.0, 6.4), (12.0, 17.6), 1.3, acc));
    v.push(Shape {
        name: "xterm",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["text"],
    });

    // --- crosshair --------------------------------------------------------------
    let mut prims = Vec::new();
    for seg in [
        ((12.0, 3.2), (12.0, 9.0)),
        ((12.0, 15.0), (12.0, 20.8)),
        ((3.2, 12.0), (9.0, 12.0)),
        ((15.0, 12.0), (20.8, 12.0)),
    ] {
        prims.extend(backed_line(seg.0, seg.1, 1.4, outline, body));
    }
    prims.push(poly(&[(11.0, 11.0), (13.0, 11.0), (13.0, 13.0), (11.0, 13.0)], acc));
    v.push(Shape {
        name: "crosshair",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["cross"],
    });

    // --- grabbing (fist) ---------------------------------------------------------
    let fist = [(6.0, 7.8), (18.0, 7.8), (18.0, 19.5), (6.0, 19.5)];
    let mut prims = outlined_poly(&fist, body, outline, 1.5);
    for x in [10.0f64, 14.0f64] {
        prims.push(line((x, 11.2), (x, 19.5), 1.2, outline));
    }
    prims.push(line((6.8, 7.8), (17.2, 7.8), 1.6, acc));
    v.push(Shape {
        name: "grabbing",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["closedhand", "all-scroll", "grab"],
    });

    // --- sb_h_double_arrow ( <-> ) ------------------------------------------------
    let lh = [(3.5, 12.0), (9.5, 7.8), (9.5, 16.2)];
    let rh = [(20.5, 12.0), (14.5, 7.8), (14.5, 16.2)];
    let mut prims = outlined_poly(&lh, body, outline, 1.4);
    prims.extend(outlined_poly(&rh, body, outline, 1.4));
    prims.extend(backed_line((9.5, 12.0), (14.5, 12.0), 1.8, acc, body));
    v.push(Shape {
        name: "sb_h_double_arrow",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["size_hor", "ew-resize", "h_double_arrow", "col-resize", "w-resize", "e-resize"],
    });

    // --- sb_v_double_arrow ( ↕ ) ---------------------------------------------------
    let th = [(12.0, 3.5), (7.8, 9.5), (16.2, 9.5)];
    let bh = [(12.0, 20.5), (7.8, 14.5), (16.2, 14.5)];
    let mut prims = outlined_poly(&th, body, outline, 1.4);
    prims.extend(outlined_poly(&bh, body, outline, 1.4));
    prims.extend(backed_line((12.0, 9.5), (12.0, 14.5), 1.8, acc, body));
    v.push(Shape {
        name: "sb_v_double_arrow",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["size_ver", "ns-resize", "v_double_arrow", "row-resize", "n-resize", "s-resize"],
    });

    // --- bd_double_arrow ( ⤡ nw-se ) -------------------------------------------------
    let nw = [(4.0, 4.0), (11.0, 6.5), (6.5, 11.0)];
    let se = [(20.0, 20.0), (13.0, 17.5), (17.5, 13.0)];
    let mut prims = outlined_poly(&nw, body, outline, 1.4);
    prims.extend(outlined_poly(&se, body, outline, 1.4));
    prims.extend(backed_line((8.2, 8.2), (15.8, 15.8), 1.8, acc, body));
    v.push(Shape {
        name: "bd_double_arrow",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["size_fdiag", "nesw-resize", "nw-resize", "se-resize"],
    });

    // --- fd_double_arrow ( ⤢ ne-sw ) --------------------------------------------------
    let ne = [(20.0, 4.0), (13.0, 6.5), (17.5, 11.0)];
    let sw = [(4.0, 20.0), (11.0, 17.5), (6.5, 13.0)];
    let mut prims = outlined_poly(&ne, body, outline, 1.4);
    prims.extend(outlined_poly(&sw, body, outline, 1.4));
    prims.extend(backed_line((8.2, 15.8), (15.8, 8.2), 1.8, acc, body));
    v.push(Shape {
        name: "fd_double_arrow",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["size_bdiag", "nwse-resize", "ne-resize", "sw-resize"],
    });

    // --- watch (hourglass) --------------------------------------------------------------
    let top = [(6.5, 4.5), (17.5, 4.5), (12.0, 11.0)];
    let bot = [(6.5, 19.5), (17.5, 19.5), (12.0, 13.0)];
    let mut prims = outlined_poly(&top, body, outline, 1.3);
    prims.extend(outlined_poly(&bot, body, outline, 1.3));
    prims.extend(backed_line((12.0, 10.6), (12.0, 13.4), 1.4, acc, body));
    v.push(Shape {
        name: "watch",
        hotspot: (12.0, 12.0),
        prims,
        aliases: &["wait"],
    });

    // --- left_ptr_watch (mini arrow + spinner) --------------------------------------------
    let mini = mapped(&arrow, 0.55, 1.0, 1.0);
    let mut prims = outlined_poly(&mini, body, outline, 1.2);
    prims.push(line((1.0, 1.4), (1.0, 9.6), 1.2, acc));
    prims.push(Prim::Circle { c: (16.5, 16.5), r: 5.2, w: 2.4, color: body });
    prims.push(Prim::Circle { c: (16.5, 16.5), r: 5.2, w: 1.4, color: outline });
    prims.push(Prim::Arc { c: (16.5, 16.5), r: 5.2, a0: -1.5708, a1: 0.0, w: 1.4, color: acc });
    v.push(Shape {
        name: "left_ptr_watch",
        hotspot: (2.0, 2.0),
        prims,
        aliases: &["progress"],
    });

    v
}

// ---------------------------------------------------------------------------
// Rasterizer (supersampled coverage -> premultiplied ARGB)
// ---------------------------------------------------------------------------

const SS: usize = 4; // 4x4 subsamples per pixel

fn point_in_poly(x: f64, y: f64, pts: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist2_seg(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        let ex = px - ax;
        let ey = py - ay;
        return ex * ex + ey * ey;
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let ex = px - (ax + t * dx);
    let ey = py - (ay + t * dy);
    ex * ex + ey * ey
}

fn norm_angle(a: f64) -> f64 {
    let mut a = a % (std::f64::consts::TAU);
    if a < 0.0 {
        a += std::f64::consts::TAU;
    }
    a
}

fn prim_covers(p: &Prim, x: f64, y: f64) -> bool {
    match p {
        Prim::Poly { pts, .. } => point_in_poly(x, y, pts),
        Prim::Line { a, b, w, .. } => {
            let half = w / 2.0;
            dist2_seg(x, y, a.0, a.1, b.0, b.1) <= half * half
        }
        Prim::Circle { c, r, w, .. } => {
            let d2 = (x - c.0) * (x - c.0) + (y - c.1) * (y - c.1);
            if *w <= 0.0 {
                d2 <= r * r
            } else {
                let ro = r + w / 2.0;
                let ri = (r - w / 2.0).max(0.0);
                d2 <= ro * ro && d2 >= ri * ri
            }
        }
        Prim::Arc { c, r, a0, a1, w, .. } => {
            let half = w / 2.0;
            let d2 = (x - c.0) * (x - c.0) + (y - c.1) * (y - c.1);
            if d2 > (r + half) * (r + half) || d2 < (r - half).max(0.0).powi(2) {
                return false;
            }
            let ang = norm_angle((y - c.1).atan2(x - c.0));
            let (s, e) = (norm_angle(*a0), norm_angle(*a1));
            if s <= e {
                ang >= s && ang <= e
            } else {
                ang >= s || ang <= e
            }
        }
    }
}

/// Bounding box of a prim in 24x24 space (expanded by its stroke).
fn prim_bbox(p: &Prim) -> (f64, f64, f64, f64) {
    let inf = f64::INFINITY;
    let (mut x0, mut y0, mut x1, mut y1) = (inf, inf, -inf, -inf);
    let mut eat = |x: f64, y: f64, pad: f64| {
        x0 = x0.min(x - pad);
        y0 = y0.min(y - pad);
        x1 = x1.max(x + pad);
        y1 = y1.max(y + pad);
    };
    match p {
        Prim::Poly { pts, .. } => for &(x, y) in pts {
            eat(x, y, 0.5)
        },
        Prim::Line { a, b, w, .. } => {
            eat(a.0, a.1, w / 2.0 + 0.5);
            eat(b.0, b.1, w / 2.0 + 0.5);
        }
        Prim::Circle { c, r, w, .. } => eat(c.0, c.1, r + w / 2.0 + 0.5),
        Prim::Arc { c, r, w, .. } => eat(c.0, c.1, r + w / 2.0 + 0.5),
    }
    (x0, y0, x1, y1)
}

/// Rasterize `prims` into a `px`-sized premultiplied ARGB image (LE words).
fn rasterize(prims: &[Prim], px: usize) -> Vec<u32> {
    let scale = px as f64 / 24.0;
    let n = px * px;
    let mut buf = vec![[0f32; 4]; n]; // premultiplied (r,g,b,a)

    for prim in prims {
        let (bx0, by0, bx1, by1) = prim_bbox(prim);
        let x0 = (bx0 * scale).floor().clamp(0.0, px as f64 - 1.0) as usize;
        let y0 = (by0 * scale).floor().clamp(0.0, px as f64 - 1.0) as usize;
        let x1 = (bx1 * scale).ceil().clamp(0.0, px as f64 - 1.0) as usize;
        let y1 = (by1 * scale).ceil().clamp(0.0, px as f64 - 1.0) as usize;

        let (cr, cg, cb) = {
            let c = prim.color();
            (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0)
        };

        for py in y0..=y1 {
            for px_ in x0..=x1 {
                let mut hits = 0u32;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let u = (px_ as f64 + (sx as f64 + 0.5) / SS as f64) / scale;
                        let v = (py as f64 + (sy as f64 + 0.5) / SS as f64) / scale;
                        if prim_covers(prim, u, v) {
                            hits += 1;
                        }
                    }
                }
                if hits == 0 {
                    continue;
                }
                let sa = hits as f32 / (SS * SS) as f32;
                let dst = &mut buf[py * px + px_];
                let keep = 1.0 - sa;
                dst[3] = sa + dst[3] * keep;
                dst[0] = cr * sa + dst[0] * keep;
                dst[1] = cg * sa + dst[1] * keep;
                dst[2] = cb * sa + dst[2] * keep;
            }
        }
    }

    buf.iter()
        .map(|c| {
            let a = (c[3].clamp(0.0, 1.0) * 255.0).round() as u32;
            let r = ((c[0].clamp(0.0, c[3].max(0.0))) * 255.0).round().min(a as f32) as u32;
            let g = ((c[1].clamp(0.0, c[3].max(0.0))) * 255.0).round().min(a as f32) as u32;
            let b = ((c[2].clamp(0.0, c[3].max(0.0))) * 255.0).round().min(a as f32) as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Emitters: SVG + XCursor binary
// ---------------------------------------------------------------------------

fn fmt(x: f64) -> String {
    format!("{:.2}", x)
}

fn prim_svg(p: &Prim) -> String {
    match p {
        Prim::Poly { pts, color } => {
            let d: Vec<String> = pts
                .iter()
                .enumerate()
                .map(|(i, (x, y))| {
                    format!("{} {} {}", if i == 0 { 'M' } else { 'L' }, fmt(*x), fmt(*y))
                })
                .collect();
            format!(
                "<path d=\"{} Z\" fill=\"{}\"/>\n",
                d.join(" "),
                color.css()
            )
        }
        Prim::Line { a, b, w, color } => format!(
            "<path d=\"M {} {} L {} {}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>\n",
            fmt(a.0), fmt(a.1), fmt(b.0), fmt(b.1),
            color.css(), fmt(*w)
        ),
        Prim::Circle { c, r, w, color } => {
            if *w <= 0.0 {
                format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>\n",
                    fmt(c.0), fmt(c.1), fmt(*r), color.css()
                )
            } else {
                format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                    fmt(c.0), fmt(c.1), fmt(*r), color.css(), fmt(*w)
                )
            }
        }
        Prim::Arc { c, r, a0, a1, w, color } => {
            let p0 = (c.0 + r * a0.cos(), c.1 + r * a0.sin());
            let p1 = (c.0 + r * a1.cos(), c.1 + r * a1.sin());
            let large = if (a1 - a0).abs() > std::f64::consts::PI { 1 } else { 0 };
            format!(
                "<path d=\"M {} {} A {} {} 0 {} 1 {} {}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>\n",
                fmt(p0.0), fmt(p0.1), fmt(*r), fmt(*r), large, fmt(p1.0), fmt(p1.1),
                color.css(), fmt(*w)
            )
        }
    }
}

fn shape_svg(shape: &Shape) -> String {
    let body: String = shape.prims.iter().map(prim_svg).collect();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\n{}</svg>\n",
        body
    )
}

/// XCursor binary sizes written for every shape.
const XCURSOR_SIZES: [u32; 3] = [24, 32, 48];

/// Write one XCursor file: header + TOC + one image chunk per size.
fn write_xcursor(path: &Path, shape: &Shape) -> Result<(), String> {
    let frames: Vec<(u32, u32, u32, Vec<u32>)> = XCURSOR_SIZES
        .iter()
        .map(|&s| {
            let px = s as usize;
            let pixels = rasterize(&shape.prims, px);
            let xhot = (shape.hotspot.0 * px as f64 / 24.0).round() as u32;
            let yhot = (shape.hotspot.1 * px as f64 / 24.0).round() as u32;
            (s, xhot, yhot, pixels)
        })
        .collect();

    let n = frames.len() as u32;
    let mut toc: Vec<(u32, u32, u32)> = Vec::with_capacity(frames.len());
    let mut pos = 16 + 12 * n;
    for (s, _, _, _) in &frames {
        let sz = *s as u32;
        toc.push((0xfffd_0002, sz, pos));
        pos += 36 + (sz * sz * 4);
    }

    let mut bytes: Vec<u8> = Vec::with_capacity(pos as usize);
    let w32 = |v: u32, bytes: &mut Vec<u8>| bytes.extend_from_slice(&v.to_le_bytes());
    w32(0x7275_6358, &mut bytes); // "Xcur"
    w32(16, &mut bytes); // file header size
    w32(65_536, &mut bytes); // file version
    w32(n, &mut bytes);
    for (t, s, p) in &toc {
        w32(*t, &mut bytes);
        w32(*s, &mut bytes);
        w32(*p, &mut bytes);
    }
    for (s, xhot, yhot, pixels) in &frames {
        let sz = *s as u32;
        w32(36, &mut bytes); // chunk header size
        w32(0xfffd_0002, &mut bytes);
        w32(sz, &mut bytes); // nominal size
        w32(1, &mut bytes); // chunk version
        w32(sz, &mut bytes); // width
        w32(sz, &mut bytes); // height
        w32(*xhot, &mut bytes);
        w32(*yhot, &mut bytes);
        w32(50, &mut bytes); // delay (ms; irrelevant for single frames)
        for p in pixels {
            w32(*p, &mut bytes);
        }
    }

    let tmp = path.with_extension("tmp");
    {
        let mut f = File::create(&tmp).map_err(|e| format!("{}: {}", path.display(), e))?;
        f.write_all(&bytes).map_err(|e| format!("{}: {}", path.display(), e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("{}: {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// Theme assembly
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Minimal ZIP writer (STORE method) for .hlc archives
// ---------------------------------------------------------------------------

/// Standard CRC-32 (IEEE 802.3 / zlib, reflected, poly 0xEDB88320).
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Pack `entries` (name -> bytes) into a STORE-method ZIP archive.
/// This is the format libhyprcursor loads as `.hlc`; uncompressed on
/// purpose: the payloads are tiny SVGs and libzip reads it natively.
fn write_zip(entries: &[( &str, &[u8] )]) -> Vec<u8> {
    struct Entry {
        name: Vec<u8>,
        data: Vec<u8>,
        crc: u32,
        offset: u32,
    }

    let mut out: Vec<u8> = Vec::new();
    let mut list: Vec<Entry> = Vec::with_capacity(entries.len());

    // fixed DOS timestamp: 2024-01-01 00:00
    let dos_time: u16 = 0;
    let dos_date: u16 = ((2024 - 1980) << 9) | (1 << 5) | 1;

    for (name, data) in entries {
        let offset = out.len() as u32;
        let name_bytes = name.as_bytes().to_vec();
        let crc = crc32(data);
        let sz = data.len() as u32;
        let nl = name_bytes.len() as u16;

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // local header
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: STORE
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&sz.to_le_bytes()); // compressed size
        out.extend_from_slice(&sz.to_le_bytes()); // uncompressed size
        out.extend_from_slice(&nl.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&name_bytes);
        out.extend_from_slice(data);

        list.push(Entry { name: name_bytes, data: data.to_vec(), crc, offset });
    }

    let cd_offset = out.len() as u32;
    for e in &list {
        let sz = e.data.len() as u32;
        let nl = e.name.len() as u16;
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // central dir
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: STORE
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&sz.to_le_bytes());
        out.extend_from_slice(&sz.to_le_bytes());
        out.extend_from_slice(&nl.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&0u16.to_le_bytes()); // comment len
        out.extend_from_slice(&0u16.to_le_bytes()); // disk start
        out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        out.extend_from_slice(&((0o100_644u32) << 16).to_le_bytes()); // external attrs
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(&e.name);
    }
    let cd_size = out.len() as u32 - cd_offset;

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes()); // EOCD
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&(list.len() as u16).to_le_bytes()); // entries this disk
    out.extend_from_slice(&(list.len() as u16).to_le_bytes()); // total entries
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len

    out
}

fn manifest_toml() -> String {
    "[General]\n\
     cursors_directory = \"hyprcursors\"\n\
     name = \"oxocarbon-cursor\"\n\
     description = \"Palette-driven Oxocarbon cursors (generated by oxocarbon-morph)\"\n\
     version = \"1.0\"\n\
     author = \"oxocarbon-morph\"\n"
        .to_string()
}

fn meta_hl(shape: &Shape) -> String {
    let mut s = String::new();
    s.push_str("# generated by oxocarbon-morph :: cursor engine\n");
    s.push_str("resize_algorithm = none\n");
    s.push_str(&format!("hotspot_x = {:.4}\n", shape.hotspot.0 / 24.0));
    s.push_str(&format!("hotspot_y = {:.4}\n", shape.hotspot.1 / 24.0));
    s.push_str("nominal_size = 1.0\n");
    s.push_str(&format!("define_size = 0, {}.svg\n", shape.name));
    if !shape.aliases.is_empty() {
        s.push_str(&format!(
            "define_override = {}\n",
            shape.aliases.join(";")
        ));
    }
    s
}

fn index_theme() -> String {
    "[Icon Theme]\n\
     Name=oxocarbon-cursor\n\
     Comment=Palette-driven Oxocarbon cursors (generated)\n\
     Inherits=DMZ-White\n"
        .to_string()
}

/// Regenerate the whole cursor theme with `pal` and apply it live.
/// Returns the number of generated shapes.
pub fn sync(pal: &Palette) -> Result<usize, String> {
    let shapes = shapes(pal);
    let dir = paths::cursor_theme_dir();

    let hypr_dir = dir.join("hyprcursors");
    let xc_dir = dir.join("cursors");
    let _ = fs::remove_dir_all(&hypr_dir);
    let _ = fs::remove_dir_all(&xc_dir);
    fs::create_dir_all(&hypr_dir).map_err(|e| format!("cursor theme: {}", e))?;
    fs::create_dir_all(&xc_dir).map_err(|e| format!("cursor theme: {}", e))?;

    paths::atomic_write(&dir.join("manifest.toml"), &manifest_toml())
        .map_err(|e| format!("manifest.toml: {}", e))?;
    paths::atomic_write(&dir.join("index.theme"), &index_theme())
        .map_err(|e| format!("index.theme: {}", e))?;

    for shape in &shapes {
        // libhyprcursor only loads regular files from cursors_directory and
        // opens them as ZIPs: emit the distribution `.hlc` format directly.
        let svg = shape_svg(shape);
        let meta = meta_hl(shape);
        let svg_name = format!("{}.svg", shape.name);
        let zip = write_zip(&[
            ("meta.hl", meta.as_bytes()),
            (svg_name.as_str(), svg.as_bytes()),
        ]);
        paths::atomic_write_bytes(&hypr_dir.join(format!("{}.hlc", shape.name)), &zip)
            .map_err(|e| format!("{}: {}", shape.name, e))?;
        write_xcursor(&xc_dir.join(shape.name), shape)?;
        for alias in shape.aliases {
            let link = xc_dir.join(alias);
            let _ = fs::remove_file(&link);
            let _ = std::os::unix::fs::symlink(shape.name, &link);
        }
    }

    if hypr::running() {
        let _ = Command::new("hyprctl")
            .args(["setcursor", "oxocarbon-cursor", "24"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    Ok(shapes.len())
}
