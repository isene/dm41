mod focal;
mod state;

use crust::{style, Crust, Input, Pane};
use focal::Names;
use state::{show, Program, State, XmFile};
use std::path::{Path, PathBuf};

const HELP: &str = "dm41: the memory of a SwissMicros DM41X in the terminal

  dm41                      the newest state on the calculator's disk, or the last copy
  dm41 FILE.d41             that state file
  dm41 --list [FILE]        every program as a listing
  dm41 --raw NAME [FILE]    one program as a .raw file, on stdout
  dm41 --xm NAME [FILE]     what an X-Memory file holds
  dm41 --encode LISTING     a listing as a .raw file, on stdout
  -h, --help   -v, --version

Keys: Left/Right tab, Up/Down pick, PgUp/PgDn scroll
  e  edit the program in scribe; NAME.raw is saved when scribe ends
  r  run the program in rpnx      s  save NAME.raw      R  read the state again
  q  quit

The calculator shows its disk in USB disk mode (File menu). dm41 reads the
newest .d41 in its STATE folder and keeps a copy in ~/.dm41/last.d41.
Listings and .raw files go to ~/.dm41/prog; a .raw also goes to the
calculator's PROG folder when its disk is mounted for writing. Load it on
the calculator with File menu, Load RAW Program.";

const HEAD_BG: u16 = 236;
const PANEL_BG: u16 = 234;
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

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".dm41")
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
        let _ = std::fs::create_dir_all(home()).and_then(|_| std::fs::copy(&p, &last));
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
        Some((_, false)) => format!("{file}: {} bytes in ~/.dm41/prog (the calculator's disk is read-only)", bytes.len()),
        None => format!("{file}: {} bytes in ~/.dm41/prog (mount the calculator's disk to copy it there)", bytes.len()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Programs,
    Registers,
    Flags,
    Xm,
}

const TABS: [(Tab, &str); 4] =
    [(Tab::Programs, "Programs"), (Tab::Registers, "Registers"), (Tab::Flags, "Flags"), (Tab::Xm, "X-Memory")];

struct App {
    path: PathBuf,
    st: State,
    names: Names,
    progs: Vec<Program>,
    xm: Vec<XmFile>,
    tab: Tab,
    sel: usize,
    note: String,
    top: Pane,
    left: Pane,
    right: Pane,
    foot: Pane,
}

/// A listing line coloured by what it does.
fn colour_step(text: &str) -> String {
    let c = if text.starts_with("LBL") {
        return style::fg(&style::bold(text), GOLD);
    } else if text.starts_with('"') {
        114
    } else if text.starts_with("GTO") || text.starts_with("XEQ") {
        177
    } else if text == "END" || text == ".END." {
        203
    } else if text.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.')
        || text.starts_with('E') && text[1..].starts_with(|c: char| c.is_ascii_digit() || c == '-')
    {
        81
    } else if text.starts_with("XROM") {
        DIM
    } else {
        252
    };
    style::fg(text, c)
}

impl App {
    fn new(path: PathBuf, st: State) -> App {
        let names = focal::builtin_names();
        let blank = || Pane::new(1, 1, 1, 1, 252, PANEL_BG);
        let mut app = App {
            progs: st.programs(&names),
            xm: st.xm_files(),
            path,
            st,
            names,
            tab: Tab::Programs,
            sel: 0,
            note: String::new(),
            top: blank(),
            left: blank(),
            right: blank(),
            foot: blank(),
        };
        app.layout();
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
        match load(None) {
            Ok((p, s)) => {
                self.progs = s.programs(&self.names);
                self.xm = s.xm_files();
                self.path = p;
                self.st = s;
                self.sel = 0;
                self.note = format!("read {}", self.path.display());
            }
            Err(e) => self.note = e,
        }
    }

    fn count(&self) -> usize {
        match self.tab {
            Tab::Programs => self.progs.len(),
            Tab::Xm => self.xm.len(),
            _ => 0,
        }
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
        let Some((name, file)) = self.listing_file() else { return };
        if self.run_child("scribe", &file) {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            self.note = save_raw(&name, &text, &self.names);
        }
    }

    fn run_rpnx(&mut self) {
        if let Some((_, file)) = self.listing_file() {
            self.run_child("rpnx", &file);
        }
    }

    fn save(&mut self) {
        let Some(p) = self.progs.get(self.sel) else { return };
        let file = self.working_copy(p);
        let text = std::fs::read_to_string(&file).unwrap_or_else(|_| focal::listing(&p.steps));
        self.note = save_raw(&p.name(), &text, &self.names);
    }

    fn render_top(&mut self) {
        let mut parts = vec![style::fg(&style::bold(" dm41"), GOLD)];
        for (t, name) in TABS {
            let label = format!(" {name} ");
            parts.push(if t == self.tab { style::fb(&style::bold(&label), 16, GOLD) } else { style::fg(&label, 250) });
        }
        let file = self.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        parts.push(style::fg(&format!("  {file}"), DIM));
        let w = self.top.w as usize;
        self.top.set_text(&crust::with_version(&parts.join(" "), w, "dm41", env!("CARGO_PKG_VERSION")));
        self.top.refresh();
    }

    fn render_foot(&mut self) {
        let keys = match self.tab {
            Tab::Programs => "e edit  r run in rpnx  s save .raw  R read again  q quit",
            _ => "←→ tab  ↑↓ pick  PgUp/PgDn scroll  R read again  q quit",
        };
        let text = if self.note.is_empty() { style::fg(keys, DIM) } else { style::fg(&self.note, GOLD) };
        self.foot.set_text(&format!(" {text}"));
        self.foot.refresh();
    }

    fn list(&self, items: Vec<String>) -> String {
        let w = self.left.w as usize;
        items
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let s: String = format!(" {s}").chars().take(w).collect();
                let pad = format!("{s:w$}");
                if i == self.sel {
                    style::fb(&style::bold(&pad), 255, SEL_BG)
                } else {
                    pad
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_body(&mut self) {
        let (left, right) = match self.tab {
            Tab::Programs => {
                let items = self.progs.iter().map(|p| p.name()).collect();
                let right = match self.progs.get(self.sel) {
                    Some(p) => {
                        let mut head = format!("{}  {} lines", style::bold(&p.name()), p.steps.len());
                        if p.labels.len() > 1 {
                            head.push_str(&format!("  labels {}", p.labels.join(" ")));
                        }
                        if self.working_copy(p).exists() {
                            head.push_str(&style::fg("  working copy in ~/.dm41/prog", GOLD));
                        }
                        let w = if p.steps.len() >= 100 { 3 } else { 2 };
                        let lines: Vec<String> = p
                            .steps
                            .iter()
                            .enumerate()
                            .map(|(n, s)| format!("{} {}", style::fg(&format!("{:0w$}", n + 1), DIM), colour_step(&s.text)))
                            .collect();
                        format!("{head}\n\n{}", lines.join("\n"))
                    }
                    None => "No programs in memory.".into(),
                };
                (self.list(items), right)
            }
            Tab::Xm => {
                let items = self.xm.iter().map(|f| format!("{:<8}{}", f.name, f.kind_name())).collect();
                let right = match self.xm.get(self.sel) {
                    Some(f) => {
                        let lines = state::xm_lines(f, &self.names);
                        let lines: Vec<String> =
                            if f.kind == 1 { lines.iter().map(|l| colour_step(l)).collect() } else { lines };
                        let head = format!("{}  {} file, {} registers", style::bold(&f.name), f.kind_name(), f.regs.len());
                        format!("{head}\n\n{}", lines.join("\n"))
                    }
                    None => "No files in X-Memory.".into(),
                };
                (self.list(items), right)
            }
            Tab::Registers => {
                let mut left: Vec<String> =
                    self.st.stack().iter().map(|(c, v)| format!(" {}  {}", style::fg(&c.to_string(), GOLD), show(*v))).collect();
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
                let f = |n| self.st.flag(n);
                let digits = (36..40).fold(0, |a, n| a * 2 + f(n) as u32);
                let mode = match (f(40), f(41)) {
                    (true, false) => "FIX",
                    (false, true) => "ENG",
                    _ => "SCI",
                };
                let angle = if f(42) { "GRAD" } else if f(43) { "RAD" } else { "DEG" };
                let left = [
                    format!(" {}  {mode} {digits}", style::fg("Display", GOLD)),
                    format!(" {}  {angle}", style::fg("Angles", GOLD)),
                    format!(" {}  {}", style::fg("USER", GOLD), if f(27) { "on" } else { "off" }),
                ];
                let right: Vec<String> = (0..56)
                    .map(|n| {
                        let (mark, c) = if f(n) { ("●", 114) } else { ("·", DIM) };
                        format!("{} {}  {}", style::fg(&format!("{n:02}"), GOLD), style::fg(mark, c), flag_name(n))
                    })
                    .collect();
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
        }
        self.left.refresh();
        self.right.refresh();
    }

    fn render(&mut self) {
        self.render_top();
        self.render_body();
        self.render_foot();
    }

    fn switch(&mut self, step: isize) {
        let i = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0) as isize;
        self.tab = TABS[(i + step).rem_euclid(TABS.len() as isize) as usize].0;
        self.sel = 0;
    }

    fn run(&mut self) {
        self.render();
        loop {
            let Some(key) = Input::getchr(None) else { continue };
            let note = std::mem::take(&mut self.note);
            match key.as_str() {
                "q" | "ESC" => break,
                "LEFT" | "S-TAB" => self.switch(-1),
                "RIGHT" | "TAB" => self.switch(1),
                "1" | "2" | "3" | "4" => {
                    self.tab = TABS[key.parse::<usize>().unwrap_or(1) - 1].0;
                    self.sel = 0;
                }
                "UP" if self.count() > 0 => self.sel = self.sel.saturating_sub(1),
                "DOWN" if self.count() > 0 => self.sel = (self.sel + 1).min(self.count() - 1),
                "UP" => self.right.lineup(),
                "DOWN" => self.right.linedown(),
                "PgUP" => self.right.pageup(),
                "PgDOWN" => self.right.pagedown(),
                "HOME" => self.right.top(),
                "END" => self.right.bottom(),
                "e" if self.tab == Tab::Programs => self.edit(),
                "r" if self.tab == Tab::Programs => self.run_rpnx(),
                "s" if self.tab == Tab::Programs => self.save(),
                "R" => self.reload(),
                "RESIZE" => self.layout(),
                _ if note.is_empty() => continue,
                _ => {}
            }
            self.render();
        }
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
    let names = focal::builtin_names();
    let fail = |e: String| -> ! {
        eprintln!("dm41: {e}");
        std::process::exit(1)
    };
    if let Some(i) = flag("--encode") {
        let file = args.get(i + 1).unwrap_or_else(|| fail("--encode needs a listing file".into()));
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| fail(format!("{file}: {e}")));
        let bytes = focal::encode(&text, &names).unwrap_or_else(|e| fail(e));
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
    App::new(path, st).run();
    Crust::cleanup();
    Crust::clear_screen();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every program in the last state read on this machine encodes to a
    /// .raw file that decodes to the same lines. Without a state, nothing
    /// to check.
    #[test]
    fn programs_on_the_calculator_encode_back() {
        let Ok(text) = std::fs::read_to_string(home().join("last.d41")) else { return };
        let names = focal::builtin_names();
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
