//! The 2.0 in 320 x 240 ST7789 display, the keys, the LEDs and the buzzer.
//!
//! Colours and glyphs come from `generated/`, which `update-design.sh`
//! makes from the design system release (palette roles and JetBrains Mono).
//! Nothing here names a colour value.
//!
//! UNTESTED ON HARDWARE. The orientation byte and the inversion command
//! depend on the display module bought: see `MADCTL` and `INVERT`.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;
use rt_core::ui::{Canvas, Font, Ink, Key, H, W};

#[allow(clippy::all)]
pub mod assets {
    include!("generated/assets.rs");
}
use assets::FontData;

pub fn font(f: Font) -> &'static FontData {
    match f {
        Font::Small => &assets::SMALL,
        Font::Body => &assets::BODY,
        Font::Big => &assets::BIG,
    }
}

/// Coverage (0..=3) of pixel (x, y) of a character's glyph.
pub fn glyph_px(f: &FontData, ch: u8, x: u16, y: u16) -> u8 {
    let ch = if (assets::FIRST..=assets::LAST).contains(&ch) { ch } else { b'?' };
    let n = (y * f.w + x) as usize;
    let byte = f.data[(ch - assets::FIRST) as usize * f.glyph_bytes + n / 4];
    byte >> (6 - 2 * (n % 4)) & 3
}

/// A colour `level`/3 of the way from `bg` to `fg`, both RGB565.
pub fn blend565(bg: u16, fg: u16, level: u8) -> u16 {
    let mix = |shift: u16, mask: u16| {
        let (b, f) = ((bg >> shift & mask) as u32, (fg >> shift & mask) as u32);
        (((b * (3 - level as u32) + f * level as u32 + 1) / 3) as u16) << shift
    };
    mix(11, 0x1f) | mix(5, 0x3f) | mix(0, 0x1f)
}

/// Landscape, connector on the left. Use 0xA0 if the picture is upside down.
const MADCTL: u8 = 0x60;
/// IPS modules need inversion on (0x21); others want 0x20.
const INVERT: u8 = 0x21;

pub struct St7789<SPI, DC, RST> {
    spi: SPI,
    dc: DC,
    rst: RST,
}

impl<SPI: SpiDevice, DC: OutputPin, RST: OutputPin> St7789<SPI, DC, RST> {
    pub fn new(spi: SPI, dc: DC, rst: RST) -> Self {
        St7789 { spi, dc, rst }
    }

    fn cmd(&mut self, c: u8, data: &[u8]) {
        let _ = self.dc.set_low();
        let _ = self.spi.write(&[c]);
        if !data.is_empty() {
            let _ = self.dc.set_high();
            let _ = self.spi.write(data);
        }
    }

    pub fn init<D: DelayNs>(&mut self, delay: &mut D) {
        let _ = self.rst.set_low();
        delay.delay_ms(10);
        let _ = self.rst.set_high();
        delay.delay_ms(120);
        self.cmd(0x01, &[]); // software reset
        delay.delay_ms(150);
        self.cmd(0x11, &[]); // sleep out
        delay.delay_ms(120);
        self.cmd(0x3a, &[0x55]); // 16 bits per pixel
        self.cmd(0x36, &[MADCTL]);
        self.cmd(INVERT, &[]);
        self.cmd(0x13, &[]); // normal display mode
        self.cmd(0x29, &[]); // display on
        delay.delay_ms(20);
    }

    /// Open a drawing window; pixel data follows.
    fn window(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let (x1, y1) = (x + w - 1, y + h - 1);
        self.cmd(0x2a, &[(x >> 8) as u8, x as u8, (x1 >> 8) as u8, x1 as u8]);
        self.cmd(0x2b, &[(y >> 8) as u8, y as u8, (y1 >> 8) as u8, y1 as u8]);
        self.cmd(0x2c, &[]);
        let _ = self.dc.set_high();
    }
}

impl<SPI: SpiDevice, DC: OutputPin, RST: OutputPin> Canvas for St7789<SPI, DC, RST> {
    fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, ink: Ink) {
        if w == 0 || h == 0 || x + w > W || y + h > H {
            return;
        }
        let c = assets::PALETTE[ink as usize].to_be_bytes();
        let mut buf = [0u8; 128];
        for px in buf.chunks_exact_mut(2) {
            px.copy_from_slice(&c);
        }
        self.window(x, y, w, h);
        let mut left = w as u32 * h as u32 * 2;
        while left > 0 {
            let n = left.min(buf.len() as u32) as usize;
            let _ = self.spi.write(&buf[..n]);
            left -= n as u32;
        }
    }

    fn text(&mut self, x: u16, y: u16, s: &str, f: Font, fg: Ink, bg: Ink) {
        let fd = font(f);
        let (fg, bg) = (assets::PALETTE[fg as usize], assets::PALETTE[bg as usize]);
        let shade = [bg, blend565(bg, fg, 1), blend565(bg, fg, 2), fg];
        let mut cx = x;
        for ch in s.bytes() {
            if cx + fd.w > W || y + fd.h > H {
                break;
            }
            self.window(cx, y, fd.w, fd.h);
            let mut buf = [0u8; 96];
            let mut n = 0;
            for gy in 0..fd.h {
                for gx in 0..fd.w {
                    let c = shade[glyph_px(fd, ch, gx, gy) as usize].to_be_bytes();
                    buf[n] = c[0];
                    buf[n + 1] = c[1];
                    n += 2;
                    if n == buf.len() {
                        let _ = self.spi.write(&buf);
                        n = 0;
                    }
                }
            }
            if n > 0 {
                let _ = self.spi.write(&buf[..n]);
            }
            cx += fd.w;
        }
    }

    fn cell(&self, f: Font) -> (u16, u16) {
        let fd = font(f);
        (fd.w, fd.h)
    }
}

/// The six keys: switches to ground with pull-ups, so a pressed key reads
/// low. Call `poll` every 10 ms; it reports a key once, when it has read
/// pressed three times in a row.
pub struct Keys<K> {
    pins: [K; 6],
    count: [u8; 6],
}

const KEY_ORDER: [Key; 6] = [Key::Esc, Key::F1, Key::F2, Key::F3, Key::F4, Key::F5];

impl<K: InputPin> Keys<K> {
    /// Pins in the order ESC, F1, F2, F3, F4, F5 (KEY0 to KEY5).
    pub fn new(pins: [K; 6]) -> Self {
        Keys { pins, count: [0; 6] }
    }

    pub fn poll(&mut self) -> Option<Key> {
        let mut hit = None;
        for i in 0..6 {
            if self.pins[i].is_low().unwrap_or(false) {
                if self.count[i] < 4 {
                    self.count[i] += 1;
                    if self.count[i] == 3 && hit.is_none() {
                        hit = Some(KEY_ORDER[i]);
                    }
                }
            } else {
                self.count[i] = 0;
            }
        }
        hit
    }
}

/// Pass LED, fail LED and buzzer (all active high).
pub struct Signals<P, F, B> {
    pub pass: P,
    pub fail: F,
    pub buzzer: B,
}

impl<P: OutputPin, F: OutputPin, B: OutputPin> Signals<P, F, B> {
    pub fn show(&mut self, verdict: Option<rt_core::report::Verdict>) {
        use rt_core::report::Verdict;
        let _ = self.pass.set_state((verdict == Some(Verdict::Pass)).into());
        let _ = self.fail.set_state((verdict == Some(Verdict::Fail)).into());
    }

    /// One beep for a pass, three for anything else.
    pub fn beep<D: DelayNs>(&mut self, delay: &mut D, pass: bool) {
        for _ in 0..if pass { 1 } else { 3 } {
            let _ = self.buzzer.set_high();
            delay.delay_ms(80);
            let _ = self.buzzer.set_low();
            delay.delay_ms(80);
        }
    }
}

/// A display in memory, for previews and tests on a PC.
#[cfg(feature = "host")]
pub struct MemCanvas {
    pub px: std::vec::Vec<[u8; 3]>,
}

#[cfg(feature = "host")]
impl MemCanvas {
    pub fn new() -> Self {
        MemCanvas { px: std::vec![[0; 3]; W as usize * H as usize] }
    }

    /// Binary PPM (P6), which every image tool reads.
    pub fn ppm(&self) -> std::vec::Vec<u8> {
        let mut out = std::format!("P6\n{W} {H}\n255\n").into_bytes();
        for p in &self.px {
            out.extend_from_slice(p);
        }
        out
    }
}

#[cfg(feature = "host")]
impl Default for MemCanvas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "host")]
impl Canvas for MemCanvas {
    fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, ink: Ink) {
        assert!(x + w <= W && y + h <= H);
        for yy in y..y + h {
            for xx in x..x + w {
                self.px[yy as usize * W as usize + xx as usize] = assets::PALETTE_RGB[ink as usize];
            }
        }
    }

    fn text(&mut self, x: u16, y: u16, s: &str, f: Font, fg: Ink, bg: Ink) {
        let fd = font(f);
        let (fg, bg) = (assets::PALETTE_RGB[fg as usize], assets::PALETTE_RGB[bg as usize]);
        for (i, ch) in s.bytes().enumerate() {
            for gy in 0..fd.h {
                for gx in 0..fd.w {
                    let l = glyph_px(fd, ch, gx, gy) as u16;
                    let mut c = [0u8; 3];
                    for k in 0..3 {
                        c[k] = ((bg[k] as u16 * (3 - l) + fg[k] as u16 * l + 1) / 3) as u8;
                    }
                    let (px, py) = (x + i as u16 * fd.w + gx, y + gy);
                    assert!(px < W && py < H, "text off screen: {s:?}");
                    self.px[py as usize * W as usize + px as usize] = c;
                }
            }
        }
    }

    fn cell(&self, f: Font) -> (u16, u16) {
        let fd = font(f);
        (fd.w, fd.h)
    }
}
