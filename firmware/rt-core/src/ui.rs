//! Front-panel interface: the 320 x 240 display and the six keys.
//!
//! This module holds the screens and the key handling and nothing about the
//! hardware. It draws through the [`Canvas`] trait in terms of roles
//! ([`Ink`], [`Font`]); the display driver in `rt-hal` turns those into the
//! colours and glyphs generated from the design system. The firmware loop is:
//!
//! ```text
//! ui.draw(&mut lcd);
//! match ui.key(k) {
//!     Action::Run { profile, full } => {
//!         ui.draw(&mut lcd);                 // the "Testing" screen
//!         let report = run the test;
//!         ui.finish(report);
//!     }
//!     Action::None => {}
//! }
//! ```

use core::fmt::{self, Write};

use crate::device;
use crate::report::{Report, Verdict, WireFault, CTL_NAMES, Mode};

pub const W: u16 = 320;
pub const H: u16 = 240;

/// The keys under the console, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Esc,
    F1,
    F2,
    F3,
    F4,
    F5,
}

/// Colour roles of the design system. The order is the order of the
/// generated palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Ink {
    Bg,
    Surface,
    Border,
    Text,
    Muted,
    Accent,
    OnAccent,
    Ok,
    OkBg,
    Warn,
    Danger,
    DangerBg,
}

/// Token names of the inks, in order (the asset generator reads this list).
pub const INK_TOKENS: [&str; 12] = [
    "bg", "surface", "border", "text", "textMuted", "accent", "onAccent", "ok", "okBg", "warn",
    "danger", "dangerBg",
];

/// Type sizes: labels, reading text, the verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Small,
    Body,
    Big,
}

/// What the interface needs from a display. Text is monospaced; `cell`
/// gives the advance and the line height of a font in pixels.
pub trait Canvas {
    fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, ink: Ink);
    fn text(&mut self, x: u16, y: u16, s: &str, font: Font, fg: Ink, bg: Ink);
    fn cell(&self, font: Font) -> (u16, u16);
}

/// A fixed-size line of text (no allocator here).
pub struct Line<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Line<N> {
    pub fn new() -> Self {
        Line { buf: [0; N], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        // only whole `&str`s are copied in, cut at a character boundary
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl<const N: usize> Default for Line<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for Line<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut room = N - self.len;
        if s.len() < room {
            room = s.len();
        }
        while !s.is_char_boundary(room) {
            room -= 1;
        }
        self.buf[self.len..self.len + room].copy_from_slice(&s.as_bytes()[..room]);
        self.len += room;
        Ok(())
    }
}

/// Facts about the unit, shown on the About screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct Info {
    pub version: &'static str,
    pub engine_id: u32,
    pub engine_ok: bool,
    pub tick_ps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Select,
    Running,
    Result,
    Detail,
    About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// Run this test now, then call [`Ui::finish`].
    Run { profile: usize, full: bool },
}

pub struct Ui {
    screen: Screen,
    sel: usize,
    full: bool,
    report: Option<Report>,
    info: Info,
}

const HEAD: u16 = 28;
const FOOT: u16 = 38;      // two rows of small text
const PAD: u16 = 8;

impl Ui {
    pub fn new(info: Info) -> Self {
        Ui { screen: Screen::Select, sel: 0, full: false, report: None, info }
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn selected(&self) -> usize {
        self.sel
    }

    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }

    /// The outcome on show, for the pass and fail LEDs.
    pub fn verdict(&self) -> Option<Verdict> {
        match self.screen {
            Screen::Result | Screen::Detail => self.report.as_ref().map(|r| r.verdict()),
            _ => None,
        }
    }

    /// A test is starting (from a key, or from the console or the web).
    pub fn begin(&mut self, profile: usize, full: bool) {
        if profile < device::ALL.len() {
            self.sel = profile;
        }
        self.full = full;
        self.report = None;
        self.screen = Screen::Running;
    }

    /// The test is over: show its result.
    pub fn finish(&mut self, report: Report) {
        self.report = Some(report);
        self.screen = Screen::Result;
    }

    pub fn key(&mut self, key: Key) -> Action {
        match (self.screen, key) {
            (Screen::Select, Key::F1) => {
                self.sel = if self.sel == 0 { device::ALL.len() - 1 } else { self.sel - 1 };
            }
            (Screen::Select, Key::F2) => {
                self.sel = (self.sel + 1) % device::ALL.len();
            }
            (Screen::Select, Key::F3) | (Screen::Select, Key::F4) => {
                let full = key == Key::F4;
                self.begin(self.sel, full);
                return Action::Run { profile: self.sel, full };
            }
            (Screen::Select, Key::F5) => self.screen = Screen::About,
            (Screen::Result, Key::F1) => {
                let full = self.full;
                self.begin(self.sel, full);
                return Action::Run { profile: self.sel, full };
            }
            (Screen::Result, Key::F2) => {
                if self.verdict() == Some(Verdict::Fail) {
                    self.screen = Screen::Detail;
                }
            }
            (Screen::Result, Key::Esc) | (Screen::About, Key::Esc) => self.screen = Screen::Select,
            (Screen::Detail, Key::Esc) => self.screen = Screen::Result,
            _ => {}
        }
        Action::None
    }

    pub fn draw<C: Canvas>(&self, c: &mut C) {
        c.fill(0, 0, W, H, Ink::Bg);
        let (title, keys): (&str, &[&str]) = match self.screen {
            Screen::Select => ("Select", &["F1 Up", "F2 Down", "F3 Quick test", "F4 Full test",
                                           "F5 About"]),
            Screen::Running => (if self.full { "Full test" } else { "Quick test" }, &[]),
            Screen::Result => match self.verdict() {
                Some(Verdict::Fail) => ("Result", &["F1 Again", "F2 Details", "ESC Back"]),
                _ => ("Result", &["F1 Again", "ESC Back"]),
            },
            Screen::Detail => ("Details", &["ESC Back"]),
            Screen::About => ("About", &["ESC Back"]),
        };
        self.chrome(c, title, keys);
        match self.screen {
            Screen::Select => self.draw_select(c),
            Screen::Running => self.draw_running(c),
            Screen::Result => self.draw_result(c),
            Screen::Detail => self.draw_detail(c),
            Screen::About => self.draw_about(c),
        }
    }

    fn chrome<C: Canvas>(&self, c: &mut C, title: &str, keys: &[&str]) {
        let (sw, sh) = c.cell(Font::Small);
        let (_bw, bh) = c.cell(Font::Body);
        c.fill(0, 0, W, HEAD, Ink::Surface);
        c.fill(0, HEAD, W, 1, Ink::Border);
        c.text(PAD, (HEAD - bh.min(HEAD)) / 2, "RAMalyser", Font::Body, Ink::Text, Ink::Surface);
        let tw = title.len() as u16 * sw;
        c.text(W - PAD - tw.min(W - PAD), (HEAD - sh.min(HEAD)) / 2, title, Font::Small,
               Ink::Muted, Ink::Surface);
        // key legend: up to two rows, wrapped
        let top = H - FOOT;
        c.fill(0, top, W, FOOT, Ink::Surface);
        c.fill(0, top - 1, W, 1, Ink::Border);
        let (mut x, mut y) = (PAD, top + 1);
        for k in keys {
            let w = k.len() as u16 * sw;
            if x + w > W - PAD && x > PAD {
                x = PAD;
                y += sh;
            }
            // the key name in the text colour, what it does in the muted one
            let split = k.find(' ').unwrap_or(k.len());
            c.text(x, y, &k[..split], Font::Small, Ink::Text, Ink::Surface);
            c.text(x + split as u16 * sw, y, &k[split..], Font::Small, Ink::Muted, Ink::Surface);
            x += w + 2 * sw;
        }
    }

    fn draw_select<C: Canvas>(&self, c: &mut C) {
        let (bw, bh) = c.cell(Font::Body);
        let (sw, sh) = c.cell(Font::Small);
        let mut y = HEAD + 4;
        for (i, pr) in device::ALL.iter().enumerate() {
            let bg = if i == self.sel { Ink::Surface } else { Ink::Bg };
            if i == self.sel {
                c.fill(0, y, W, bh, Ink::Surface);
                c.fill(0, y, 4, bh, Ink::Accent);
            }
            c.text(PAD + 4, y, pr.name, Font::Body, Ink::Text, bg);
            if pr.aux_rails {
                let note = "-5 V +12 V";
                let nx = (PAD + 4 + (pr.name.len() as u16 + 1) * bw).max(W - PAD - note.len() as u16 * sw);
                if nx + note.len() as u16 * sw <= W {
                    c.text(nx, y + (bh - sh.min(bh)) / 2, note, Font::Small, Ink::Muted, bg);
                }
            }
            y += bh;
        }
    }

    fn draw_running<C: Canvas>(&self, c: &mut C) {
        let (_bw, bh) = c.cell(Font::Body);
        let (_gw, gh) = c.cell(Font::Big);
        let y = HEAD + 12;
        c.text(PAD, y, device::ALL[self.sel].name, Font::Body, Ink::Text, Ink::Bg);
        c.text(PAD, y + bh + 8, "Testing", Font::Big, Ink::Accent, Ink::Bg);
        c.text(PAD, y + bh + 8 + gh + 8, "Keep the module in its socket.", Font::Small, Ink::Muted,
               Ink::Bg);
    }

    fn draw_result<C: Canvas>(&self, c: &mut C) {
        let r = match &self.report {
            Some(r) => r,
            None => return,
        };
        let (_bw, bh) = c.cell(Font::Body);
        let (_gw, gh) = c.cell(Font::Big);
        // the word carries the verdict; the colour only repeats it
        let (word, fg, band) = match r.verdict() {
            Verdict::Pass => ("PASS", Ink::Ok, Ink::OkBg),
            Verdict::Fail => ("FAIL", Ink::Danger, Ink::DangerBg),
            Verdict::NoDevice => ("NO DEVICE", Ink::Warn, Ink::Surface),
        };
        let band_h = gh + 8;
        c.fill(0, HEAD + 1, W, band_h, band);
        c.text(PAD, HEAD + 5, word, Font::Big, fg, band);
        let mut y = HEAD + 1 + band_h + 6;
        let mut row = |c: &mut C, s: &str, ink: Ink| {
            if y + bh <= H - FOOT {
                c.text(PAD, y, s, Font::Body, ink, Ink::Bg);
                y += bh;
            }
        };
        row(c, r.profile, Ink::Text);
        if let Some(l) = problem(r) {
            row(c, l.as_str(), Ink::Text);
        }
        if r.geometry.row_bits != 0 {
            row(c, size_line(r).as_str(), Ink::Text);
            row(c, speed_line(r).as_str(), Ink::Text);
            if let Some(ms) = r.retention_ms {
                let mut l = Line::<26>::new();
                let _ = write!(l, "Holds {ms} ms");
                row(c, l.as_str(), Ink::Muted);
            }
        }
    }

    fn draw_detail<C: Canvas>(&self, c: &mut C) {
        let r = match &self.report {
            Some(r) => r,
            None => return,
        };
        let (sw, sh) = c.cell(Font::Small);
        let cols = ((W - 2 * PAD) / sw) as usize;
        let mut y = HEAD + 6;
        let mut row = |c: &mut C, s: &str, ink: Ink| {
            if y + sh <= H - FOOT {
                c.text(PAD, y, s, Font::Small, ink, Ink::Bg);
                y += sh;
            }
        };
        row(c, r.profile, Ink::Muted);
        if r.power_fault {
            let mut l = Line::<40>::new();
            let _ = write!(l, "Supply fault, {} mA", r.current_ma);
            row(c, l.as_str(), Ink::Text);
        }
        for w in r.wire_faults.iter().flatten() {
            row(c, wire_line(w).as_str(), Ink::Text);
        }
        if let Some(x) = &r.failure {
            let mut l = Line::<40>::new();
            let _ = write!(l, "Phase     {}", x.phase.name());
            row(c, l.as_str(), Ink::Text);
            let mut l = Line::<40>::new();
            let _ = write!(l, "Errors    {}", x.errors);
            row(c, l.as_str(), Ink::Text);
            let mut l = Line::<40>::new();
            let _ = write!(l, "First at  {:#x}", x.addr);
            row(c, l.as_str(), Ink::Text);
            let mut l = Line::<40>::new();
            let _ = write!(l, "Bad bits  {:#012x}", x.bits);
            row(c, l.as_str(), Ink::Text);
            let mut l = Line::<40>::new();
            let _ = write!(l, "Read      {:#012x}", x.read);
            row(c, l.as_str(), Ink::Text);
            // the failing data lines by name, wrapped to the width
            let mut l = Line::<40>::new();
            let _ = write!(l, "Lines    ");
            for bit in 0..40u8 {
                if x.bits >> bit & 1 == 1 {
                    let mut name = Line::<6>::new();
                    let _ = write!(name, " DQ{bit}");
                    if l.as_str().len() + name.as_str().len() > cols.min(40) {
                        row(c, l.as_str(), Ink::Text);
                        l = Line::<40>::new();
                        let _ = write!(l, "         ");
                    }
                    let _ = write!(l, "{}", name.as_str());
                }
            }
            row(c, l.as_str(), Ink::Text);
        }
    }

    fn draw_about<C: Canvas>(&self, c: &mut C) {
        let (_bw, bh) = c.cell(Font::Body);
        let mut y = HEAD + 8;
        let mut row = |c: &mut C, s: &str, ink: Ink| {
            c.text(PAD, y, s, Font::Body, ink, Ink::Bg);
            y += bh;
        };
        let mut l = Line::<26>::new();
        let _ = write!(l, "Firmware {}", self.info.version);
        row(c, l.as_str(), Ink::Text);
        let mut l = Line::<26>::new();
        let _ = write!(l, "Engine {:#010x}", self.info.engine_id);
        row(c, l.as_str(), Ink::Text);
        row(c, if self.info.engine_ok { "Engine answers" } else { "Engine does not answer" },
            if self.info.engine_ok { Ink::Ok } else { Ink::Danger });
        let mut l = Line::<26>::new();
        let _ = write!(l, "Tick {} ps", self.info.tick_ps);
        row(c, l.as_str(), Ink::Text);
        let mut l = Line::<26>::new();
        let _ = write!(l, "{} profiles", device::ALL.len());
        row(c, l.as_str(), Ink::Muted);
    }
}

fn wire_line(w: &WireFault) -> Line<40> {
    let mut l = Line::new();
    let _ = match *w {
        WireFault::StuckLow(i) => write!(l, "{} stuck low", CTL_NAMES[i as usize]),
        WireFault::StuckHigh(i) => write!(l, "{} stuck high", CTL_NAMES[i as usize]),
        WireFault::Short(a, b) => write!(l, "{} shorted to {}", CTL_NAMES[a as usize],
                                         CTL_NAMES[b as usize]),
    };
    l
}

/// The first thing wrong, in a few words.
fn problem(r: &Report) -> Option<Line<40>> {
    let mut l = Line::new();
    if r.power_fault {
        let _ = write!(l, "Supply fault {} mA", r.current_ma);
        return Some(l);
    }
    if let Some(w) = r.wire_faults.iter().flatten().next() {
        return Some(wire_line(w));
    }
    if r.no_device {
        let _ = write!(l, "Nothing answers");
        return Some(l);
    }
    r.failure.as_ref().map(|x| {
        let _ = write!(l, "{}: {} errors", x.phase.name(), x.errors);
        l
    })
}

fn size_line(r: &Report) -> Line<26> {
    let g = &r.geometry;
    let mut l = Line::new();
    let kbits = g.bits() / 1024;
    let _ = if kbits >= 8192 {
        write!(l, "{} MB  {}K x{}", kbits / 8192, g.words() / 1024, g.data_bits())
    } else {
        write!(l, "{} Kbit  {}K x{}", kbits, g.words() / 1024, g.data_bits())
    };
    l
}

fn speed_line(r: &Report) -> Line<26> {
    let mut l = Line::new();
    let _ = match r.mode {
        Some(Mode::Edo) => write!(l, "EDO"),
        Some(Mode::Fpm) => write!(l, "FPM"),
        None => Ok(()),
    };
    if let Some(t) = r.t_rac_ns {
        let _ = write!(l, "  {t} ns");
    }
    let _ = write!(l, "  {} mA", r.current_ma);
    l
}
