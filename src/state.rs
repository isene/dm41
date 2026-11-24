//! The DM41X state file (.d41): the calculator's whole memory as hex, one
//! line per four registers, then the CPU registers.

use crate::focal::{self, Names, Step};

/// The memory of an HP-41CX: 1024 registers of 56 bits (7 bytes), and the
/// CPU lines that end the file, kept as they are.
#[derive(Clone)]
pub struct State {
    pub regs: Vec<u64>,
    pub cpu: String,
}

/// A program in main memory: its global labels and its lines.
pub struct Program {
    pub labels: Vec<String>,
    pub steps: Vec<Step>,
}

impl Program {
    pub fn name(&self) -> String {
        let end = self.steps.last().map_or("END", |s| s.text.as_str());
        self.labels.first().cloned().unwrap_or_else(|| end.to_string())
    }
}

/// A file in X-Memory, with its name and header registers as they are.
#[derive(Clone)]
pub struct XmFile {
    pub name: String,
    pub kind: u8,
    pub regs: Vec<u64>,
    pub name_reg: u64,
    pub head: u64,
}

impl XmFile {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            1 => "program",
            2 => "data",
            3 => "text",
            _ => "other",
        }
    }
}

/// Nibbles `hi` down to `lo` of a register.
pub fn field(v: u64, hi: u32, lo: u32) -> u64 {
    (v >> (4 * lo)) & ((1u64 << (4 * (hi - lo + 1))) - 1)
}

fn byte(v: u64, k: u32) -> u8 {
    (v >> (8 * k)) as u8
}

/// The bytes of registers in the order a program or file reads them: from
/// the left byte to the right one in each register.
fn bytes_of(regs: impl Iterator<Item = u64>) -> Vec<u8> {
    regs.flat_map(|v| (0..7).rev().map(move |k| byte(v, k))).collect()
}

/// A register as the calculator shows it: a number, a text, or hex.
pub fn show(v: u64) -> String {
    let d = |n: u32| field(v, n, n) as u8;
    let digits_ok = (3..=12).all(|n| d(n) <= 9) && d(0) <= 9 && d(1) <= 9;
    if matches!(d(13), 0 | 9) && matches!(d(2), 0 | 9) && digits_ok {
        let mant: String = (3..=12).rev().map(|n| (b'0' + d(n)) as char).collect();
        let e = (d(1) * 10 + d(0)) as i32;
        let e = if d(2) == 9 { e - 100 } else { e };
        return number(d(13) == 9, &mant, e);
    }
    if byte(v, 6) == 0x10 {
        let text: Vec<u8> = (0..6).rev().map(|k| byte(v, k)).skip_while(|&b| b == 0).collect();
        return format!("\"{}\"", focal::hp_text(&text));
    }
    format!("{v:014x}")
}

/// A number from its ten mantissa digits and its exponent.
fn number(neg: bool, mant: &str, e: i32) -> String {
    let m = mant.trim_end_matches('0');
    if m.is_empty() {
        return "0".into();
    }
    let sign = if neg { "-" } else { "" };
    let s = if (0..10).contains(&e) {
        let e = e as usize;
        let int: String = format!("{m:0<w$}", w = e + 1)[..=e].to_string();
        let frac = m.get(e + 1..).unwrap_or("");
        if frac.is_empty() {
            int
        } else {
            format!("{int}.{frac}")
        }
    } else if (-5..0).contains(&e) {
        format!("0.{}{m}", "0".repeat((-e - 1) as usize))
    } else {
        let frac = &m[1..];
        let dot = if frac.is_empty() { "" } else { "." };
        format!("{}{dot}{frac}E{e}", &m[..1])
    };
    format!("{sign}{s}")
}

impl State {
    pub fn parse(text: &str) -> Result<State, String> {
        if !text.starts_with("DM41") {
            return Err("not a DM41X state file".into());
        }
        let mut regs = vec![0u64; 1024];
        let mut cpu = String::new();
        for line in text.lines().skip(1) {
            let reg_line = line.split_once("  ").and_then(|(addr, rest)| Some((usize::from_str_radix(addr, 16).ok()?, rest)));
            let Some((a, rest)) = reg_line else {
                cpu.push_str(line);
                cpu.push('\n');
                continue;
            };
            let addr = &line[..line.find(' ').unwrap_or(0)];
            for (k, hex) in rest.split_whitespace().enumerate() {
                let v = u64::from_str_radix(hex, 16).map_err(|_| format!("bad register {hex} at {addr}"))?;
                if let Some(r) = regs.get_mut(a + k) {
                    *r = v;
                }
            }
        }
        Ok(State { regs, cpu })
    }

    /// The state as a .d41 file. Like the calculator, it leaves out each line
    /// whose four registers are all zero.
    pub fn text(&self) -> String {
        let mut out = String::from("DM41\n");
        for (i, four) in self.regs.chunks(4).enumerate().filter(|(_, f)| f.iter().any(|&v| v != 0)) {
            out.push_str(&format!("{:02x}  ", i * 4));
            for v in four {
                out.push_str(&format!("{v:014x}  "));
            }
            out.push('\n');
        }
        out + &self.cpu
    }

    fn reg(&self, a: u64) -> u64 {
        self.regs.get(a as usize).copied().unwrap_or(0)
    }

    /// Register c holds where memory is split up.
    fn c(&self) -> u64 {
        self.reg(0x00D)
    }

    /// The address of R00; the data registers run from here to 0x1FF.
    pub fn curtain(&self) -> u64 {
        field(self.c(), 5, 3)
    }

    /// The register that holds .END., the bottom of program memory.
    fn end_reg(&self) -> u64 {
        field(self.c(), 2, 0)
    }

    pub fn size(&self) -> u64 {
        0x200u64.saturating_sub(self.curtain())
    }

    pub fn data(&self, n: u64) -> u64 {
        self.reg(self.curtain() + n)
    }

    pub fn flag(&self, n: u32) -> bool {
        self.reg(0x00E) >> (55 - n) & 1 == 1
    }

    /// X, Y, Z, T and LASTX.
    pub fn stack(&self) -> [(char, u64); 5] {
        [('X', self.reg(3)), ('Y', self.reg(2)), ('Z', self.reg(1)), ('T', self.reg(0)), ('L', self.reg(4))]
    }

    /// The ALPHA register: up to 24 characters in P, O, N and M.
    pub fn alpha(&self) -> String {
        let mut b = vec![byte(self.reg(8), 2), byte(self.reg(8), 1), byte(self.reg(8), 0)];
        b.extend(bytes_of([7, 6, 5].into_iter().map(|a| self.reg(a))));
        let b: Vec<u8> = b.into_iter().skip_while(|&x| x == 0).collect();
        focal::hp_text(&b)
    }

    /// The program bytes, from the top of program memory down to .END.
    pub fn code(&self) -> Vec<u8> {
        let (top, bottom) = (self.curtain(), self.end_reg());
        if top <= bottom || top > 0x200 {
            return Vec::new();
        }
        bytes_of((bottom..top).rev().map(|a| self.reg(a)))
    }

    pub fn programs(&self, names: &Names) -> Vec<Program> {
        split(focal::decode(&self.code(), names))
    }

    /// Each program's bytes, from the byte after the END above it down to its
    /// own END, so the nulls inside stay. .END. is left out; a last program
    /// that ends at .END. gets an END of its own, as GTO ".." gives it.
    pub fn program_code(&self) -> Vec<Vec<u8>> {
        let code = self.code();
        let mut out = Vec::new();
        let mut start = 0;
        for s in focal::decode(&code, &Names::new()).iter().filter(|s| s.end) {
            if s.text == "END" {
                out.push(code[start..s.at + 3].to_vec());
            } else if code[start..s.at].iter().any(|&b| b != 0) {
                out.push([&code[start..s.at], &[0xC0, 0x00, 0x09][..]].concat());
            }
            start = s.at + 3;
        }
        out
    }

    /// Each program with its bytes, as program_code() gives them.
    pub fn program_list(&self, names: &Names) -> Vec<(Program, Vec<u8>)> {
        self.program_code().into_iter().filter_map(|c| Some((split(focal::decode(&c, names)).into_iter().next()?, c))).collect()
    }

    /// This memory with other programs, in this order, and .END. after them
    /// in the last three bytes of its register. Each global label and END
    /// points at the one above it: the distance in bytes, as registers and
    /// bytes left over. Jumps inside a program count bytes too, so a program
    /// keeps working wherever it lands. The calculator then stands at .END.,
    /// line 00, as after GTO "..". Fails when the programs do not fit.
    pub fn with_programs(&self, progs: &[Vec<u8>]) -> Result<State, String> {
        let (top, old_end) = (self.curtain(), self.end_reg());
        if top <= old_end || top > 0x200 {
            return Err("program memory is not where it should be".into());
        }
        // key assignments and buffers fill registers from 0x0C0 up; the first empty one is free
        let low = (0x0C0..old_end).find(|&a| self.reg(a) == 0).unwrap_or(old_end);
        let mut bytes = progs.concat();
        let n = bytes.len();
        bytes.resize(n + (7 - (n + 3) % 7) % 7, 0);
        bytes.extend([0xC0, 0x00, self.code().last().copied().unwrap_or(0x20)]);
        let need = (bytes.len() / 7) as u64;
        if need > top - low {
            return Err(format!("{} registers too few for these programs", need - (top - low)));
        }
        let mut chain = Vec::new();
        let mut start = 0;
        for p in progs {
            for s in focal::decode(p, &Names::new()) {
                if s.end || s.text.starts_with("LBL \"") {
                    chain.push(start + s.at);
                }
            }
            start += p.len();
        }
        chain.push(bytes.len() - 3);
        for k in 0..chain.len() {
            let d = if k == 0 { 0 } else { chain[k] - chain[k - 1] };
            let link = ((d % 7) << 9) | (d / 7);
            bytes[chain[k]] = 0xC0 | (link >> 8) as u8;
            bytes[chain[k] + 1] = link as u8;
        }
        let mut st = self.clone();
        st.regs[low as usize..top as usize].fill(0);
        for (i, &b) in bytes.iter().enumerate() {
            st.regs[(top - 1) as usize - i / 7] |= (b as u64) << (8 * (6 - i % 7));
        }
        st.regs[0x00D] = (st.regs[0x00D] & !0xFFF) | (top - need);
        let pc = n as i64 - 1; // the last byte of the last END, or the byte above the top
        let (reg, k) = (top as i64 - 1 - pc.div_euclid(7), 6 - pc.rem_euclid(7));
        st.regs[0x00B] = 0;
        st.regs[0x00C] = ((k as u64) << 12) | reg as u64;
        st.regs[0x00F] &= !0xFFF;
        Ok(st)
    }

    pub fn set_flag(&mut self, n: u32, on: bool) {
        let bit = 1u64 << (55 - n);
        if on {
            self.regs[0x00E] |= bit;
        } else {
            self.regs[0x00E] &= !bit;
        }
    }

    /// The X-Memory files. The directory starts at the top of the
    /// X-Functions memory (0x0BF) and runs down, then on into the memory
    /// modules at 0x2EF and 0x3EF. Each file is a name register, a header
    /// register (type and size) and its data. All ones ends the list.
    pub fn xm_files(&self) -> Vec<XmFile> {
        let order = self.xm_order();
        let mut files = Vec::new();
        let mut i = 0;
        while i + 1 < order.len() {
            let name_reg = self.reg(order[i]);
            if name_reg == 0 || name_reg == 0xFF_FFFF_FFFF_FFFF {
                break;
            }
            let head = self.reg(order[i + 1]);
            let size = field(head, 2, 0) as usize;
            let regs = order.iter().skip(i + 2).take(size).map(|&a| self.reg(a)).collect();
            let name = focal::hp_text(&bytes_of(std::iter::once(name_reg))).trim_end().to_string();
            files.push(XmFile { name, kind: field(head, 13, 13) as u8, regs, name_reg, head });
            i += 2 + size;
        }
        files
    }

    /// This memory with other X-Memory files, in this order. A header that
    /// names its own address gets the new one, all ones ends the list, and
    /// the working file keeps its number among the files, or there is none.
    pub fn with_xm(&self, files: &[XmFile]) -> Result<State, String> {
        let order = self.xm_order();
        let need: usize = files.iter().map(|f| 2 + f.regs.len()).sum();
        if need > order.len() {
            return Err(format!("{} registers too few in X-Memory", need - order.len()));
        }
        let mut st = self.clone();
        for &a in &order {
            st.regs[a as usize] = 0;
        }
        let mut i = 0;
        for f in files {
            let at = order[i + 1];
            let head = if field(f.head, 12, 10) != 0 { f.head & !(0xFFF << 40) | at << 40 } else { f.head };
            st.regs[order[i] as usize] = f.name_reg;
            st.regs[at as usize] = head;
            for (k, &v) in f.regs.iter().enumerate() {
                st.regs[order[i + 2 + k] as usize] = v;
            }
            i += 2 + f.regs.len();
        }
        if let Some(&a) = order.get(i) {
            st.regs[a as usize] = 0xFF_FFFF_FFFF_FFFF;
        }
        let old = self.xm_files();
        let w = field(self.reg(0x040), 11, 9) as usize;
        let w = old.get(w.wrapping_sub(1)).and_then(|o| files.iter().position(|f| f.name == o.name)).map_or(0, |p| p + 1);
        st.regs[0x040] = st.regs[0x040] & !(0xFFF << 36) | (w as u64) << 36;
        Ok(st)
    }

    /// The X-Memory registers in the order files fill them.
    fn xm_order(&self) -> Vec<u64> {
        let mut order = Vec::new();
        let (mut top, mut head) = (field(self.reg(0x040), 2, 0), 0x040);
        for _ in 0..3 {
            if top <= head || top > 0x3FF {
                break;
            }
            order.extend((head + 1..=top).rev());
            let next = field(self.reg(head), 5, 3);
            (top, head) = (next, (next & 0xF00) | 1);
        }
        order
    }
}

/// Lines into programs: each END closes one.
pub fn split(steps: Vec<Step>) -> Vec<Program> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for s in steps {
        let end = s.end;
        cur.push(s);
        if end {
            let labels = cur
                .iter()
                .filter_map(|s: &Step| s.text.strip_prefix("LBL \"").map(|l| l.trim_end_matches('"').to_string()))
                .collect();
            out.push(Program { labels, steps: std::mem::take(&mut cur) });
        }
    }
    out
}

/// What an X-Memory file holds, as lines.
/// A text file for X-Memory: each line a record (its length, then its
/// characters), 0xFF after the last, in as few registers as hold them.
pub fn text_file(name: &str, text: &str) -> Result<XmFile, String> {
    let hp = |c: char, what: &str| focal::from_hp_char(c).ok_or(format!("{what}: the calculator has no {c}"));
    let mut b = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let chars = line.replace('\t', " ").chars().map(|c| hp(c, &format!("line {}", i + 1))).collect::<Result<Vec<u8>, _>>()?;
        if chars.len() > 254 {
            return Err(format!("line {} is longer than 254 characters", i + 1));
        }
        b.push(chars.len() as u8);
        b.extend(chars);
    }
    b.push(0xFF);
    b.resize(b.len().div_ceil(7) * 7, 0);
    let pack = |c: &[u8]| c.iter().fold(0u64, |a, &x| a << 8 | x as u64);
    let regs: Vec<u64> = b.chunks(7).map(pack).collect();
    let mut n = [b' '; 7];
    for (k, c) in name.chars().take(7).enumerate() {
        n[k] = hp(c, "the name")?;
    }
    let name = focal::hp_text(&n).trim_end().to_string();
    Ok(XmFile { name, kind: 3, head: 3 << 52 | regs.len() as u64, regs, name_reg: pack(&n) })
}

/// A program's bytes from a .raw file or an encoded listing, up to and with
/// its first END; one without an END gets one.
pub fn program_bytes(raw: &[u8]) -> Vec<u8> {
    match focal::decode(raw, &Names::new()).iter().find(|s| s.end) {
        Some(s) if s.text == "END" => raw[..s.at + 3].to_vec(),
        Some(s) => [&raw[..s.at], &[0xC0, 0x00, 0x09][..]].concat(),
        None => [raw, &[0xC0, 0x00, 0x09][..]].concat(),
    }
}

pub fn xm_lines(f: &XmFile, names: &Names) -> Vec<String> {
    match f.kind {
        1 => focal::decode(&bytes_of(f.regs.iter().copied()), names)
            .iter()
            .take_while_inclusive(|s| !s.end)
            .map(|s| s.text.clone())
            .collect(),
        2 => f.regs.iter().enumerate().map(|(i, &v)| format!("{i:03}  {}", show(v))).collect(),
        3 => text_records(&bytes_of(f.regs.iter().copied())),
        _ => f.regs.iter().map(|v| format!("{v:014x}")).collect(),
    }
}

/// The records of a text file: a length byte, then that many characters;
/// 0xFF ends the file.
fn text_records(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() && b[i] != 0xFF {
        let n = b[i] as usize;
        out.push(focal::hp_text(&b[(i + 1).min(b.len())..(i + 1 + n).min(b.len())]));
        i += 1 + n;
    }
    out
}

trait TakeInclusive: Iterator + Sized {
    fn take_while_inclusive<P: FnMut(&Self::Item) -> bool>(self, p: P) -> TakeIncl<Self, P> {
        TakeIncl { it: self, p, done: false }
    }
}
impl<I: Iterator> TakeInclusive for I {}

struct TakeIncl<I, P> {
    it: I,
    p: P,
    done: bool,
}

impl<I: Iterator, P: FnMut(&I::Item) -> bool> Iterator for TakeIncl<I, P> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        if self.done {
            return None;
        }
        let x = self.it.next()?;
        self.done = !(self.p)(&x);
        Some(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up state: SIZE 2, one program, a flag, a text file in X-Memory.
    fn sample() -> State {
        let mut s = State { regs: vec![0; 1024], cpu: String::new() };
        s.regs[3] = 0x01457400000002; // X = 145.74
        s.regs[2] = 0x95000000000998; // Y = -5E-2
        s.regs[0x00D] = (0x169 << 24) | (0x1FE << 12) | 0x1FC; // curtain 0x1FE, .END. at 0x1FC
        s.regs[0x00E] = 1 << (55 - 21); // flag 21
        // LBL "AB", 2, *, .END. in registers 0x1FD and 0x1FC, left byte first
        s.regs[0x1FD] = u64::from_be_bytes([0, 0xC0, 0x00, 0xF3, 0x00, b'A', b'B', 0x12]);
        s.regs[0x1FC] = u64::from_be_bytes([0, 0x42, 0, 0, 0, 0xC0, 0x00, 0x2D]);
        s.regs[0x1FE] = 0x01250000000003; // R00 = 1250
        s.regs[0x1FF] = u64::from_be_bytes([0, 0x10, 0, 0, 0, b'H', b'I', b'!']); // R01 = "HI!"
        // X-Memory: one text file "T" holding "AB" and "C"
        s.regs[0x040] = 0x0BF;
        s.regs[0x0BF] = u64::from_be_bytes([0, b'T', b' ', b' ', b' ', b' ', b' ', b' ']);
        s.regs[0x0BE] = 0x30000000000001;
        s.regs[0x0BD] = u64::from_be_bytes([0, 2, b'A', b'B', 1, b'C', 0xFF, 0]);
        s.regs[0x0BC] = 0xFF_FFFF_FFFF_FFFF;
        s
    }

    #[test]
    fn numbers_read_as_the_calculator_shows_them() {
        assert_eq!(show(0x01457400000002), "145.74");
        assert_eq!(show(0x95000000000998), "-0.05");
        assert_eq!(show(0x01250000000003), "1250");
        assert_eq!(show(0x01000000000020), "1E20");
        assert_eq!(show(0x01500000000980), "1.5E-20");
        assert_eq!(show(0), "0");
    }

    #[test]
    fn the_sample_state_reads_back() {
        let s = sample();
        assert_eq!(s.size(), 2);
        assert_eq!(show(s.data(0)), "1250");
        assert_eq!(show(s.data(1)), "\"HI!\"");
        assert!(s.flag(21) && !s.flag(20));
        assert_eq!(show(s.stack()[0].1), "145.74");
        let p = s.programs(&Names::new());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name(), "AB");
        let lines: Vec<&str> = p[0].steps.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(lines, ["LBL \"AB\"", "2", "*", ".END."]);
        let xm = s.xm_files();
        assert_eq!(xm.len(), 1);
        assert_eq!((xm[0].name.as_str(), xm[0].kind_name()), ("T", "text"));
        assert_eq!(xm_lines(&xm[0], &Names::new()), ["AB", "C"]);
    }

    #[test]
    fn a_state_file_parses_from_its_text() {
        let s = State::parse("DM41\n00  00000000000000  00000000000000  01436180000001  01457400000002  \nA: 0\n")
            .unwrap();
        assert_eq!(show(s.stack()[1].1), "14.3618");
        assert!(State::parse("hello").is_err());
    }

    /// The global labels and ENDs as the calculator finds them: from .END.
    /// up along the links, as (register, byte) places.
    fn walk_chain(s: &State) -> Vec<(u64, u64)> {
        let at = |i: i64| ((s.curtain() as i64 - 1 - i.div_euclid(7)) as u64, (6 - i.rem_euclid(7)) as u64);
        let byte_at = |i: i64| {
            let (r, k) = at(i);
            byte(s.reg(r), k as u32)
        };
        let top = s.curtain() as i64;
        let mut i = (top - 1 - s.end_reg() as i64) * 7 + 4; // .END. starts at byte 2
        let mut out = vec![at(i)];
        loop {
            let link = ((byte_at(i) as i64 & 0x0F) << 8) | byte_at(i + 1) as i64;
            if link == 0 {
                return out;
            }
            i -= (link & 0x1FF) * 7 + (link >> 9);
            out.push(at(i));
        }
    }

    /// The states read on this machine: the last one and the full backups.
    fn real_states() -> Vec<(String, State)> {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".dm41");
        let mut files = vec![home.join("last.d41")];
        if let Ok(rd) = std::fs::read_dir(home.join("disk/BACKUP")) {
            files.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "d41")));
        }
        files.iter().filter_map(|p| Some((std::fs::read_to_string(p).ok()?, p))).map(|(t, p)| {
            assert_eq!(State::parse(&t).unwrap().text(), t, "{} writes back as it was", p.display());
            (p.display().to_string(), State::parse(&t).unwrap())
        }).collect()
    }

    #[test]
    fn a_state_writes_back_as_it_was_read() {
        let s = sample();
        let back = State::parse(&s.text()).unwrap();
        assert_eq!(back.regs, s.regs);
        real_states();
    }

    #[test]
    fn the_same_programs_give_the_same_memory() {
        for (name, s) in real_states() {
            let t = s.with_programs(&s.program_code()).unwrap();
            let area = s.end_reg() as usize..s.curtain() as usize;
            assert_eq!(t.regs[area.clone()], s.regs[area], "{name}: program memory");
            assert_eq!(t.regs[0x00D], s.regs[0x00D], "{name}: register c");
            let t = s.with_xm(&s.xm_files()).unwrap();
            let files: Vec<String> = t.xm_files().iter().map(|f| f.name.clone()).collect();
            let was: Vec<String> = s.xm_files().iter().map(|f| f.name.clone()).collect();
            assert_eq!(files, was, "{name}: X-Memory files");
            assert_eq!(t.regs[0x040], s.regs[0x040], "{name}: X-Memory header");
            for (a, b) in t.xm_files().iter().zip(s.xm_files()) {
                assert_eq!((a.head, &a.regs), (b.head, &b.regs), "{name}: file {}", a.name);
            }
        }
    }

    #[test]
    fn programs_taken_out_and_put_back_keep_the_chain() {
        let mut states = real_states();
        states.push(("sample".into(), sample()));
        for (name, s) in states {
            let mut code = s.program_code();
            if code.is_empty() {
                continue;
            }
            let links = |c: &[Vec<u8>]| -> usize {
                1 + c.iter().flat_map(|p| focal::decode(p, &Names::new())).filter(|x| x.end || x.text.starts_with("LBL \"")).count()
            };
            let first = code.remove(0);
            let t = s.with_programs(&code).unwrap();
            assert_eq!(walk_chain(&t).len(), links(&code), "{name}: chain without the first program");
            let text = |c: &[Vec<u8>]| -> Vec<String> {
                c.iter().flat_map(|p| focal::decode(p, &Names::new())).map(|x| x.text).collect()
            };
            assert_eq!(text(&t.program_code()), text(&code), "{name}: the other programs stay as they were");
            assert_eq!(t.size(), s.size(), "{name}: data registers stay");
            code.push(first.clone());
            let u = t.with_programs(&code).unwrap();
            assert_eq!(walk_chain(&u).len(), links(&code), "{name}: chain with it at the end");
            assert_eq!(text(&u.program_code()[code.len() - 1..]), text(&[first]), "{name}: it comes back whole");
        }
    }

    #[test]
    fn a_program_that_ends_at_end_gets_an_end_of_its_own() {
        let s = sample();
        let t = s.with_programs(&s.program_code()).unwrap();
        let lines: Vec<String> = t.programs(&Names::new()).iter().flat_map(|p| p.steps.clone()).map(|x| x.text).collect();
        assert_eq!(lines, ["LBL \"AB\"", "2", "*", "END", ".END."]);
        assert_eq!(walk_chain(&t).len(), 3);
    }

    #[test]
    fn programs_that_do_not_fit_are_refused() {
        let s = sample();
        let big = vec![vec![0x40; 5000]];
        assert!(s.with_programs(&big).is_err());
        let t = s.with_programs(&[]).unwrap();
        assert!(t.programs(&Names::new()).iter().all(|p| p.name() == ".END."));
        assert_eq!(walk_chain(&t).len(), 1);
    }

    #[test]
    fn a_text_file_reads_back_as_written() {
        let f = text_file("NOTES", "first line\n\nΣ and ⊢\tok\n").unwrap();
        assert_eq!(xm_lines(&f, &Names::new()), ["first line", "", "Σ and ⊢ ok"]);
        assert_eq!((f.name.as_str(), f.kind, f.regs.len()), ("NOTES", 3, 4));
        assert_eq!(f.name_reg, 0x4E4F5445532020);
        assert!(text_file("X", "ok\nnot ☃").err().unwrap().contains("line 2"));
        assert!(text_file("X", &"a".repeat(255)).is_err());
        let t = sample().with_xm(&[f.clone()]).unwrap();
        assert_eq!(xm_lines(&t.xm_files()[0], &Names::new()), xm_lines(&f, &Names::new()));
    }

    #[test]
    fn a_program_file_is_cut_at_its_end() {
        let names = Names::new();
        let texts = |b: &[u8]| -> Vec<String> { focal::decode(b, &names).into_iter().map(|s| s.text).collect() };
        // a .raw file ends in the .END. form of END; it becomes a plain END
        let mut raw = focal::encode("LBL \"A\"\n1\nEND\n", &names).unwrap();
        raw.extend([0x00, 0x00]);
        assert_eq!(texts(&program_bytes(&raw)), ["LBL \"A\"", "1", "END"]);
        let bare = focal::encode("LBL \"A\"\n1\n", &names).unwrap();
        assert_eq!(texts(&program_bytes(&bare)), ["LBL \"A\"", "1", "END"]);
        let mut two = program_bytes(&raw);
        two.extend(program_bytes(&bare));
        assert_eq!(program_bytes(&two), program_bytes(&raw));
    }

    #[test]
    fn x_memory_files_come_and_go() {
        let s = sample();
        let t = s.with_xm(&[]).unwrap();
        assert!(t.xm_files().is_empty());
        let u = t.with_xm(&s.xm_files()).unwrap();
        assert_eq!(xm_lines(&u.xm_files()[0], &Names::new()), ["AB", "C"]);
        let mut f = s.xm_files();
        f[0].head |= 0x0BE << 40;
        let v = s.with_xm(&[f[0].clone(), f[0].clone()]).unwrap();
        assert_eq!(field(v.xm_files()[1].head, 12, 10), 0x0BB, "a header gets its new address");
    }

    #[test]
    fn flags_are_set_and_cleared() {
        let mut s = sample();
        s.set_flag(27, true);
        s.set_flag(21, false);
        assert!(s.flag(27) && !s.flag(21));
    }
}
