//! Module files (.MOD) and setups (.m41) on the calculator's disk: what
//! each module holds, the names of its functions, and which modules a setup
//! plugs in.

use crate::focal::{self, Names};
use std::path::{Path, PathBuf};

/// One 4K page of a module, and its function table when it has one.
pub struct Rom {
    pub name: String,
    pub bank: u8,
    pub xrom: u8,
    /// Function names by number; empty where the table points at no name.
    pub funcs: Vec<String>,
}

pub struct Module {
    pub file: String,
    pub title: String,
    pub author: String,
    pub version: String,
    pub roms: Vec<Rom>,
}

const HEAD: usize = 729; // the file header
const PAGE: usize = 5188; // one page: 36 bytes about it, the image, 32 spare
const IMAGE: usize = 36;

fn cstr(b: &[u8]) -> String {
    String::from_utf8_lossy(b.split(|&c| c == 0).next().unwrap_or_default()).trim().to_string()
}

/// A page image as 4096 words of 10 bits; five bytes hold four words.
fn words(img: &[u8]) -> Vec<u16> {
    let mut w = Vec::with_capacity(4096);
    for b in img.chunks_exact(5) {
        let b: Vec<u16> = b.iter().map(|&x| x as u16).collect();
        w.push(b[0] | (b[1] & 3) << 8);
        w.push(b[1] >> 2 | (b[2] & 0xF) << 6);
        w.push(b[2] >> 4 | (b[3] & 0x3F) << 4);
        w.push(b[3] >> 6 | b[4] << 2);
    }
    w
}

/// A character of a function name, in the display's own set.
fn lcd(c: u16) -> char {
    let n = (c & 0x3F) as u8;
    let plain = if n < 0x20 { (n + 0x40) as char } else { n as char };
    match (c & 0x40 != 0, n) {
        (true, 1..=5) => (b'a' + n - 1) as char,
        (true, 0x0D) => '≠',
        (true, 0x0E) => 'Σ',
        _ => plain,
    }
}

/// The name a function table entry points at. A machine code function has
/// its name just below its start, last letter first and marked; a program
/// starts with a global label.
fn name_at(w: &[u16], hi: u16, lo: u16) -> Option<String> {
    if hi & 0x1F0 != 0 {
        return None;
    }
    let a = ((hi as usize & 0x0F) << 8) | (lo as usize & 0xFF);
    if hi & 0x200 != 0 {
        let (start, len) = (*w.get(a)?, *w.get(a + 2)?);
        if start & 0xF0 != 0xC0 || len & 0xF0 != 0xF0 {
            return None;
        }
        let n = (len as usize & 0x0F).checked_sub(1)?;
        return Some(w.get(a + 4..a + 4 + n)?.iter().map(|&c| focal::hp_char(c as u8)).collect());
    }
    let mut s = String::new();
    for k in 1..=15 {
        let c = *w.get(a.checked_sub(k)?)?;
        s.push(lcd(c));
        if c & 0x80 != 0 {
            return Some(s);
        }
    }
    None
}

fn rom(pg: &[u8]) -> Rom {
    let mut r = Rom { name: cstr(&pg[..20]), bank: pg[31], xrom: 0, funcs: vec![] };
    if pg[35] == 0 {
        return r;
    }
    let w = words(&pg[IMAGE..IMAGE + 5120]);
    if !(1..32).contains(&w[0]) || w[1] > 64 {
        return r;
    }
    r.xrom = w[0] as u8;
    r.funcs = (0..w[1] as usize).map(|i| name_at(&w, w[2 + 2 * i], w[3 + 2 * i]).unwrap_or_default()).collect();
    r
}

/// Read a .MOD file; None when it is not one.
pub fn read(path: &Path) -> Option<Module> {
    let d = std::fs::read(path).ok()?;
    if d.len() < HEAD || &d[..4] != b"MOD1" {
        return None;
    }
    let pages = d[696] as usize;
    let roms = (0..pages).filter_map(|p| d.get(HEAD + PAGE * p..HEAD + PAGE * (p + 1))).map(rom).collect();
    Some(Module {
        file: path.file_name()?.to_string_lossy().into_owned(),
        title: cstr(&d[5..55]),
        version: cstr(&d[55..65]),
        author: cstr(&d[85..135]),
        roms,
    })
}

/// Every module in a folder, by file name.
pub fn read_all(dir: &Path) -> Vec<Module> {
    let mut v: Vec<Module> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().filter_map(|e| read(&e.path())).collect()).unwrap_or_default();
    v.sort_by_key(|m| m.file.to_lowercase());
    v
}

impl Module {
    pub fn stem(&self) -> &str {
        self.file.rsplit_once('.').map_or(&self.file, |(s, _)| s)
    }

    /// The XROM numbers its function tables use, without repeats.
    pub fn xroms(&self) -> Vec<u8> {
        let mut v: Vec<u8> = self.roms.iter().filter(|r| r.xrom > 0).map(|r| r.xrom).collect();
        v.dedup();
        v
    }
}

/// A setup: the module list the calculator loads with Module menu, Load
/// Modules. `A` lines are plugged in, `N` lines only sit in flash.
#[derive(Clone)]
pub struct Setup {
    pub name: String,
    pub path: PathBuf,
    pub lines: Vec<(char, String)>,
    /// Part of a full backup (in the BACKUP folder), so never changed here.
    pub backup: bool,
}

/// A module's file name in a setup line, such as `/MODS/ISENE.mod`.
fn file_of(entry: &str) -> &str {
    entry.rsplit('/').next().unwrap_or(entry)
}

impl Setup {
    pub fn parse(path: &Path, text: &str, backup: bool) -> Setup {
        let lines = text
            .lines()
            .filter_map(|l| {
                let (k, f) = l.trim().split_once(' ')?;
                Some((k.chars().next()?, f.trim().to_string()))
            })
            .collect();
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        Setup { name, path: path.to_path_buf(), lines, backup }
    }

    pub fn text(&self) -> String {
        self.lines.iter().map(|(k, f)| format!("{k} {f}\n")).collect()
    }

    /// `A` when the module is plugged in, `N` when it only sits in flash.
    pub fn mark(&self, file: &str) -> Option<char> {
        self.lines.iter().find(|(_, f)| file_of(f).eq_ignore_ascii_case(file)).map(|(k, _)| *k)
    }

    /// Plug a module in (at the end, since the order picks the pages), or
    /// take it out.
    pub fn toggle(&mut self, file: &str) {
        match self.lines.iter().position(|(_, f)| file_of(f).eq_ignore_ascii_case(file)) {
            Some(i) if self.lines[i].0 == 'A' => {
                self.lines.remove(i);
            }
            Some(i) => self.lines[i].0 = 'A',
            None => {
                let at = self.lines.iter().rposition(|(k, _)| *k == 'A').map_or(0, |i| i + 1);
                self.lines.insert(at, ('A', format!("/MODS/{file}")));
            }
        }
    }
}

/// The setups in a copy of the disk: those of full backups, then those
/// made here, each group by name.
pub fn setups(disk: &Path) -> Vec<Setup> {
    let mut v = vec![];
    for (dir, backup) in [("BACKUP", true), ("MODS", false)] {
        let Ok(rd) = std::fs::read_dir(disk.join(dir)) else {
            continue;
        };
        let mut group: Vec<Setup> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("m41")))
            .filter_map(|p| Some(Setup::parse(&p, &std::fs::read_to_string(&p).ok()?, backup)))
            .collect();
        group.sort_by_key(|s| s.name.to_lowercase());
        v.extend(group);
    }
    // a setup dm41 made has its memory in BACKUP too; the MODS copy is the one to show
    let mine: Vec<String> = v.iter().filter(|s| !s.backup).map(|s| s.name.to_lowercase()).collect();
    v.retain(|s| !s.backup || !mine.contains(&s.name.to_lowercase()));
    v
}

/// The names for listings: the 41CX's own functions, then those of the
/// modules the setup plugs in, in its order.
pub fn names(mods: &[Module], setup: Option<&Setup>) -> Names {
    let mut names = focal::builtin_names();
    let Some(s) = setup else { return names };
    for (_, entry) in s.lines.iter().filter(|(k, _)| *k == 'A') {
        // the printer lives in the calculator's flash, not on its disk
        if file_of(entry).eq_ignore_ascii_case("PRINTER.mod") {
            for (f, n) in focal::PRINTER.iter().enumerate().skip(1) {
                names.entry((29, f as u8)).or_insert_with(|| n.to_string());
            }
        }
        let Some(m) = mods.iter().find(|m| m.file.eq_ignore_ascii_case(file_of(entry))) else {
            continue;
        };
        for r in m.roms.iter().filter(|r| r.xrom > 0) {
            for (f, n) in r.funcs.iter().enumerate().skip(1) {
                if !n.is_empty() && !n.starts_with('-') {
                    names.entry((r.xrom, f as u8)).or_insert_with(|| n.clone());
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(w: &[u16]) -> Vec<u8> {
        w.chunks(4)
            .flat_map(|q| {
                [
                    q[0] as u8,
                    (q[0] >> 8 | (q[1] & 0x3F) << 2) as u8,
                    (q[1] >> 6 | (q[2] & 0xF) << 4) as u8,
                    (q[2] >> 4 | (q[3] & 3) << 6) as u8,
                    (q[3] >> 2) as u8,
                ]
            })
            .collect()
    }

    /// A made-up module: XROM 7 with a header, one machine code function
    /// and one program.
    fn test_module() -> Vec<u8> {
        let mut w = vec![0u16; 4096];
        w[0] = 7;
        w[1] = 3;
        let mut code = |at: usize, name: &str, entry: usize| {
            for (k, c) in name.bytes().enumerate() {
                let n = if c >= 0x40 { c - 0x40 } else { c } as u16;
                w[at - 1 - k] = n | if k + 1 == name.len() { 0x80 } else { 0 };
            }
            w[2 + 2 * entry] = (at >> 8) as u16;
            w[3 + 2 * entry] = (at & 0xFF) as u16;
        };
        code(0x100, "-TEST", 0);
        code(0x120, "AB?", 1);
        let p = 0x200;
        w[p..p + 7].copy_from_slice(&[0xC0, 0x00, 0xF4, 0x00, b'F' as u16, b'O' as u16, b'O' as u16]);
        w[6] = 0x200 | (p >> 8) as u16;
        w[7] = (p & 0xFF) as u16;
        let mut d = vec![0u8; HEAD + PAGE];
        d[..4].copy_from_slice(b"MOD1");
        d[5..9].copy_from_slice(b"Test");
        d[696] = 1;
        let pg = &mut d[HEAD..];
        pg[..4].copy_from_slice(b"TEST");
        pg[31] = 1;
        pg[35] = 1;
        pg[IMAGE..IMAGE + 5120].copy_from_slice(&pack(&w));
        d
    }

    #[test]
    fn a_module_file_gives_its_function_names() {
        let dir = std::env::temp_dir().join(format!("dm41-mod-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("TEST.MOD"), test_module()).unwrap();
        let m = read(&dir.join("TEST.MOD")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(m.title, "Test");
        assert_eq!(m.xroms(), vec![7]);
        assert_eq!(m.roms[0].funcs, vec!["-TEST", "AB?", "FOO"]);
        let s = Setup::parse(Path::new("x.m41"), "A /MODS/TEST.MOD\n", false);
        let n = names(&[m], Some(&s));
        assert_eq!(n[&(7, 1)], "AB?");
        assert_eq!(n[&(7, 2)], "FOO");
        assert!(!n.contains_key(&(7, 0)));
    }

    #[test]
    fn the_printer_names_come_only_when_it_is_plugged_in() {
        let plugged = Setup::parse(Path::new("x.m41"), "A PRINTER.mod\n", false);
        let n = names(&[], Some(&plugged));
        assert_eq!((n[&(29, 8)].as_str(), n[&(29, 20)].as_str()), ("PRA", "PRX"));
        let in_flash = Setup::parse(Path::new("x.m41"), "N PRINTER.mod\n", false);
        assert!(!names(&[], Some(&in_flash)).contains_key(&(29, 8)));
    }

    #[test]
    fn a_setup_plugs_modules_in_and_out() {
        let mut s = Setup::parse(Path::new("BACKUP/x.m41"), "A /MODS/ONE.MOD\nA /MODS/TWO.mod\nN PRINTER.mod\n", true);
        assert_eq!(s.name, "x");
        assert_eq!(s.mark("two.MOD"), Some('A'));
        s.toggle("ONE.MOD");
        s.toggle("THREE.MOD");
        s.toggle("PRINTER.mod");
        assert_eq!(s.text(), "A /MODS/TWO.mod\nA /MODS/THREE.MOD\nA PRINTER.mod\n");
    }
}
