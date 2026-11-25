mod focal;
mod keys;
mod modules;
mod off;
mod state;

use crust::{style, Crust, Input, Pane};
use focal::Names;
use modules::{Module, Setup};
use state::{show, Program, State, XmFile};
use std::path::{Path, PathBuf};

const HELP: &str = "dm41: the memory of a SwissMicros DM41X in the terminal

  dm41                      the newest state on the calculator's disk, or the last copy
  dm41 FILE.d41             that state file
  dm41 --list [FILE]        every program as a listing
  dm41 --raw NAME [FILE]    one program as a .raw file, on stdout
  dm41 --xm NAME [FILE]     what an X-Memory file holds
  dm41 --encode LISTING     a listing as a .raw file, on stdout
  dm41 --mount              mount the calculator's disk for writing
  dm41 --unmount            unmount it, so the calculator can be unplugged
  dm41 --off [DIR]          write the pictures ~/.dm41/off names to the
                            calculator's OFFIMG folder, or to DIR
  -h, --help   -v, --version

Keys: Left/Right tab, Up/Down pick, PgUp/PgDn scroll, R read again, q quit
      B copies the calculator's whole disk to ~/.dm41/backups
  Programs  e edit in scribe (NAME.raw is saved when scribe ends)
            r run in rpnx    s save NAME.raw    l load a .raw or a listing
  X-Memory  e edit a text file in scribe    l load a text file
  Modules   Space plugs the module in or out of the setup
  Keys      the functions and programs on keys in USER mode, and the
            CST menus in the calculator's KEYS folder
  Library   the programs in the folders ~/.dm41/library names
            l loads the picked one into the setup
  Setups    Up/Down picks the setup the listings use
            n new setup from this one    s copy it to the calculator
            D deletes it from the calculator; a copy goes to ~/.dm41/deleted
  In a setup made with n, Space in Programs, X-Memory and Flags puts that
  program, file or flag in or out of the setup's memory. Loading a file
  and editing put the new version in the setup.

The calculator shows its disk in USB disk mode (File menu). dm41 mounts it
at /mnt/dm41x when a sudoers line allows (see the README), and unmounts it
on quit. It reads the newest .d41 in its STATE folder and keeps a copy in
~/.dm41/last.d41.
Listings and .raw files go to ~/.dm41/prog; a .raw also goes to the
calculator's PROG folder when its disk is mounted for writing. Load it on
the calculator with File menu, Load RAW Program.

Modules and setups (.m41 module lists) are copied from the disk to
~/.dm41/disk. A new setup goes to its MODS folder; load it on the
calculator with Module menu, Load Modules, Load Flash and AM Lists.
A setup with a memory of its own is written as a full backup in BACKUP;
load it with Settings menu, Restore from Backup.

~/.dm41/off names pictures for the calculator to show while it is off,
one a line: moon [LAT LON], agenda (from tock) and photo FILE. dm41 makes
them each time it finds the calculator's disk mounted for writing.";

const HEAD_BG: u16 = 236;
const PANEL_BG: u16 = 16;
const SEL_BG: u8 = 238;
const GOLD: u8 = 178; // the shift key's colour on the HP-41
const DIM: u8 = 244;

/// What each flag does, as the calculator's own help names it.
fn flag_name(n: u32) -> &'static str {
    match n {
        0..=4 => "annunciator",
        5..=8 | 19 | 20 => "free for programs",
        9 => "matrix end of row",
        10 => "matrix end of column",
        11 => "start on power on",
        12 => "print double wide",
        13 => "print lower case",
        14 => "card reader may overwrite",
        15 => "HP-IL printer manual",
        16 => "HP-IL printer normal",
        17 => "record not complete",
        18 => "HP-IL interrupt",
        21 => "printer on",
        22 => "number typed",
        23 => "text typed",
        24 => "ignore range errors",
        25 => "ignore one error",
        26 => "sound",
        27 => "USER keys",
        28 => "point as decimal mark",
        29 => "digit groups",
        30 => "catalog shown",
        31 => "dates as day.month",
        32 => "HP-IL manual I/O",
        33 => "HP-IL control",
        34 => "HP-IL auto address",
        35 => "no auto start",
        36..=39 => "number of digits",
        40 | 41 => "display mode",
        42 | 43 => "angle mode",
        44 => "stay on",
        45 => "system data entry",
        46 => "partial key sequence",
        47 => "shift",
        48 => "ALPHA mode",
        49 => "low battery",
        50 => "message shown",
        51 => "single step",
        52 => "program mode",
        53 => "HP-IL I/O",
        54 => "pause",
        _ => "printer present",
    }
}

/// Display mode, angles and USER mode, as the flags of a memory set them.
fn flag_summary(st: &State) -> Vec<String> {
    let f = |n| st.flag(n);
    let digits = (36..40).fold(0, |a, n| a * 2 + f(n) as u32);
    let mode = match (f(40), f(41)) {
        (true, false) => "FIX",
        (false, true) => "ENG",
        _ => "SCI",
    };
    let angle = if f(42) {
        "GRAD"
    } else if f(43) {
        "RAD"
    } else {
        "DEG"
    };
    vec![
        format!(" {}  {mode} {digits}", style::fg("Display", GOLD)),
        format!(" {}  {angle}", style::fg("Angles", GOLD)),
        format!(" {}  {}", style::fg("USER", GOLD), if f(27) { "on" } else { "off" }),
    ]
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".dm41")
}

const DEV: &str = "/dev/disk/by-label/DM41X";
const POINT: &str = "/mnt/dm41x";

/// Run a command through `sudo -n`, quietly; true when it worked.
fn sudo(args: &[&str]) -> bool {
    use std::process::{Command, Stdio};
    let mut run = Command::new("sudo");
    run.arg("-n").args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    run.status().is_ok_and(|s| s.success())
}

/// Mount the calculator's disk for writing when it is plugged in and not
/// mounted yet. True when this did the mount.
fn attach() -> bool {
    use std::os::unix::fs::MetadataExt;
    if mount().is_some() || !Path::new(DEV).exists() {
        return false;
    }
    let Ok(me) = std::fs::metadata("/proc/self") else { return false };
    sudo(&["/usr/bin/mount", "-o", &format!("rw,uid={},gid={}", me.uid(), me.gid()), DEV, POINT])
}

/// Unmount the calculator's disk, wherever it is mounted.
fn detach() -> Result<(), String> {
    let Some((m, _)) = mount() else { return Ok(()) };
    match sudo(&["/usr/bin/umount", &m.to_string_lossy()]) {
        true => Ok(()),
        false => Err(format!("could not unmount {}", m.display())),
    }
}

/// Where the calculator's disk is mounted, and whether it can be written.
fn mount() -> Option<(PathBuf, bool)> {
    let dev = std::fs::canonicalize("/dev/disk/by-label/DM41X").ok()?;
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    mounts.lines().find_map(|l| {
        let f: Vec<&str> = l.split(' ').collect();
        let rw = f.get(3).is_some_and(|o| o.split(',').any(|x| x == "rw"));
        (f.len() > 3 && Path::new(f[0]) == dev).then(|| (PathBuf::from(f[1].replace("\\040", " ")), rw))
    })
}

/// The copy of the calculator's modules and setups.
fn disk() -> PathBuf {
    home().join("disk")
}

fn modified(p: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|x| x.modified()).ok()
}

/// Copy the calculator's modules and setups to ~/.dm41/disk when they are
/// new or changed there, so they can be read with the calculator unplugged.
fn mirror(m: &Path) {
    for dir in ["MODS", "BACKUP", "KEYS"] {
        let Ok(rd) = std::fs::read_dir(m.join(dir)) else {
            continue;
        };
        let to = disk().join(dir);
        let _ = std::fs::create_dir_all(&to);
        for e in rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())) {
            let dest = to.join(e.file_name());
            if modified(&e.path()) > modified(&dest) {
                let _ = std::fs::copy(e.path(), &dest);
            }
        }
    }
}

fn newest_d41(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("d41")))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// The state to show: the named file, the newest on the calculator's disk
/// (copied to ~/.dm41/last.d41), or that last copy.
fn load(arg: Option<&str>) -> Result<(PathBuf, State), String> {
    let read = |p: &Path| -> Result<State, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        State::parse(&text).map_err(|e| format!("{}: {e}", p.display()))
    };
    if let Some(a) = arg {
        let p = PathBuf::from(a);
        return read(&p).map(|s| (p, s));
    }
    let last = home().join("last.d41");
    if let Some(p) = mount().and_then(|(m, _)| newest_d41(&m.join("STATE"))) {
        let s = read(&p)?;
        if modified(&p) > modified(&last) {
            let _ = std::fs::create_dir_all(home()).and_then(|_| std::fs::copy(&p, &last));
        }
        return Ok((p, s));
    }
    if last.exists() {
        return read(&last).map(|s| (last, s));
    }
    Err("no state yet: put the calculator in USB disk mode and mount its disk, or name a .d41 file".into())
}

/// A program name as a file name.
fn file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if s.is_empty() {
        "PROG".into()
    } else {
        s
    }
}

/// Encode a listing, write NAME.raw to ~/.dm41/prog, and to the
/// calculator's PROG folder when its disk can be written. Says what it did.
fn save_raw(name: &str, listing: &str, names: &Names) -> String {
    let bytes = match focal::encode(listing, names) {
        Ok(b) => b,
        Err(e) => return format!("{name}: {e}"),
    };
    let file = format!("{}.raw", file_name(name));
    let dir = home().join("prog");
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&file), &bytes)) {
        return format!("{file}: {e}");
    }
    match mount() {
        Some((m, true)) => {
            let to = m.join("PROG");
            match std::fs::create_dir_all(&to).and_then(|_| std::fs::write(to.join(&file), &bytes)) {
                Ok(_) => format!("{file}: {} bytes, on the calculator's disk in PROG", bytes.len()),
                Err(e) => format!("{file} saved in ~/.dm41/prog; the calculator's disk said: {e}"),
            }
        }
        Some((_, false)) => {
            format!("{file}: {} bytes in ~/.dm41/prog (the calculator's disk is read-only)", bytes.len())
        }
        None => format!("{file}: {} bytes in ~/.dm41/prog (mount the calculator's disk to copy it there)", bytes.len()),
    }
}

/// Write a setup to ~/.dm41/disk, and to the calculator's disk when it can
/// be written. Says what it did.
fn put_setup(s: &Setup) -> String {
    let file = format!("{}.m41", s.name);
    if let Err(e) = std::fs::create_dir_all(disk().join("MODS")).and_then(|_| std::fs::write(&s.path, s.text())) {
        return format!("{file}: {e}");
    }
    match mount() {
        Some((m, true)) => match std::fs::write(m.join("MODS").join(&file), s.text()) {
            Ok(_) => format!("{file} is on the calculator's disk in MODS"),
            Err(e) => format!("{file} saved in ~/.dm41/disk; the calculator's disk said: {e}"),
        },
        Some((_, false)) => {
            format!("{file} saved in ~/.dm41/disk (the calculator's disk is read-only)")
        }
        None => format!("{file} saved in ~/.dm41/disk (mount the calculator's disk, then s copies it there)"),
    }
}

/// The local date and time, as 2026-11-24_2315.
fn stamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // SAFETY: localtime_r only writes the tm we hand it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = now as libc::time_t;
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:04}-{:02}-{:02}_{:02}{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

/// Copy every file on the calculator's disk to ~/.dm41/backups/DATE_TIME.
fn backup_disk() -> String {
    fn copy(from: &Path, to: &Path, count: &mut (u64, u64)) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)?.flatten() {
            let t = e.file_type()?;
            if t.is_dir() {
                copy(&e.path(), &to.join(e.file_name()), count)?;
            } else if t.is_file() {
                count.1 += std::fs::copy(e.path(), to.join(e.file_name()))?;
                count.0 += 1;
            }
        }
        Ok(())
    }
    let Some((m, _)) = mount() else { return "mount the calculator's disk first: R mounts it".into() };
    let name = stamp();
    let mut count = (0, 0);
    match copy(&m, &home().join("backups").join(&name), &mut count) {
        Ok(_) => format!(
            "{} files ({} kB) copied to ~/.dm41/backups/{name}, with the memory as the calculator last saved it",
            count.0,
            count.1 / 1024
        ),
        Err(e) => format!("backup stopped after {} files: {e}", count.0),
    }
}

/// The files of setup NAME under root (the calculator's disk or its copy):
/// BACKUP/NAME.* and, for a setup that is not a full backup, MODS/NAME.m41.
fn setup_files(root: &Path, name: &str, backup: bool) -> Vec<PathBuf> {
    let same = |p: &Path| p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(name));
    let m41 = |p: &Path| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("m41"));
    let mut v = vec![];
    for (dir, all) in [("BACKUP", true), ("MODS", false)] {
        if dir == "MODS" && backup {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(root.join(dir)) {
            v.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && same(p) && (all || m41(p))));
        }
    }
    v.sort();
    v
}

/// Move the files of setup NAME off the calculator's disk (m) into keep,
/// and out of the copy in disk. Copies all before removing any, so a
/// failed copy leaves the calculator as it was. Gives the number moved.
fn delete_setup_files(m: &Path, disk: &Path, keep: &Path, name: &str, backup: bool) -> Result<usize, String> {
    let files = setup_files(m, name, backup);
    if files.is_empty() {
        return Err(format!("{name} has no files on the calculator's disk"));
    }
    for f in &files {
        let to = keep.join(f.strip_prefix(m).unwrap_or(f));
        let copied = to.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::copy(f, &to));
        copied.map_err(|e| format!("{}: {e}; nothing was deleted", f.display()))?;
    }
    for f in &files {
        std::fs::remove_file(f).map_err(|e| format!("{}: {e}", f.display()))?;
    }
    for f in setup_files(disk, name, backup) {
        let _ = std::fs::remove_file(f);
    }
    Ok(files.len())
}

/// A path typed at a prompt, with ~/ as the home folder.
fn expand(p: &str) -> PathBuf {
    match p.trim().strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p.trim()),
    }
}

/// Program lines numbered and coloured, as the Programs tab shows them.
fn numbered(steps: &[focal::Step]) -> Vec<String> {
    let w = if steps.len() >= 100 { 3 } else { 2 };
    steps
        .iter()
        .enumerate()
        .map(|(n, s)| format!("{} {}", style::fg(&format!("{:0w$}", n + 1), DIM), colour_step(&s.text)))
        .collect()
}

/// The bytes of a .raw file, or of a listing made into bytes.
fn program_file(path: &Path, names: &Names) -> Result<Vec<u8>, String> {
    let err = |e: String| format!("{}: {e}", path.display());
    let b = std::fs::read(path).map_err(|e| err(e.to_string()))?;
    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("raw")) {
        return Ok(b);
    }
    // a listing not in UTF-8 is taken as Latin-1, as old ones often are
    let text = String::from_utf8(b).unwrap_or_else(|e| e.into_bytes().iter().map(|&c| c as char).collect());
    focal::encode(&text, names).map_err(err)
}

/// The .41 listings and .raw files in the folders `cfg` names, one a line,
/// and in their subfolders, sorted by name.
fn library_files(cfg: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(cfg) else { return vec![] };
    let is_prog = |p: &Path| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("41") || x.eq_ignore_ascii_case("raw"));
    let visible = |e: &std::fs::DirEntry| !e.file_name().to_string_lossy().starts_with('.');
    let mut out = vec![];
    for dir in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Ok(rd) = std::fs::read_dir(expand(dir)) else { continue };
        for e in rd.flatten().filter(visible) {
            let p = e.path();
            if p.is_dir() {
                let sub = std::fs::read_dir(&p).into_iter().flatten().flatten().filter(visible);
                out.extend(sub.map(|e| e.path()).filter(|p| is_prog(p)));
            } else if is_prog(&p) {
                out.push(p);
            }
        }
    }
    out.sort_by_key(|p| (p.file_stem().map(|s| s.to_string_lossy().to_lowercase()), p.clone()));
    out.dedup();
    out
}

/// The name a program goes by: its first global label, as in the lists.
fn prog_name(bytes: &[u8], names: &Names) -> String {
    state::split(focal::decode(bytes, names)).first().map_or_else(String::new, |p| p.name())
}

/// The memory a setup made by dm41 has, if it has one yet.
fn set_memory(s: &Setup) -> Option<State> {
    State::parse(&std::fs::read_to_string(disk().join("BACKUP").join(format!("{}.d41", s.name))).ok()?).ok()
}

/// Write a setup with its memory as a full backup in BACKUP: NAME.d41,
/// NAME.m41 and NAME.b41, with NAME.ram and NAME.cst copied from the
/// newest backup (src, its .m41) when missing. To ~/.dm41/disk, and to the
/// calculator's disk when it can be written. Says what it did.
fn put_set(s: &Setup, mem: &State, src: Option<&Path>) -> String {
    let n = &s.name;
    let write = |dir: &Path| -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let has = |ext: &str| dir.join(format!("{n}.{ext}")).exists();
        for ext in ["ram", "cst"] {
            if let Some(from) = src.map(|p| p.with_extension(ext)).filter(|f| f.exists() && !has(ext)) {
                std::fs::copy(from, dir.join(format!("{n}.{ext}")))?;
            }
        }
        let mut b41 = format!("MODS {n}.m41\n");
        if has("ram") {
            b41.push_str(&format!("RAM: {n}.ram\n"));
        }
        b41.push_str(&format!("STAT {n}.d41\n"));
        if has("cst") {
            b41.push_str(&format!("KEYS {n}.cst\n"));
        }
        std::fs::write(dir.join(format!("{n}.d41")), mem.text())?;
        std::fs::write(dir.join(format!("{n}.m41")), s.text())?;
        std::fs::write(dir.join(format!("{n}.b41")), b41)
    };
    if let Err(e) = write(&disk().join("BACKUP")) {
        return format!("{n}.b41: {e}");
    }
    match mount() {
        Some((m, true)) => match write(&m.join("BACKUP")) {
            Ok(_) => format!("{n}.b41 is on the calculator's disk in BACKUP"),
            Err(e) => format!("{n}.b41 saved in ~/.dm41/disk; the calculator's disk said: {e}"),
        },
        Some((_, false)) => format!("{n}.b41 saved in ~/.dm41/disk (the calculator's disk is read-only)"),
        None => format!("{n}.b41 saved in ~/.dm41/disk (mount the calculator's disk, then s copies it there)"),
    }
}

/// A program's steps without the END or .END. it ends with, so the same
/// program matches whether it has its END yet or not.
fn steps_of(p: &Program) -> Vec<String> {
    let mut v: Vec<String> = p.steps.iter().map(|s| s.text.clone()).collect();
    if v.last().is_some_and(|t| t == "END" || t == ".END.") {
        v.pop();
    }
    v
}

/// The setup of the newest full backup, which is what the calculator last
/// had plugged in as far as its disk tells.
fn newest_setup(setups: &[Setup]) -> usize {
    setups.iter().enumerate().filter(|(_, s)| s.backup).max_by_key(|(_, s)| modified(&s.path)).map_or(0, |(i, _)| i)
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Programs,
    Registers,
    Flags,
    Xm,
    Modules,
    Setups,
    Keys,
    Library,
}

const TABS: [(Tab, &str); 8] = [
    (Tab::Programs, "Programs"),
    (Tab::Registers, "Registers"),
    (Tab::Flags, "Flags"),
    (Tab::Xm, "X-Memory"),
    (Tab::Modules, "Modules"),
    (Tab::Setups, "Setups"),
    (Tab::Keys, "Keys"),
    (Tab::Library, "Library"),
];

struct App {
    path: PathBuf,
    st: State,
    names: Names,
    progs: Vec<Program>,
    xm: Vec<XmFile>,
    mods: Vec<Module>,
    setups: Vec<Setup>,
    setup: usize,
    /// The memory of the picked setup, when dm41 made it.
    mem: Option<State>,
    /// For each listed program and X-Memory file: 0 not in the setup's
    /// memory, 1 in it, 2 only in it.
    pmarks: Vec<u8>,
    xmarks: Vec<u8>,
    keys: Vec<keys::Assigned>,
    cst: Vec<(String, Vec<(String, String)>)>,
    /// The programs in the folders ~/.dm41/library names, read the first
    /// time the Library tab opens.
    lib: Option<Vec<PathBuf>>,
    /// dm41 mounted the calculator's disk, so it unmounts it on quit.
    attached: bool,
    tab: Tab,
    sel: usize,
    note: String,
    top: Pane,
    left: Pane,
    right: Pane,
    foot: Pane,
}

/// A listing line coloured by what it does, the same as in hp41.vim: labels
/// gold, jumps purple, stops red, tests orange, registers and flags blue,
/// text green, numbers cyan.
fn step_colour(text: &str) -> u8 {
    let (word, arg) = text.split_once(' ').unwrap_or((text, ""));
    match word {
        "LBL" => GOLD,
        _ if text.starts_with('"') => 114,
        "GTO" | "XEQ" => 177,
        "RTN" | "END" | ".END." | "STOP" | "PROMPT" | "OFF" if arg.is_empty() => 203,
        "ISG" | "DSE" => 208,
        _ if word.contains('?') => 208,
        "STO" | "RCL" | "ST+" | "ST-" | "ST*" | "ST/" | "X<>" | "ARCL" | "ASTO" | "VIEW" | "SF" | "CF"
            if !arg.is_empty() =>
        {
            111
        }
        "XROM" => DIM,
        _ if text.contains(|c: char| c.is_ascii_digit()) && text.chars().all(|c| "0123456789.-E".contains(c)) => 81,
        _ => 252,
    }
}

fn colour_step(text: &str) -> String {
    let c = step_colour(text);
    if c == GOLD { style::fg(&style::bold(text), c) } else { style::fg(text, c) }
}

impl App {
    fn new(path: PathBuf, st: State, mods: Vec<Module>, setups: Vec<Setup>, attached: bool) -> App {
        let setup = newest_setup(&setups);
        let names = modules::names(&mods, setups.get(setup));
        let blank = || Pane::new(1, 1, 1, 1, 252, PANEL_BG);
        let mut app = App {
            progs: st.programs(&names),
            xm: st.xm_files(),
            path,
            st,
            names,
            mods,
            setups,
            setup,
            mem: None,
            pmarks: vec![],
            xmarks: vec![],
            keys: vec![],
            cst: vec![],
            lib: None,
            attached,
            tab: Tab::Programs,
            sel: 0,
            note: String::new(),
            top: blank(),
            left: blank(),
            right: blank(),
            foot: blank(),
        };
        app.layout();
        app.pick_setup(setup);
        app
    }

    fn layout(&mut self) {
        let (cols, rows) = Crust::terminal_size();
        let lw = 26.min(cols / 3).max(12);
        let body = rows.saturating_sub(2).max(1);
        let mk = |x, y, w: u16, h: u16, bg| {
            let mut p = Pane::new(x, y, w.max(1), h.max(1), 252, bg);
            p.wrap = false;
            p.scroll = false;
            p
        };
        self.top = mk(1, 1, cols, 1, HEAD_BG);
        self.left = mk(1, 2, lw, body, PANEL_BG);
        self.right = mk(lw + 2, 2, cols.saturating_sub(lw + 1), body, PANEL_BG);
        self.foot = mk(1, rows, cols, 1, HEAD_BG);
        Crust::clear_screen();
    }

    fn reload(&mut self) {
        self.attached |= attach();
        if let Some((m, _)) = mount() {
            mirror(&m);
        }
        let off = off_pictures();
        match load(None) {
            Ok((p, s)) => {
                let name = self.setups.get(self.setup).map(|s| s.name.clone());
                self.mods = modules::read_all(&disk().join("MODS"));
                self.setups = modules::setups(&disk());
                self.setup = self.setups.iter().position(|s| Some(&s.name) == name.as_ref()).unwrap_or(0);
                self.path = p;
                self.st = s;
                self.sel = 0;
                self.lib = None;
                if self.tab == Tab::Library {
                    self.go(Tab::Library);
                }
                self.pick_setup(self.setup);
                self.note = format!("read {}", self.path.display());
                if let Some(n) = off {
                    self.note = format!("{}; {n}", self.note);
                }
            }
            Err(e) => self.note = e,
        }
    }

    fn count(&self) -> usize {
        match self.tab {
            Tab::Programs => self.progs.len(),
            Tab::Xm => self.xm.len(),
            Tab::Modules => self.mods.len(),
            Tab::Setups => self.setups.len(),
            Tab::Flags => 56,
            Tab::Keys => self.keys.len(),
            Tab::Library => self.lib.as_ref().map_or(0, |l| l.len()),
            _ => 0,
        }
    }

    /// Use another setup's modules for the names in listings, and its
    /// memory for the marks when dm41 made it.
    fn pick_setup(&mut self, i: usize) {
        self.setup = i;
        self.names = modules::names(&self.mods, self.setups.get(i));
        self.mem = self.setups.get(i).filter(|s| !s.backup).map(|s| set_memory(s).unwrap_or_else(|| self.st.clone()));
        self.fill();
    }

    /// The programs and X-Memory files to list: the calculator's, then those
    /// only the setup's memory has, each marked by whether that memory has it.
    fn fill(&mut self) {
        let mut progs = self.st.programs(&self.names);
        let end = if progs.last().is_some_and(|p| steps_of(p).is_empty()) { progs.pop() } else { None };
        let mut xm = self.st.xm_files();
        (self.pmarks, self.xmarks) = (vec![], vec![]);
        if let Some(m) = &self.mem {
            let theirs: Vec<Program> = m.program_list(&self.names).into_iter().map(|(p, _)| p).collect();
            let keys: Vec<Vec<String>> = theirs.iter().map(steps_of).collect();
            let mine: Vec<Vec<String>> = progs.iter().map(steps_of).collect();
            self.pmarks = mine.iter().map(|k| keys.contains(k) as u8).collect();
            for (p, k) in theirs.into_iter().zip(keys) {
                if !mine.contains(&k) {
                    progs.push(p);
                    self.pmarks.push(2);
                }
            }
            let files = m.xm_files();
            self.xmarks = xm.iter().map(|f| files.iter().any(|g| g.name == f.name) as u8).collect();
            // a file both have is shown as the setup has it, since edits go there
            for f in xm.iter_mut() {
                if let Some(g) = files.iter().find(|g| g.name == f.name) {
                    *f = g.clone();
                }
            }
            for g in files {
                if !xm.iter().any(|f| f.name == g.name) {
                    xm.push(g);
                    self.xmarks.push(2);
                }
            }
        }
        progs.extend(end);
        self.progs = progs;
        self.xm = xm;
        self.keys = keys::assigned(&self.st, &self.names);
        self.cst = keys::cst(&disk().join("KEYS"));
        self.sel = self.sel.min(self.count().saturating_sub(1));
    }

    /// The newest full backup's .m41, whose .ram and .cst a new set copies.
    fn newest_backup(&self) -> Option<PathBuf> {
        self.setups.iter().filter(|s| s.backup).max_by_key(|s| modified(&s.path)).map(|s| s.path.clone())
    }

    /// Put the picked program, X-Memory file or flag in or out of the
    /// setup's memory, and write the setup as a full backup.
    fn pick(&mut self) {
        let Some(mem) = self.mem.clone() else {
            self.note = "pick a setup of your own in Setups first; n there makes one".into();
            return;
        };
        let new = match self.tab {
            Tab::Programs => {
                let Some(p) = self.progs.get(self.sel) else { return };
                let k = steps_of(p);
                if k.is_empty() {
                    self.note = ".END. marks the end of memory; it is always there".into();
                    return;
                }
                let mut code = mem.program_list(&self.names);
                let Some(i) = code.iter().position(|(q, _)| steps_of(q) == k) else {
                    // in it goes: the working copy when there is one, else the calculator's version
                    let bytes = match std::fs::read_to_string(self.working_copy(p)) {
                        Ok(text) => match focal::encode(&text, &self.names) {
                            Ok(b) => state::program_bytes(&b),
                            Err(e) => return self.note = format!("{}: {e}", p.name()),
                        },
                        Err(_) => match self.st.program_list(&self.names).into_iter().find(|(q, _)| steps_of(q) == k) {
                            Some((_, c)) => c,
                            None => return,
                        },
                    };
                    return self.put_program(bytes);
                };
                code.remove(i);
                mem.with_programs(&code.into_iter().map(|(_, c)| c).collect::<Vec<_>>())
            }
            Tab::Xm => {
                let Some(f) = self.xm.get(self.sel) else { return };
                let mut files = mem.xm_files();
                match files.iter().position(|g| g.name == f.name) {
                    Some(i) => drop(files.remove(i)),
                    None => files.push(f.clone()),
                }
                mem.with_xm(&files)
            }
            Tab::Flags => {
                let n = self.sel as u32;
                if n > 44 {
                    self.note = format!("flag {n} is the calculator's own; it sets it itself");
                    return;
                }
                let mut m = mem.clone();
                m.set_flag(n, !m.flag(n));
                Ok(m)
            }
            _ => return,
        };
        match new {
            Ok(m) => self.commit(m),
            Err(e) => self.note = e,
        }
    }

    /// Keep a new memory for the setup, and write the setup as a full backup.
    fn commit(&mut self, m: State) {
        let src = self.newest_backup();
        if let Some(s) = self.setups.get(self.setup) {
            self.note = put_set(s, &m, src.as_deref());
        }
        self.mem = Some(m);
        self.fill();
    }

    /// Put a program in the setup's memory, in place of one with its name.
    fn put_program(&mut self, bytes: Vec<u8>) {
        let Some(mem) = &self.mem else { return };
        let name = prog_name(&bytes, &self.names);
        let mut code: Vec<Vec<u8>> = vec![];
        let mut put = Some(bytes);
        for (p, c) in mem.program_list(&self.names) {
            code.push(if p.name() == name { put.take().unwrap_or(c) } else { c });
        }
        code.extend(put);
        match mem.with_programs(&code) {
            Ok(m) => self.commit(m),
            Err(e) => self.note = e,
        }
    }

    /// Put an X-Memory file in the setup's memory, in place of one with its name.
    fn put_file(&mut self, f: XmFile) {
        let Some(mem) = &self.mem else { return };
        let mut files = mem.xm_files();
        match files.iter().position(|g| g.name == f.name) {
            Some(i) => files[i] = f,
            None => files.push(f),
        }
        match mem.with_xm(&files) {
            Ok(m) => self.commit(m),
            Err(e) => self.note = e,
        }
    }

    /// True when a setup of dm41's own is picked, else says what to do.
    fn has_setup(&mut self) -> bool {
        if self.mem.is_none() {
            self.note = "pick a setup of your own in Setups first; n there makes one".into();
        }
        self.mem.is_some()
    }

    /// Load a program from a .raw file or a listing into the setup.
    fn load_program(&mut self) {
        if !self.has_setup() {
            return;
        }
        let Some(path) = self.foot.ask_or_cancel(" Program file (.raw or a listing): ", "~/.dm41/prog/") else { return };
        self.load_path(&expand(&path));
    }

    /// Load the program picked in the Library into the setup.
    fn load_library(&mut self) {
        let Some(path) = self.lib.as_ref().and_then(|l| l.get(self.sel)).cloned() else { return };
        if self.has_setup() {
            self.load_path(&path);
        }
    }

    fn load_path(&mut self, path: &Path) {
        match program_file(path, &self.names) {
            Ok(b) => self.put_program(state::program_bytes(&b)),
            Err(e) => self.note = e,
        }
    }

    /// Load a text file into the setup's X-Memory, a line to a record.
    fn load_text(&mut self) {
        if !self.has_setup() {
            return;
        }
        let Some(path) = self.foot.ask_or_cancel(" Text file: ", "~/") else { return };
        let path = expand(&path);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return self.note = format!("{}: {e}", path.display()),
        };
        let stem: String = path.file_stem().map(|x| x.to_string_lossy().to_uppercase()).unwrap_or_default().chars().take(7).collect();
        let Some(name) = self.foot.ask_or_cancel(" Name in X-Memory, 7 characters at most: ", &stem) else { return };
        match state::text_file(name.trim(), &text) {
            Ok(f) => self.put_file(f),
            Err(e) => self.note = e,
        }
    }

    /// Edit the picked X-Memory text file in scribe; the setup gets the result.
    fn edit_text(&mut self) {
        let Some(f) = self.xm.get(self.sel).cloned() else { return };
        if f.kind != 3 {
            self.note = "only text files open in scribe".into();
            return;
        }
        if !self.has_setup() {
            return;
        }
        let file = home().join("xm").join(format!("{}.txt", file_name(&f.name)));
        let lines = state::xm_lines(&f, &self.names);
        let text = if lines.is_empty() { String::new() } else { lines.join("\n") + "\n" };
        if let Err(e) = std::fs::create_dir_all(home().join("xm")).and_then(|_| std::fs::write(&file, text)) {
            return self.note = format!("{}: {e}", file.display());
        }
        if self.run_child("scribe", &file) {
            match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| state::text_file(&f.name, &t)) {
                Ok(g) => self.put_file(g),
                Err(e) => self.note = e,
            }
        }
    }

    /// Plug the picked module in or out of the setup, and save it.
    fn toggle(&mut self) {
        let Some(m) = self.mods.get(self.sel) else {
            return;
        };
        let Some(s) = self.setups.get_mut(self.setup) else {
            self.note = "no setup yet: the calculator's disk has none".into();
            return;
        };
        if s.backup {
            self.note = format!("{} belongs to a full backup; make a copy with n in Setups", s.name);
            return;
        }
        s.toggle(&m.file);
        self.note = put_setup(s);
        let s = s.clone();
        if let Some(mem) = set_memory(&s) {
            self.note = put_set(&s, &mem, self.newest_backup().as_deref());
        }
        self.pick_setup(self.setup);
    }

    /// A new setup with the same modules as the picked one.
    fn new_setup(&mut self) {
        let Some(from) = self.setups.get(self.setup) else {
            return;
        };
        let prompt = format!(" New setup with the modules of {}, named: ", from.name);
        let Some(name) = self.foot.ask_or_cancel(&prompt, "") else {
            return;
        };
        let name = file_name(name.trim());
        let path = disk().join("MODS").join(format!("{name}.m41"));
        if self.setups.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
            self.note = format!("there is a setup {name} already");
            return;
        }
        let s = Setup { name: name.clone(), path, lines: from.lines.clone(), backup: false };
        let mem = set_memory(from);
        self.note = put_setup(&s);
        if let Some(mem) = mem {
            self.note = put_set(&s, &mem, self.newest_backup().as_deref());
        }
        self.setups = modules::setups(&disk());
        let i = self.setups.iter().position(|s| s.name == name).unwrap_or(0);
        self.sel = i;
        self.pick_setup(i);
    }

    /// Delete the picked setup from the calculator's disk, keeping a copy
    /// in ~/.dm41/deleted. The calculator itself cannot delete files.
    fn delete_setup(&mut self) {
        let Some(s) = self.setups.get(self.sel).cloned() else { return };
        let Some((m, true)) = mount() else {
            return self.note = "the calculator's disk must be mounted for writing: plug it in, R mounts it".into();
        };
        let n = setup_files(&m, &s.name, s.backup).len();
        let prompt = format!(" Delete {} ({n} files) from the calculator? A copy goes to ~/.dm41/deleted. y/n: ", s.name);
        if !self.foot.ask_or_cancel(&prompt, "").is_some_and(|a| a.trim().eq_ignore_ascii_case("y")) {
            return;
        }
        let keep = home().join("deleted").join(format!("{}_{}", file_name(&s.name), stamp()));
        self.note = match delete_setup_files(&m, &disk(), &keep, &s.name, s.backup) {
            Ok(k) => format!("{} deleted from the calculator ({k} files); a copy is in {}", s.name, keep.display()),
            Err(e) => e,
        };
        self.setups = modules::setups(&disk());
        let i = self.sel.min(self.setups.len().saturating_sub(1));
        self.sel = i;
        self.pick_setup(i);
    }

    fn working_copy(&self, p: &Program) -> PathBuf {
        home().join("prog").join(format!("{}.txt", file_name(&p.name())))
    }

    /// The listing to edit, run or save: the working copy when there is
    /// one, else the program as the calculator has it, written out first.
    fn listing_file(&mut self) -> Option<(String, PathBuf)> {
        let p = self.progs.get(self.sel)?;
        let file = self.working_copy(p);
        if !file.exists() {
            let made = std::fs::create_dir_all(home().join("prog"))
                .and_then(|_| std::fs::write(&file, focal::listing(&p.steps)));
            if let Err(e) = made {
                self.note = format!("{}: {e}", file.display());
                return None;
            }
        }
        Some((p.name(), file))
    }

    /// Hand the terminal to another program, then take it back.
    fn run_child(&mut self, cmd: &str, file: &Path) -> bool {
        Crust::cleanup();
        let ok = std::process::Command::new(cmd).arg(file).status().is_ok();
        Crust::init();
        Crust::set_app_identity("dm41");
        self.layout();
        if !ok {
            self.note = format!("could not start {cmd}");
        }
        ok
    }

    fn edit(&mut self) {
        let Some((name, file)) = self.listing_file() else {
            return;
        };
        if self.run_child("scribe", &file) {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            self.note = save_raw(&name, &text, &self.names);
            // a setup that has this program gets the new version
            let has = |m: &State| m.program_list(&self.names).iter().any(|(p, _)| p.name() == name);
            if self.mem.as_ref().is_some_and(has) {
                match focal::encode(&text, &self.names) {
                    Ok(b) => self.put_program(state::program_bytes(&b)),
                    Err(e) => self.note = format!("{name}: {e}"),
                }
            }
        }
    }

    fn run_rpnx(&mut self) {
        if let Some((_, file)) = self.listing_file() {
            self.run_child("rpnx", &file);
        }
    }

    fn save(&mut self) {
        let Some(p) = self.progs.get(self.sel) else {
            return;
        };
        let file = self.working_copy(p);
        let text = std::fs::read_to_string(&file).unwrap_or_else(|_| focal::listing(&p.steps));
        self.note = save_raw(&p.name(), &text, &self.names);
    }

    fn render_top(&mut self) {
        let mut parts = vec![style::fg(&style::bold(" dm41"), GOLD)];
        for (t, name) in TABS {
            let label = format!(" {name} ");
            // fb ends by resetting the background, so the bar's own comes back after it
            parts.push(if t == self.tab {
                style::fb(&style::bold(&label), 16, GOLD) + &style::set_bg(HEAD_BG as u8)
            } else {
                style::fg(&label, 250)
            });
        }
        let path = self.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let file = match (self.tab, self.setups.get(self.setup)) {
            (Tab::Modules | Tab::Setups, Some(s)) => format!("setup {}", s.name),
            (Tab::Programs | Tab::Xm | Tab::Flags, Some(s)) if self.mem.is_some() => format!("{path}  setup {}", s.name),
            _ => path,
        };
        parts.push(style::fg(&format!("  {file}"), DIM));
        let w = self.top.w as usize;
        self.top.set_text(&crust::with_version(&parts.join(" "), w, "dm41", env!("CARGO_PKG_VERSION")));
        self.top.refresh();
    }

    fn render_foot(&mut self) {
        let keys = match self.tab {
            Tab::Programs => "e edit  r run in rpnx  s save .raw  l load a file  R read again  q quit",
            Tab::Xm => "e edit a text file  l load a text file  ↑↓ pick  q quit",
            Tab::Library => "l load it into the setup  ↑↓ pick  PgUp/PgDn scroll  R read again  q quit",
            Tab::Modules => "Space plug in or out of the setup  ↑↓ pick  PgUp/PgDn scroll  q quit",
            Tab::Setups => "n new setup from this one  s copy it to the calculator  D delete it  ↑↓ pick  q quit",
            _ => "←→ tab  ↑↓ pick  PgUp/PgDn scroll  B back up the disk  R read again  q quit",
        };
        let keys = match self.tab {
            Tab::Programs | Tab::Xm | Tab::Flags if self.mem.is_some() => format!("Space in or out of the setup  {keys}"),
            _ => keys.to_string(),
        };
        let text = if self.note.is_empty() { style::fg(&keys, DIM) } else { style::fg(&self.note, GOLD) };
        self.foot.set_text(&format!(" {text}"));
        self.foot.refresh();
    }

    /// The left side's list, each line in its own colour.
    fn list(&self, items: Vec<(String, u8)>) -> String {
        let w = self.left.w as usize;
        items
            .iter()
            .enumerate()
            .map(|(i, (s, c))| {
                let s: String = format!(" {s}").chars().take(w).collect();
                let pad = format!("{s:w$}");
                if i == self.sel {
                    style::fb(&style::bold(&pad), 255, SEL_BG)
                } else {
                    style::fg(&pad, *c)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_body(&mut self) {
        let (left, right) = match self.tab {
            Tab::Programs => {
                let items = self.progs.iter().enumerate().map(|(i, p)| self.marked(&self.pmarks, i, p.name())).collect();
                let right = match self.progs.get(self.sel) {
                    Some(p) => {
                        let mut head = format!("{}  {} lines", style::bold(&p.name()), p.steps.len());
                        head.push_str(&self.where_is(&self.pmarks));
                        if p.labels.len() > 1 {
                            head.push_str(&format!("  labels {}", p.labels.join(" ")));
                        }
                        if self.working_copy(p).exists() {
                            head.push_str(&style::fg("  working copy in ~/.dm41/prog", GOLD));
                        }
                        format!("{head}\n\n{}", numbered(&p.steps).join("\n"))
                    }
                    None => "No programs in memory.".into(),
                };
                (self.list(items), right)
            }
            Tab::Xm => {
                let items = self
                    .xm
                    .iter()
                    .enumerate()
                    .map(|(i, f)| self.marked(&self.xmarks, i, format!("{:<8}{}", f.name, f.kind_name())))
                    .collect();
                let right = match self.xm.get(self.sel) {
                    Some(f) => {
                        let lines = state::xm_lines(f, &self.names);
                        let lines: Vec<String> =
                            if f.kind == 1 { lines.iter().map(|l| colour_step(l)).collect() } else { lines };
                        let head = format!(
                            "{}  {} file, {} registers{}",
                            style::bold(&f.name),
                            f.kind_name(),
                            f.regs.len(),
                            self.where_is(&self.xmarks)
                        );
                        format!("{head}\n\n{}", lines.join("\n"))
                    }
                    None => "No files in X-Memory.".into(),
                };
                (self.list(items), right)
            }
            Tab::Modules => self.render_modules(),
            Tab::Setups => self.render_setups(),
            Tab::Keys => self.render_keys(),
            Tab::Library => self.render_library(),
            Tab::Registers => {
                let mut left: Vec<String> = self
                    .st
                    .stack()
                    .iter()
                    .map(|(c, v)| format!(" {}  {}", style::fg(&c.to_string(), GOLD), show(*v)))
                    .collect();
                left.push(String::new());
                left.push(format!(" {}", style::fg("ALPHA", GOLD)));
                left.push(format!(" {}", self.st.alpha()));
                left.push(String::new());
                left.push(format!(" {}  {:03}", style::fg("SIZE", GOLD), self.st.size()));
                let right: Vec<String> = (0..self.st.size())
                    .map(|n| format!("{}  {}", style::fg(&format!("R{n:02}"), GOLD), show(self.st.data(n))))
                    .collect();
                (left.join("\n"), right.join("\n"))
            }
            Tab::Flags => {
                let mut left = flag_summary(&self.st);
                let mut right = vec![];
                if let (Some(m), Some(s)) = (&self.mem, self.setups.get(self.setup)) {
                    left.insert(0, style::fg(" Calculator", 250));
                    left.extend([String::new(), style::fg(&format!(" Setup {}", s.name), 117)]);
                    left.extend(flag_summary(m));
                    right.push(format!("   {} calculator  {} setup {}", style::fg("●", 114), style::fg("●", 117), s.name));
                    right.push(String::new());
                }
                let mark = |on: bool, c| if on { ("●", c) } else { ("·", DIM) };
                for n in 0..56 {
                    let mut marks = vec![mark(self.st.flag(n), 114)];
                    marks.extend(self.mem.as_ref().map(|m| mark(m.flag(n), 117)));
                    let line = if n as usize == self.sel {
                        let plain: Vec<&str> = marks.iter().map(|(m, _)| *m).collect();
                        let w = self.right.w as usize;
                        style::fb(&style::bold(&format!("{:w$}", format!("{n:02} {}  {}", plain.join(" "), flag_name(n)))), 255, SEL_BG)
                    } else {
                        let coloured: Vec<String> = marks.iter().map(|(m, c)| style::fg(m, *c)).collect();
                        format!("{} {}  {}", style::fg(&format!("{n:02}"), GOLD), coloured.join(" "), flag_name(n))
                    };
                    right.push(line);
                }
                (left.join("\n"), right.join("\n"))
            }
        };
        if self.left.text() != left {
            self.left.set_text(&left);
            self.left.ix = 0;
        }
        if self.right.text() != right {
            self.right.set_text(&right);
            self.right.ix = 0;
            if self.tab == Tab::Flags {
                // keep the picked flag in view
                let row = self.sel + if self.mem.is_some() { 2 } else { 0 };
                self.right.ix = (row + 1).saturating_sub(self.right.h as usize);
            }
        }
        self.left.refresh();
        self.right.refresh();
    }

    /// A list line with a mark when the picked setup has a memory: in it
    /// (green), only in it (the setup's blue), or not in it.
    fn marked(&self, marks: &[u8], i: usize, text: String) -> (String, u8) {
        match (self.mem.is_some(), marks.get(i)) {
            (false, _) => (text, 252),
            (true, Some(1)) => (format!("● {text}"), 114),
            (true, Some(2)) => (format!("● {text}"), 117),
            (true, _) => (format!("  {text}"), 252),
        }
    }

    /// Where the picked program or file is, said after its name.
    fn where_is(&self, marks: &[u8]) -> String {
        let Some(s) = self.setups.get(self.setup).filter(|_| self.mem.is_some()) else { return String::new() };
        match marks.get(self.sel) {
            Some(1) => style::fg(&format!("  in setup {}", s.name), 114),
            Some(2) => style::fg(&format!("  only in setup {}, not on the calculator", s.name), 117),
            Some(_) => style::fg(&format!("  not in setup {}", s.name), DIM),
            None => String::new(),
        }
    }

    /// How a setup has a module: plugged in, only in flash, or not at all.
    fn mark(&self, file: &str) -> (&'static str, u8) {
        match self.setups.get(self.setup).and_then(|s| s.mark(file)) {
            Some('A') => ("●", 114),
            Some(_) => ("○", GOLD),
            None => (" ", 250),
        }
    }

    fn render_modules(&self) -> (String, String) {
        let items = self
            .mods
            .iter()
            .map(|m| {
                let (mark, c) = self.mark(&m.file);
                let ids: Vec<String> = m.xroms().iter().map(|x| x.to_string()).collect();
                (format!("{mark} {:<12} {}", m.stem(), ids.join(" ")), c)
            })
            .collect();
        let Some(m) = self.mods.get(self.sel) else {
            return (self.list(items), "No modules: mount the calculator's disk once, and dm41 keeps a copy.".into());
        };
        let setup = self.setups.get(self.setup).map_or("", |s| s.name.as_str());
        let (_, c) = self.mark(&m.file);
        let how = match self.setups.get(self.setup).and_then(|s| s.mark(&m.file)) {
            Some('A') => format!("plugged in by {setup}"),
            Some(_) => format!("in flash with {setup}, not plugged in"),
            None => format!("not in {setup}"),
        };
        let mut out = vec![
            style::bold(if m.title.is_empty() { m.stem() } else { &m.title }),
            style::fg(
                &[m.file.as_str(), &m.version, &m.author]
                    .iter()
                    .filter(|x| !x.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("  "),
                DIM,
            ),
            style::fg(&how, c),
        ];
        for r in &m.roms {
            out.push(String::new());
            let bank = if r.bank > 1 { format!("  bank {}", r.bank) } else { String::new() };
            if r.funcs.is_empty() {
                out.push(style::fg(&format!("{}{bank}: no function table", r.name), DIM));
                continue;
            }
            out.push(style::fg(&style::bold(&format!("XROM {}  {}{bank}", r.xrom, r.funcs[0])), GOLD));
            for (f, n) in r.funcs.iter().enumerate().skip(1) {
                let n = match n.as_str() {
                    "" => style::fg("?", DIM),
                    h if h.starts_with('-') => style::fg(h, GOLD),
                    _ => n.clone(),
                };
                out.push(format!("{}  {n}", style::fg(&format!("{:02},{f:02}", r.xrom), DIM)));
            }
        }
        (self.list(items), out.join("\n"))
    }

    fn render_library(&self) -> (String, String) {
        let files = self.lib.as_deref().unwrap_or_default();
        let items = files
            .iter()
            .map(|p| (p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), 252))
            .collect();
        let Some(path) = files.get(self.sel) else {
            let none = "No programs: name folders with .41 listings or .raw files in ~/.dm41/library, one a line.";
            return (self.list(items), none.into());
        };
        let home = std::env::var("HOME").unwrap_or_default();
        let shown = path.to_string_lossy().replacen(&home, "~", 1);
        let right = match program_file(path, &self.names) {
            Ok(b) => {
                let steps = focal::decode(&b, &self.names);
                let name = prog_name(&b, &self.names);
                let head = format!("{}  {} lines  {}", style::bold(&name), steps.len(), style::fg(&shown, DIM));
                format!("{head}\n\n{}", numbered(&steps).join("\n"))
            }
            Err(e) => {
                let text = std::fs::read_to_string(path).unwrap_or_default();
                format!("{}\n\n{text}", style::fg(&e.replacen(&home, "~", 1), 203))
            }
        };
        (self.list(items), right)
    }

    fn render_keys(&self) -> (String, String) {
        let items = self
            .keys
            .iter()
            .map(|a| (format!("{:>4}  {}", a.code, a.name), if a.program { 114 } else if a.code < 0 { GOLD } else { 252 }))
            .collect();
        let programs = self.keys.iter().filter(|a| a.program).count();
        let mut out = vec![
            format!(
                "{}  {} on keys",
                style::bold("USER mode"),
                [(self.keys.len() - programs, "function"), (programs, "program")]
                    .map(|(n, w)| format!("{n} {w}{}", if n == 1 { "" } else { "s" }))
                    .join(" and ")
            ),
            style::fg("Gold above a key is shifted; green is a program.", DIM),
            String::new(),
        ];
        let picked = self.keys.get(self.sel).map(|a| a.code);
        out.extend(keys::board(&self.keys, picked, self.right.w as usize));
        for (file, keys) in &self.cst {
            out.push(style::fg(&style::bold(&format!(" CST menu  KEYS/{file}")), GOLD));
            let w = (self.right.w as usize).saturating_sub(2) / 4;
            for four in keys.iter().filter(|(k, _)| k.len() == 1 && k.as_bytes()[0].is_ascii_uppercase()).collect::<Vec<_>>().chunks(4) {
                let line: String = four
                    .iter()
                    .map(|(k, f)| {
                        let f: String = if f.is_empty() { "·".into() } else { f.chars().take(w.saturating_sub(3)).collect() };
                        format!("{} {f:<pad$}", style::fg(k, DIM), pad = w.saturating_sub(2))
                    })
                    .collect();
                out.push(format!(" {line}"));
            }
            for (k, f) in keys.iter().filter(|(_, f)| !f.is_empty()) {
                let shift = match k.as_str() {
                    "1" => "SHIFT ▲",
                    "2" => "SHIFT ▼",
                    "3" => "SHIFT α",
                    _ => continue,
                };
                out.push(format!(" {} {f}", style::fg(shift, DIM)));
            }
            out.push(String::new());
        }
        (self.list(items), out.join("\n"))
    }

    fn render_setups(&self) -> (String, String) {
        let items = self
            .setups
            .iter()
            .map(|s| if s.backup { (format!("{:<12} backup", s.name), 250) } else { (s.name.clone(), 117) })
            .collect();
        let Some(s) = self.setups.get(self.sel) else {
            return (self.list(items), "No setups: the calculator writes one with each full backup.".into());
        };
        let on = s.lines.iter().filter(|(k, _)| *k == 'A').count();
        let mut out = vec![format!("{}  {on} modules plugged in", style::bold(&s.name))];
        if s.backup {
            out.push(style::fg(
                "Part of a full backup: Settings menu, Restore from Backup loads it with its memory.",
                DIM,
            ));
            out.push(style::fg("n makes a copy to change.", DIM));
        } else {
            let set = self.mem.as_ref().filter(|_| set_memory(s).is_some());
            let (dir, file) = if set.is_some() { ("BACKUP", "b41") } else { ("MODS", "m41") };
            if let Some(m) = set {
                let (p, x) = (m.program_list(&self.names).len(), m.xm_files().len());
                out.push(format!("Its memory: {p} programs, {x} X-Memory files"));
                out.push(style::fg(&format!("Restore it with Settings menu, Restore from Backup, then {}.", s.name), DIM));
            } else {
                out.push(style::fg("Load it with Module menu, Load Modules, Load Flash and AM Lists.", DIM));
                out.push(style::fg("Space in Programs, X-Memory or Flags gives it a memory of its own.", DIM));
            }
            let there = match mount() {
                Some((m, _)) if m.join(dir).join(format!("{}.{file}", s.name)).exists() => "on the calculator's disk",
                Some(_) => "not on the calculator's disk yet: s copies it there",
                None => "the calculator's disk is not mounted",
            };
            out.push(style::fg(there, DIM));
        }
        out.push(String::new());
        for (k, entry) in &s.lines {
            let file = entry.rsplit('/').next().unwrap_or(entry);
            let m = self.mods.iter().find(|m| m.file.eq_ignore_ascii_case(file));
            let (mark, c) = if *k == 'A' { ("●", 114) } else { ("○", GOLD) };
            let stem = m.map_or(file, |m| m.stem());
            let ids: Vec<String> = m.map(|m| m.xroms().iter().map(|x| x.to_string()).collect()).unwrap_or_default();
            let title = m.map_or("", |m| m.title.as_str());
            out.push(format!("{} {:<12} {:<6} {}", style::fg(mark, c), stem, ids.join(" "), style::fg(title, DIM)));
        }
        (self.list(items), out.join("\n"))
    }

    fn render(&mut self) {
        self.render_top();
        self.render_body();
        self.render_foot();
    }

    fn switch(&mut self, step: isize) {
        let i = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0) as isize;
        self.go(TABS[(i + step).rem_euclid(TABS.len() as isize) as usize].0);
    }

    fn go(&mut self, tab: Tab) {
        self.tab = tab;
        self.sel = if tab == Tab::Setups { self.setup } else { 0 };
        if tab == Tab::Library && self.lib.is_none() {
            self.lib = Some(library_files(&home().join("library")));
        }
    }

    fn run(&mut self) {
        self.render();
        loop {
            let Some(key) = Input::getchr(None) else {
                continue;
            };
            let note = std::mem::take(&mut self.note);
            match key.as_str() {
                "q" => break,
                "LEFT" | "S-TAB" => self.switch(-1),
                "RIGHT" | "TAB" => self.switch(1),
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" => self.go(TABS[key.parse::<usize>().unwrap_or(1) - 1].0),
                "UP" if self.count() > 0 => self.sel = self.sel.saturating_sub(1),
                "DOWN" if self.count() > 0 => self.sel = (self.sel + 1).min(self.count() - 1),
                "UP" => self.right.lineup(),
                "DOWN" => self.right.linedown(),
                "PgUP" => self.right.pageup(),
                "PgDOWN" => self.right.pagedown(),
                "HOME" => self.right.top(),
                "END" => self.right.bottom(),
                "e" if self.tab == Tab::Programs => self.edit(),
                "e" if self.tab == Tab::Xm => self.edit_text(),
                "l" if self.tab == Tab::Programs => self.load_program(),
                "l" if self.tab == Tab::Xm => self.load_text(),
                "l" if self.tab == Tab::Library => self.load_library(),
                "B" => self.note = backup_disk(),
                "r" if self.tab == Tab::Programs => self.run_rpnx(),
                "s" if self.tab == Tab::Programs => self.save(),
                " " if self.tab == Tab::Modules => self.toggle(),
                " " if matches!(self.tab, Tab::Programs | Tab::Xm | Tab::Flags) => self.pick(),
                "n" if self.tab == Tab::Setups => self.new_setup(),
                "D" if self.tab == Tab::Setups => self.delete_setup(),
                "s" if self.tab == Tab::Setups => {
                    if let Some(s) = self.setups.get(self.sel) {
                        self.note = match (s.backup, set_memory(s)) {
                            (true, _) => format!("{} is on the disk already", s.name),
                            (false, Some(mem)) => {
                                put_setup(s);
                                put_set(s, &mem, self.newest_backup().as_deref())
                            }
                            (false, None) => put_setup(s),
                        };
                    }
                }
                "R" => self.reload(),
                "RESIZE" => self.layout(),
                _ if note.is_empty() => continue,
                _ => {}
            }
            if self.tab == Tab::Setups && self.sel != self.setup {
                self.pick_setup(self.sel);
            }
            self.render();
        }
    }
}

/// Make the off pictures when the calculator's disk is mounted for
/// writing and ~/.dm41/off asks for some; a note on what was written.
fn off_pictures() -> Option<String> {
    match mount() {
        Some((m, true)) => off::refresh(&m.join("OFFIMG")),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().position(|a| a == f);
    if flag("-h").or(flag("--help")).is_some() {
        println!("{HELP}");
        return;
    }
    if flag("-v").or(flag("--version")).is_some() {
        println!("dm41 {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let fail = |e: String| -> ! {
        eprintln!("dm41: {e}");
        std::process::exit(1)
    };
    if flag("--unmount").is_some() {
        match detach() {
            Ok(_) => println!("unmounted: the calculator can be unplugged"),
            Err(e) => fail(e),
        }
        return;
    }
    if flag("--mount").is_some() {
        attach();
        match mount() {
            Some((m, rw)) => println!("{} {}", m.display(), if rw { "for writing" } else { "read-only" }),
            None => fail("not mounted: is the calculator in USB disk mode, and the sudoers line in place?".into()),
        }
        return;
    }
    if let Some(i) = flag("--off") {
        let (dir, did) = match args.get(i + 1) {
            Some(d) => (PathBuf::from(d), false),
            None => {
                let did = attach();
                match mount() {
                    Some((m, true)) => (m.join("OFFIMG"), did),
                    _ => fail("the calculator's disk is not mounted for writing: plug it in, or give a folder".into()),
                }
            }
        };
        println!("{}", off::refresh(&dir).unwrap_or_else(|| "no ~/.dm41/off, so no pictures (see the README)".into()));
        if did {
            if let Err(e) = detach() {
                eprintln!("dm41: {e}");
            }
        }
        return;
    }
    let tui = !args.iter().any(|a| a.starts_with("--"));
    let attached = tui && attach();
    if let Some((m, _)) = mount() {
        mirror(&m);
    }
    let off = if tui { off_pictures() } else { None };
    let mods = modules::read_all(&disk().join("MODS"));
    let setups = modules::setups(&disk());
    let names = modules::names(&mods, setups.get(newest_setup(&setups)));
    if let Some(i) = flag("--encode") {
        let file = args.get(i + 1).unwrap_or_else(|| fail("--encode needs a listing file".into()));
        let bytes = program_file(Path::new(file), &names).unwrap_or_else(|e| fail(e));
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
        return;
    }
    let state_arg = args.iter().find(|a| a.to_lowercase().ends_with(".d41")).map(String::as_str);
    let (path, st) = load(state_arg).unwrap_or_else(|e| fail(e));
    let progs = st.programs(&names);
    let find = |i: usize| args.get(i + 1).unwrap_or_else(|| fail("which one? Give a name".into())).clone();
    if flag("--list").is_some() {
        for p in &progs {
            println!("{}", focal::listing(&p.steps));
        }
        return;
    }
    if let Some(i) = flag("--raw") {
        let name = find(i);
        let p = progs.iter().find(|p| p.labels.contains(&name)).unwrap_or_else(|| fail(format!("no program {name}")));
        let bytes = focal::encode(&focal::listing(&p.steps), &names).unwrap_or_else(|e| fail(e));
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
        return;
    }
    if let Some(i) = flag("--xm") {
        let name = find(i);
        let f = st.xm_files().into_iter().find(|f| f.name == name).unwrap_or_else(|| fail(format!("no file {name}")));
        for l in state::xm_lines(&f, &names) {
            println!("{l}");
        }
        return;
    }
    Crust::init();
    Crust::set_app_identity("dm41");
    let mut app = App::new(path, st, mods, setups, attached);
    if let Some(n) = off {
        app.note = n;
    }
    app.run();
    Crust::cleanup();
    Crust::clear_screen();
    if app.attached {
        match detach() {
            Ok(_) => println!("dm41: unmounted, the calculator can be unplugged"),
            Err(e) => eprintln!("dm41: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_finds_programs_in_folders_and_their_subfolders() {
        let root = std::env::temp_dir().join(format!("dm41-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in ["a/b.41", "a/one/c.RAW", "a/one/deep/d.41", "a/.git/e.41", "a/notes.txt", "x/f.41"] {
            std::fs::create_dir_all(root.join(f).parent().unwrap()).unwrap();
            std::fs::write(root.join(f), "01 LBL \"T\"\n02 END\n").unwrap();
        }
        let cfg = root.join("library");
        std::fs::write(&cfg, format!("# my programs\n{}\n\n", root.join("a").display())).unwrap();
        let got: Vec<String> = library_files(&cfg).iter().map(|p| p.strip_prefix(&root).unwrap().display().to_string()).collect();
        assert_eq!(got, ["a/b.41", "a/one/c.RAW"]);
        assert!(library_files(&root.join("none")).is_empty());
        // a listing with a byte order mark still makes bytes
        std::fs::write(root.join("bom.41"), "\u{feff}01 LBL \"T\"\n02 END\n").unwrap();
        assert!(program_file(&root.join("bom.41"), &focal::builtin_names()).is_ok());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn delete_moves_only_that_setup_and_keeps_a_copy() {
        let root = std::env::temp_dir().join(format!("dm41-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (m, disk, keep) = (root.join("calc"), root.join("disk"), root.join("keep"));
        for top in [&m, &disk] {
            for f in ["BACKUP/old.b41", "BACKUP/old.d41", "BACKUP/OLD.ram", "BACKUP/oldx.d41", "BACKUP/mine.d41", "MODS/old.m41", "MODS/mine.m41"] {
                std::fs::create_dir_all(top.join(f).parent().unwrap()).unwrap();
                std::fs::write(top.join(f), f).unwrap();
            }
        }
        // a full backup: its BACKUP files go, MODS/old.m41 and oldx stay
        assert_eq!(delete_setup_files(&m, &disk, &keep, "old", true), Ok(3));
        for top in [&m, &disk] {
            assert!(!top.join("BACKUP/old.b41").exists() && !top.join("BACKUP/OLD.ram").exists());
            assert!(top.join("BACKUP/oldx.d41").exists() && top.join("MODS/old.m41").exists());
        }
        assert_eq!(std::fs::read_to_string(keep.join("BACKUP/old.d41")).unwrap(), "BACKUP/old.d41");
        // a setup of dm41's own: MODS/NAME.m41 goes too
        assert_eq!(delete_setup_files(&m, &disk, &root.join("keep2"), "mine", false), Ok(2));
        assert!(!m.join("MODS/mine.m41").exists() && !disk.join("BACKUP/mine.d41").exists());
        assert!(delete_setup_files(&m, &disk, &keep, "gone", true).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn steps_take_the_colours_of_hp41_vim() {
        for (step, c) in [
            ("LBL \"CLR\"", GOLD),
            ("\"⊢ M2\"", 114),
            ("\"R?\"", 114),
            ("GTO 00", 177),
            (".END.", 203),
            ("PROMPT", 203),
            ("X>Y?", 208),
            ("FS?C 25", 208),
            ("ISG 07", 208),
            ("STO IND X", 111),
            ("X<> 05", 111),
            ("X<>Y", 252),
            ("STOFLAG", 252),
            ("-", 252),
            ("-1.5E-3", 81),
            ("E3", 81),
            ("XROM 05,03", DIM),
        ] {
            assert_eq!(step_colour(step), c, "{step}");
        }
    }

    /// Every program in the last state read on this machine encodes to a
    /// .raw file that decodes to the same lines, with the names of the
    /// modules it uses. Without a state, nothing to check.
    #[test]
    fn programs_on_the_calculator_encode_back() {
        let Ok(text) = std::fs::read_to_string(home().join("last.d41")) else {
            return;
        };
        let setups = modules::setups(&disk());
        let names = modules::names(&modules::read_all(&disk().join("MODS")), setups.get(newest_setup(&setups)));
        for p in State::parse(&text).unwrap().programs(&names) {
            let bytes = focal::encode(&focal::listing(&p.steps), &names).unwrap();
            let back: Vec<String> = focal::decode(&bytes, &names).into_iter().map(|s| s.text).collect();
            let mut want: Vec<String> = p.steps.iter().map(|s| s.text.clone()).collect();
            *want.last_mut().unwrap() = ".END.".into();
            assert_eq!(back, want, "{}", p.name());
        }
    }

    #[test]
    fn program_names_make_safe_file_names() {
        assert_eq!(file_name("AB*"), "AB_");
        assert_eq!(file_name("A/B"), "A_B");
        assert_eq!(file_name(""), "PROG");
    }
}
