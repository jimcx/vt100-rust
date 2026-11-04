use crate::term::BufWrite as _;
use alloc::string::String;

/// Represents a foreground or background color for cells.
#[derive(Eq, PartialEq, Debug, Copy, Clone, Default)]
pub enum Color {
    /// The default terminal color.
    #[default]
    Default,

    /// An indexed terminal color.
    Idx(u8),

    /// An RGB terminal color. The parameters are (red, green, blue).
    Rgb(u8, u8, u8),
}

const TEXT_MODE_INTENSITY: u8 = 0b0000_0011;
const TEXT_MODE_BOLD: u8 = 0b0000_0001;
const TEXT_MODE_DIM: u8 = 0b0000_0010;
const TEXT_MODE_ITALIC: u8 = 0b0000_0100;
const TEXT_MODE_UNDERLINE: u8 = 0b0000_1000;
const TEXT_MODE_INVERSE: u8 = 0b0001_0000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attrs {
    pub fgcolor: Color,
    pub bgcolor: Color,
    pub mode: u8,
}

impl Default for Attrs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "compress")]
impl Color {
    fn encode(self, out: &mut alloc::vec::Vec<u8>) {
        match self {
            Self::Default => out.push(0),
            Self::Idx(i) => out.extend_from_slice(&[1, i]),
            Self::Rgb(r, g, b) => out.extend_from_slice(&[2, r, g, b]),
        }
    }

    fn decode(data: &[u8], pos: &mut usize) -> Option<Self> {
        let tag = *data.get(*pos)?;
        *pos += 1;
        let (color, n) = match tag {
            0 => (Self::Default, 0),
            1 => (Self::Idx(*data.get(*pos)?), 1),
            2 => (Self::Rgb(*data.get(*pos)?, *data.get(*pos + 1)?, *data.get(*pos + 2)?), 3),
            _ => return None,
        };
        *pos += n;
        Some(color)
    }
}

#[cfg(feature = "compress")]
impl Attrs {
    pub(crate) fn encode(&self, out: &mut alloc::vec::Vec<u8>) {
        self.fgcolor.encode(out);
        self.bgcolor.encode(out);
        out.push(self.mode);
    }

    pub(crate) fn decode(data: &[u8], pos: &mut usize) -> Option<Self> {
        let fgcolor = Color::decode(data, pos)?;
        let bgcolor = Color::decode(data, pos)?;
        let mode = *data.get(*pos)?;
        *pos += 1;
        Some(Self { fgcolor, bgcolor, mode })
    }
}

impl Attrs {
    pub const fn new() -> Self {
        Self {
            fgcolor: Color::Default,
            bgcolor: Color::Default,
            mode: 0,
        }
    }

    pub fn bold(&self) -> bool {
        self.mode & TEXT_MODE_BOLD != 0
    }

    pub fn dim(&self) -> bool {
        self.mode & TEXT_MODE_DIM != 0
    }

    fn intensity(&self) -> u8 {
        self.mode & TEXT_MODE_INTENSITY
    }

    pub fn set_bold(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
        self.mode |= TEXT_MODE_BOLD;
    }

    pub fn set_dim(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
        self.mode |= TEXT_MODE_DIM;
    }

    pub fn set_normal_intensity(&mut self) {
        self.mode &= !TEXT_MODE_INTENSITY;
    }

    pub fn italic(&self) -> bool {
        self.mode & TEXT_MODE_ITALIC != 0
    }

    pub fn set_italic(&mut self, italic: bool) {
        if italic {
            self.mode |= TEXT_MODE_ITALIC;
        } else {
            self.mode &= !TEXT_MODE_ITALIC;
        }
    }

    pub fn underline(&self) -> bool {
        self.mode & TEXT_MODE_UNDERLINE != 0
    }

    pub fn set_underline(&mut self, underline: bool) {
        if underline {
            self.mode |= TEXT_MODE_UNDERLINE;
        } else {
            self.mode &= !TEXT_MODE_UNDERLINE;
        }
    }

    pub fn inverse(&self) -> bool {
        self.mode & TEXT_MODE_INVERSE != 0
    }

    pub fn set_inverse(&mut self, inverse: bool) {
        if inverse {
            self.mode |= TEXT_MODE_INVERSE;
        } else {
            self.mode &= !TEXT_MODE_INVERSE;
        }
    }

    pub fn write_escape_code_diff(
        &self,
        contents: &mut String,
        other: &Self,
    ) {
        self.write_escape_code_diff_fmt(contents, other)
            .expect("writing to a String cannot fail");
    }

    pub fn write_escape_code_diff_fmt(
        &self,
        writer: &mut impl alloc::fmt::Write,
        other: &Self,
    ) -> alloc::fmt::Result {
        if self != other && self == &Self::default() {
            return crate::term::ClearAttrs.write_fmt(writer);
        }

        let attrs = crate::term::Attrs::default();

        let attrs = if self.fgcolor == other.fgcolor {
            attrs
        } else {
            attrs.fgcolor(self.fgcolor)
        };
        let attrs = if self.bgcolor == other.bgcolor {
            attrs
        } else {
            attrs.bgcolor(self.bgcolor)
        };
        let attrs = if self.intensity() == other.intensity() {
            attrs
        } else {
            attrs.intensity(match self.intensity() {
                0 => crate::term::Intensity::Normal,
                TEXT_MODE_BOLD => crate::term::Intensity::Bold,
                TEXT_MODE_DIM => crate::term::Intensity::Dim,
                _ => unreachable!(),
            })
        };
        let attrs = if self.italic() == other.italic() {
            attrs
        } else {
            attrs.italic(self.italic())
        };
        let attrs = if self.underline() == other.underline() {
            attrs
        } else {
            attrs.underline(self.underline())
        };
        let attrs = if self.inverse() == other.inverse() {
            attrs
        } else {
            attrs.inverse(self.inverse())
        };

        attrs.write_fmt(writer)
    }
}
