//! Pictures the calculator shows while it is off: tonight's Moon, the
//! agenda from tock and a photo, each a 400×240 black-and-white BMP in
//! the calculator's OFFIMG folder. The calculator shows the next one each
//! time it is turned off. ~/.dm41/off names the pictures to make, one a
//! line; without that file dm41 makes none:
//!
//!   moon 51.48 0.00       the Moon, with rise and set at that latitude and longitude
//!   agenda                the next days from `tock --agenda`
//!   photo ~/Pictures/x.jpg

use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const W: usize = 400;
const H: usize = 240;
const SYNODIC: f64 = 29.530_588_853;
/// The near side, 512×512 8-bit gray, north up, from NASA's LRO mosaic
/// (the same map as the moon app's).
const MOON: &[u8] = include_bytes!("../img/moon-512.gray");
const MOON_N: usize = 512;
/// Map value that counts as white, the map value that turns black, and
/// how sharp the line between day and night is. The night side stays
/// black: faint earthshine dithers into streaks.
const MOON_WHITE: f64 = 0.72;
const MOON_SEA: f64 = 0.4;
const RAMP: f64 = 5.0;
const KINDS: [&str; 3] = ["moon", "agenda", "photo"];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// One picture, a pixel a byte: true is ink (dark), false is paper.
pub struct Pic {
    ink: Vec<bool>,
}

impl Pic {
    /// Grey 0 (dark) to 1 (light) to ink and paper by error diffusion.
    /// Atkinson's way passes on only six eighths of the error, which keeps
    /// the Moon crisp; Floyd and Steinberg's passes on all of it, which
    /// keeps the middle greys of a photo.
    fn dither(mut grey: Vec<f64>, photo: bool) -> Pic {
        const ATKINSON: [(i64, i64, f64); 6] = [(1, 0, 0.125), (2, 0, 0.125), (-1, 1, 0.125), (0, 1, 0.125), (1, 1, 0.125), (0, 2, 0.125)];
        const FLOYD: [(i64, i64, f64); 4] = [(1, 0, 7.0 / 16.0), (-1, 1, 3.0 / 16.0), (0, 1, 5.0 / 16.0), (1, 1, 1.0 / 16.0)];
        let spread: &[(i64, i64, f64)] = if photo { &FLOYD } else { &ATKINSON };
        let mut ink = vec![false; W * H];
        for y in 0..H {
            for x in 0..W {
                let v = grey[y * W + x];
                let dark = v < 0.5;
                ink[y * W + x] = dark;
                let e = v - if dark { 0.0 } else { 1.0 };
                for &(dx, dy, k) in spread {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if (0..W as i64).contains(&nx) && (ny as usize) < H {
                        grey[ny as usize * W + nx as usize] += e * k;
                    }
                }
            }
        }
        Pic { ink }
    }

    fn paper() -> Pic {
        Pic { ink: vec![false; W * H] }
    }

    fn fill(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, ink: bool) {
        for y in y0..y1.min(H) {
            for x in x0..x1.min(W) {
                self.ink[y * W + x] = ink;
            }
        }
    }

    /// The BMP the calculator reads: one bit a pixel, rows from the bottom,
    /// with the same header as the pictures it comes with.
    pub fn bmp(&self) -> Vec<u8> {
        let stride = W.div_ceil(32) * 4;
        let size = stride * H;
        let mut b = Vec::with_capacity(130 + size);
        let u32le = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(b"BM");
        u32le(&mut b, (130 + size) as u32);
        u32le(&mut b, 0);
        u32le(&mut b, 130);
        u32le(&mut b, 108);
        u32le(&mut b, W as u32);
        u32le(&mut b, H as u32);
        b.extend_from_slice(&[1, 0, 1, 0]);
        u32le(&mut b, 0);
        u32le(&mut b, size as u32);
        u32le(&mut b, 5906);
        u32le(&mut b, 5906);
        u32le(&mut b, 2);
        u32le(&mut b, 2);
        b.extend_from_slice(b"BGRs");
        b.resize(14 + 108, 0);
        b.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
        for y in (0..H).rev() {
            let mut row = vec![0u8; stride];
            for x in 0..W {
                if !self.ink[y * W + x] {
                    row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            b.extend_from_slice(&row);
        }
        b
    }
}

/// The fonts, narrow ones first since the screen is small.
struct Fonts {
    regular: Option<FontVec>,
    bold: Option<FontVec>,
}

impl Fonts {
    fn load() -> Fonts {
        let first = |names: &[&str]| names.iter().find_map(|p| std::fs::read(p).ok().and_then(|b| FontVec::try_from_vec(b).ok()));
        Fonts {
            regular: first(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf",
                "/usr/share/fonts/TTF/DejaVuSansCondensed.ttf",
                "/usr/share/fonts/dejavu/DejaVuSansCondensed.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/System/Library/Fonts/Supplemental/Arial Narrow.ttf",
                "/System/Library/Fonts/Supplemental/Arial.ttf",
            ]),
            bold: first(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSansCondensed-Bold.ttf",
                "/usr/share/fonts/dejavu/DejaVuSansCondensed-Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                "/System/Library/Fonts/Supplemental/Arial Narrow Bold.ttf",
                "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            ]),
        }
    }

    fn face(&self, bold: bool) -> Option<&FontVec> {
        if bold {
            self.bold.as_ref().or(self.regular.as_ref())
        } else {
            self.regular.as_ref()
        }
    }

    fn width(&self, s: &str, size: f32, bold: bool) -> f32 {
        let Some(font) = self.face(bold) else { return 0.0 };
        let sf = font.as_scaled(PxScale::from(size));
        s.chars().map(|c| sf.h_advance(font.glyph_id(c))).sum()
    }

    /// `s` cut with an ellipsis to fit `max` pixels.
    fn fit(&self, s: &str, size: f32, bold: bool, max: f32) -> String {
        if self.width(s, size, bold) <= max {
            return s.to_string();
        }
        let mut t: String = s.to_string();
        while !t.is_empty() && self.width(&format!("{t}…"), size, bold) > max {
            t.pop();
        }
        format!("{}…", t.trim_end())
    }

    /// Write `s` with its top at (`x`, `top`), in ink or in paper.
    #[allow(clippy::too_many_arguments)]
    fn write(&self, p: &mut Pic, s: &str, x: f32, top: f32, size: f32, bold: bool, ink: bool) {
        let Some(font) = self.face(bold) else { return };
        let scale = PxScale::from(size);
        let sf = font.as_scaled(scale);
        let base = top + sf.ascent();
        let mut pen = x;
        for c in s.chars() {
            let id = font.glyph_id(c);
            if let Some(g) = font.outline_glyph(id.with_scale_and_position(scale, point(pen, base))) {
                let b = g.px_bounds();
                g.draw(|gx, gy, v| {
                    let (px, py) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                    if v > 0.45 && (0..W as i64).contains(&px) && (0..H as i64).contains(&py) {
                        p.ink[py as usize * W + px as usize] = ink;
                    }
                });
            }
            pen += sf.h_advance(id);
        }
    }
}

/// The moment the pictures are made, local and UT.
pub struct Now {
    y: i32,
    m: u32,
    d: u32,
    hour: u32,
    min: u32,
    wday: usize,
    /// Hours east of UT, summer time included.
    tz: f64,
    /// The UT date and hour, for the Moon's phase.
    ut: (i32, u32, u32, f64),
}

impl Now {
    pub fn get() -> Now {
        let t = unsafe { libc::time(std::ptr::null_mut()) };
        // SAFETY: localtime_r and gmtime_r only write the tm we hand them.
        let mut lt: libc::tm = unsafe { std::mem::zeroed() };
        let mut ut: libc::tm = unsafe { std::mem::zeroed() };
        unsafe {
            libc::localtime_r(&t, &mut lt);
            libc::gmtime_r(&t, &mut ut);
        }
        Now {
            y: lt.tm_year + 1900,
            m: lt.tm_mon as u32 + 1,
            d: lt.tm_mday as u32,
            hour: lt.tm_hour as u32,
            min: lt.tm_min as u32,
            wday: lt.tm_wday as usize,
            tz: lt.tm_gmtoff as f64 / 3600.0,
            ut: (ut.tm_year + 1900, ut.tm_mon as u32 + 1, ut.tm_mday as u32, ut.tm_hour as f64 + ut.tm_min as f64 / 60.0),
        }
    }

    fn short(&self) -> String {
        format!("{} {} {}", &DAYS[self.wday][..3], self.d, &MONTHS[self.m as usize - 1][..3])
    }
}

/// Map brightness (0..1) at disk point (x, y), y up.
fn moon_map(x: f64, y: f64) -> f64 {
    let n = MOON_N as f64;
    let u = ((x + 1.0) / 2.0 * n - 0.5).clamp(0.0, n - 1.0) as usize;
    let v = ((1.0 - y) / 2.0 * n - 0.5).clamp(0.0, n - 1.0) as usize;
    let a = MOON[v * MOON_N + u] as f64 / 255.0 / MOON_WHITE;
    // Spread the seas and the highlands apart: in ink and paper alone a
    // map this even would show as one grey.
    ((a - MOON_SEA) / (1.0 - MOON_SEA)).clamp(0.06, 1.0)
}

fn phase_name(f: f64) -> &'static str {
    let near = |c: f64| {
        let d = (f - c).abs();
        !(0.017..=0.983).contains(&d)
    };
    match f {
        _ if near(0.0) => "New moon",
        _ if near(0.25) => "First quarter",
        _ if near(0.5) => "Full moon",
        _ if near(0.75) => "Last quarter",
        _ if f < 0.25 => "Waxing crescent",
        _ if f < 0.5 => "Waxing gibbous",
        _ if f < 0.75 => "Waning gibbous",
        _ => "Waning crescent",
    }
}

/// "full in 11 days", "full tomorrow" or "full today".
fn until(f: f64, target: f64, what: &str) -> String {
    let d = (target - f).rem_euclid(1.0) * SYNODIC;
    match d {
        _ if !(0.5..=SYNODIC - 0.5).contains(&d) => format!("{what} today"),
        _ if d < 1.5 => format!("{what} tomorrow"),
        _ => format!("{what} in {d:.0} days"),
    }
}

/// The Moon as it is now on a black sky, with its phase beside it, and
/// when it rises and sets where `at` is.
fn moon(now: &Now, at: Option<(f64, f64)>, fonts: &Fonts) -> Pic {
    let (uy, um, ud, uh) = now.ut;
    let f = (orbit::moon_phase(uy, um, ud).phase + uh / 24.0 / SYNODIC).rem_euclid(1.0);
    let south = at.is_some_and(|(lat, _)| lat < 0.0);
    let sun = ((f * 2.0 * PI).sin(), -(f * 2.0 * PI).cos());
    let (cx, cy, r) = (122.0, 120.0, 110.0);
    let mut grey = vec![0.0; W * H];
    for py in 0..H {
        for px in 0..W {
            let (mut x, mut y) = ((px as f64 + 0.5 - cx) / r, (cy - py as f64 - 0.5) / r);
            if south {
                (x, y) = (-x, -y);
            }
            let rr = x.hypot(y);
            if rr > 1.0 {
                continue;
            }
            let z = (1.0 - rr * rr).sqrt();
            let lit = ((x * sun.0 + z * sun.1) * RAMP).clamp(0.0, 1.0);
            grey[py * W + px] = moon_map(x, y) * lit;
        }
    }
    let mut p = Pic::dither(grey, false);
    let x = 250.0;
    fonts.write(&mut p, &now.short(), x, 14.0, 20.0, true, false);
    let mut y = 50.0;
    for word in phase_name(f).split(' ') {
        fonts.write(&mut p, &capital(word), x, y, 26.0, true, false);
        y += 29.0;
    }
    y += 10.0;
    let lit = ((1.0 - (f * 2.0 * PI).cos()) / 2.0 * 100.0).round();
    let next = if f < 0.5 { until(f, 0.5, "full") } else { until(f, 1.0, "new") };
    let mut lines = vec![format!("{lit:.0}% lit"), next];
    if let Some((up, down)) = at.and_then(|(lat, lon)| orbit::moon_times(now.y, now.m, now.d, lat, lon, now.tz)) {
        lines.push(format!("up {up}"));
        lines.push(format!("down {down}"));
    }
    for l in lines {
        fonts.write(&mut p, &l, x, y, 17.0, false, false);
        y += 22.0;
    }
    p
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// One line of `tock --agenda`: the date, the start ("09:00" or "all
/// day") and the title, without the place and the calendar.
fn agenda_line(l: &str) -> Option<((i32, u32, u32), String, String)> {
    let date = l.get(..10)?;
    let ymd = (date.get(..4)?.parse().ok()?, date.get(5..7)?.parse().ok()?, date.get(8..10)?.parse().ok()?);
    let rest = l.get(15..)?;
    let (when, title) = if let Some(t) = rest.strip_prefix("all day") {
        ("all day".to_string(), t)
    } else {
        (rest.get(..5)?.to_string(), rest.get(11..)?)
    };
    let title = title.rsplit_once("  (").map_or(title, |(t, _)| t);
    let title = title.split_once("  @ ").map_or(title, |(t, _)| t);
    Some((ymd, when, title.trim().to_string()))
}

/// Day of the week, 0 Sunday, for a date in the Gregorian calendar.
fn weekday(y: i32, m: u32, d: u32) -> usize {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[m as usize - 1] + d as i32).rem_euclid(7)) as usize
}

/// The coming events, a heading a day, black on light, under a bar that
/// says when it was made. An event that began before today goes under
/// today.
fn agenda(now: &Now, tock: &str, fonts: &Fonts) -> Pic {
    let mut p = Pic::paper();
    p.fill(0, 0, W, 22, true);
    fonts.write(&mut p, "Agenda", 5.0, 2.0, 16.0, true, false);
    let made = format!("{} {:02}:{:02}", now.short(), now.hour, now.min);
    let mw = fonts.width(&made, 16.0, false);
    fonts.write(&mut p, &made, W as f32 - 5.0 - mw, 2.0, 16.0, false, false);
    let today = (now.y, now.m, now.d);
    let mut y = 27.0;
    let mut day = None;
    let mut any = false;
    for (ymd, when, title) in tock.lines().filter_map(agenda_line) {
        let ymd = ymd.max(today);
        if day != Some(ymd) {
            if y + 18.0 + 19.0 > H as f32 {
                break;
            }
            let head = format!("{} {} {}", DAYS[weekday(ymd.0, ymd.1, ymd.2)], ymd.2, MONTHS[ymd.1 as usize - 1]);
            fonts.write(&mut p, &head, 5.0, y, 16.0, true, true);
            y += 20.0;
            day = Some(ymd);
        }
        if y + 18.0 > H as f32 {
            break;
        }
        fonts.write(&mut p, &when, 10.0, y, 15.0, false, true);
        fonts.write(&mut p, &fonts.fit(&title, 15.0, false, W as f32 - 70.0), 62.0, y, 15.0, false, true);
        y += 18.0;
        any = true;
    }
    if !any {
        fonts.write(&mut p, "Nothing on the calendar", 5.0, y, 16.0, false, true);
    }
    p
}

/// A photo cut to fill the screen, its greys spread from black to white
/// and the middle ones lifted, since dots read darker than a grey.
pub fn photo(path: &Path) -> Result<Pic, String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?.to_luma8();
    let (w, h) = (img.width() as f64, img.height() as f64);
    let k = (W as f64 / w).max(H as f64 / h);
    let (cw, ch) = ((W as f64 / k).round() as u32, (H as f64 / k).round() as u32);
    let crop = image::imageops::crop_imm(&img, (img.width() - cw.min(img.width())) / 2, (img.height() - ch.min(img.height())) / 2, cw, ch);
    let small = image::imageops::resize(&crop.to_image(), W as u32, H as u32, image::imageops::FilterType::Triangle);
    let mut sorted: Vec<u8> = small.as_raw().clone();
    sorted.sort_unstable();
    let (lo, hi) = (sorted[sorted.len() / 100] as f64, sorted[sorted.len() * 99 / 100] as f64);
    let span = (hi - lo).max(1.0);
    Ok(Pic::dither(small.as_raw().iter().map(|&v| ((v as f64 - lo) / span).clamp(0.0, 1.0).powf(0.75)).collect(), true))
}

/// The pictures ~/.dm41/off asks for, by file name, and a note for each
/// one that could not be made.
pub fn make(cfg: &str, now: &Now) -> (Vec<(String, Vec<u8>)>, Vec<String>) {
    let fonts = Fonts::load();
    let (mut pics, mut notes) = (vec![], vec![]);
    for line in cfg.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (kind, arg) = line.split_once(char::is_whitespace).map_or((line, ""), |(k, a)| (k, a.trim()));
        let pic = match kind {
            "moon" => {
                let n: Vec<f64> = arg.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                Ok(moon(now, (n.len() == 2).then(|| (n[0], n[1])), &fonts))
            }
            "agenda" => match std::process::Command::new("tock").args(["--agenda", "7"]).output() {
                Ok(o) => Ok(agenda(now, &String::from_utf8_lossy(&o.stdout), &fonts)),
                Err(e) => Err(format!("agenda: tock: {e}")),
            },
            "photo" => photo(&crate::expand(arg)),
            _ => Err(format!("~/.dm41/off: no picture called {kind}")),
        };
        match pic {
            Ok(p) => pics.push((format!("dm41-{kind}.bmp"), p.bmp())),
            Err(e) => notes.push(e),
        }
    }
    if fonts.regular.is_none() && !pics.is_empty() {
        notes.push("no font found, so the pictures have no text".into());
    }
    (pics, notes)
}

/// Write the pictures into `dir`, each only when it changed, and remove
/// dm41's own pictures that ~/.dm41/off no longer asks for. Other
/// pictures in the folder are left alone. None when there is no
/// ~/.dm41/off.
pub fn refresh(dir: &Path) -> Option<String> {
    let cfg = std::fs::read_to_string(crate::home().join("off")).ok()?;
    let (pics, mut notes) = make(&cfg, &Now::get());
    Some(match put(dir, &pics) {
        Ok(n) => {
            notes.insert(0, format!("{n} off picture{} written to {}", if n == 1 { "" } else { "s" }, dir.display()));
            notes.join("; ")
        }
        Err(e) => e,
    })
}

/// Write `pics` into `dir`; gives how many were new or changed.
fn put(dir: &Path, pics: &[(String, Vec<u8>)]) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut n = 0;
    for (name, bytes) in pics {
        let to: PathBuf = dir.join(name);
        if std::fs::read(&to).ok().as_ref() != Some(bytes) {
            std::fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
            n += 1;
        }
    }
    for kind in KINDS {
        let name = format!("dm41-{kind}.bmp");
        if !pics.iter().any(|(n, _)| *n == name) {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn when() -> Now {
        Now { y: 2026, m: 11, d: 25, hour: 21, min: 15, wday: 3, tz: 1.0, ut: (2026, 11, 25, 20.25) }
    }

    #[test]
    fn bmp_has_the_calculators_own_layout() {
        let mut p = Pic::paper();
        p.ink[0] = true;
        let b = p.bmp();
        assert_eq!(b.len(), 12610);
        assert_eq!(&b[..2], b"BM");
        assert_eq!(u32::from_le_bytes(b[10..14].try_into().unwrap()), 130);
        assert_eq!(&b[18..30], &[0x90, 1, 0, 0, 0xf0, 0, 0, 0, 1, 0, 1, 0]);
        // The top row is stored last; its first pixel is ink, so bit 0.
        let top = 130 + 52 * 239;
        assert_eq!(b[top], 0x7f);
        assert_eq!(b[130], 0xff);
    }

    #[test]
    fn a_crescent_is_mostly_dark_and_a_full_moon_mostly_light() {
        let fonts = Fonts { regular: None, bold: None };
        let light = |d: u32| {
            let now = Now { y: 2026, m: 12, d, hour: 18, min: 0, wday: 0, tz: 1.0, ut: (2026, 12, d, 17.0) };
            moon(&now, None, &fonts).ink.iter().filter(|&&i| !i).count()
        };
        // New moon on 9 December 2026, full on 23 December.
        assert!(light(23) > 5 * light(11));
    }

    #[test]
    fn agenda_lines_lose_the_place_and_calendar() {
        let l = "2026-11-25 Wed 09:00-10:00  Dentist  @ Main St 3  (Home, calendar 2, event 7)";
        assert_eq!(agenda_line(l), Some(((2026, 11, 25), "09:00".into(), "Dentist".into())));
        let l = "2026-11-26 Thu all day  Trip to the coast  (Home, calendar 2, event 8)";
        assert_eq!(agenda_line(l), Some(((2026, 11, 26), "all day".into(), "Trip to the coast".into())));
        assert_eq!(agenda_line("tock: no store"), None);
    }

    #[test]
    fn weekdays_match_the_calendar() {
        assert_eq!(weekday(2026, 11, 25), 3);
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(weekday(2024, 2, 29), 4);
    }

    #[test]
    fn pictures_are_written_only_when_changed_and_others_are_left() {
        let dir = std::env::temp_dir().join(format!("dm41-off-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mine.bmp"), b"x").unwrap();
        std::fs::write(dir.join("dm41-photo.bmp"), b"old").unwrap();
        let fonts = Fonts::load();
        let pics = vec![("dm41-moon.bmp".to_string(), moon(&when(), Some((51.5, 0.0)), &fonts).bmp())];
        assert_eq!(put(&dir, &pics), Ok(1));
        assert_eq!(put(&dir, &pics), Ok(0));
        assert!(dir.join("mine.bmp").exists());
        assert!(!dir.join("dm41-photo.bmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
