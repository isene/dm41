//! The DM41X state file (.d41): the calculator's whole memory as hex, one
//! line per four registers, then the CPU registers.

use crate::focal::{self, Names, Step};

/// The memory of an HP-41CX: 1024 registers of 56 bits (7 bytes).
pub struct State {
    pub regs: Vec<u64>,
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

/// A file in X-Memory.
pub struct XmFile {
    pub name: String,
    pub kind: u8,
    pub regs: Vec<u64>,
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
        for line in text.lines().skip(1) {
            let Some((addr, rest)) = line.split_once("  ") else { continue };
            let Ok(a) = usize::from_str_radix(addr, 16) else { continue };
            for (k, hex) in rest.split_whitespace().enumerate() {
                let v = u64::from_str_radix(hex, 16).map_err(|_| format!("bad register {hex} at {addr}"))?;
                if let Some(r) = regs.get_mut(a + k) {
                    *r = v;
                }
            }
        }
        Ok(State { regs })
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

    /// The X-Memory files. The directory starts at the top of the
    /// X-Functions memory (0x0BF) and runs down, then on into the memory
    /// modules at 0x2EF and 0x3EF. Each file is a name register, a header
    /// register (type and size) and its data. All ones ends the list.
    pub fn xm_files(&self) -> Vec<XmFile> {
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
        let mut files = Vec::new();
        let mut i = 0;
        while i + 1 < order.len() {
            let name = self.reg(order[i]);
            if name == 0 || name == 0xFF_FFFF_FFFF_FFFF {
                break;
            }
            let h = self.reg(order[i + 1]);
            let size = field(h, 2, 0) as usize;
            let regs = order.iter().skip(i + 2).take(size).map(|&a| self.reg(a)).collect();
            let name = focal::hp_text(&bytes_of(std::iter::once(name))).trim_end().to_string();
            files.push(XmFile { name, kind: field(h, 13, 13) as u8, regs });
            i += 2 + size;
        }
        files
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
        let mut s = State { regs: vec![0; 1024] };
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
}
