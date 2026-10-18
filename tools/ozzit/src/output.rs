//! Text-stream compatibility for AFE helpers, which retain Python's stream encoding.
use std::io::{self, Write};

#[derive(Clone, Copy)]
pub enum Encoding {
    Utf8,
    Ascii,
    Latin1,
    Cp1252,
}

impl Encoding {
    pub fn from_override(value: &str) -> Result<Self, String> {
        let label = value.split(':').next().unwrap_or("").to_ascii_lowercase();
        let label = label.replace(['-', '_', ' '], "");
        match label.as_str() {
            "" | "utf8" | "u8" | "utf" => Ok(Self::Utf8),
            "ascii" | "usascii" | "646" => Ok(Self::Ascii),
            "latin1" | "latin" | "iso88591" | "l1" => Ok(Self::Latin1),
            "cp1252" | "windows1252" | "1252" => Ok(Self::Cp1252),
            _ => Err(format!(
                "unsupported PYTHONIOENCODING for AFE output: {value:?}; use utf-8, ascii, latin-1 or cp1252"
            )),
        }
    }

    fn byte(self, ch: char) -> Option<u8> {
        let n = ch as u32;
        match self {
            Self::Ascii => (n < 128).then_some(n as u8),
            Self::Latin1 => (n < 256).then_some(n as u8),
            Self::Cp1252 if n < 128 || (160..256).contains(&n) => Some(n as u8),
            Self::Cp1252 => {
                const SPECIAL: &str = "€\0‚ƒ„…†‡ˆ‰Š‹Œ\0Ž\0\0‘’“”•–—˜™š›œ\0žŸ";
                SPECIAL
                    .chars()
                    .position(|c| c != '\0' && c == ch)
                    .map(|i| 128 + i as u8)
            }
            Self::Utf8 => None,
        }
    }

    pub fn write(self, text: &str, sink: &mut dyn Write) -> io::Result<()> {
        if matches!(self, Self::Utf8) {
            return sink.write_all(text.as_bytes());
        }
        let mut bytes = Vec::with_capacity(text.len());
        for ch in text.chars() {
            if let Some(byte) = self.byte(ch) {
                bytes.push(byte);
            } else {
                let n = ch as u32;
                let escaped = if n <= 0xff {
                    format!("\\x{n:02x}")
                } else if n <= 0xffff {
                    format!("\\u{n:04x}")
                } else {
                    format!("\\U{n:08x}")
                };
                bytes.extend_from_slice(escaped.as_bytes());
            }
        }
        sink.write_all(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrepresentable_characters_use_python_backslash_replacement() {
        for (name, expected) in [
            (
                "cp1252:strict",
                b"\x80 \xb6 \\x81 \\u03bb \\u2192 \\U0001f600".as_slice(),
            ),
            (
                "ascii",
                b"\\u20ac \\xb6 \\x81 \\u03bb \\u2192 \\U0001f600".as_slice(),
            ),
            (
                "latin-1",
                b"\\u20ac \xb6 \x81 \\u03bb \\u2192 \\U0001f600".as_slice(),
            ),
        ] {
            let mut bytes = vec![];
            Encoding::from_override(name)
                .unwrap()
                .write("€ ¶ \u{81} λ → 😀", &mut bytes)
                .unwrap();
            assert_eq!(bytes, expected, "{name}");
        }
        assert!(Encoding::from_override("unknown-codec").is_err());
    }

    #[test]
    fn cp1252_specials_are_bijective_and_undefined_controls_are_escaped() {
        let encoding = Encoding::Cp1252;
        let mut seen = std::collections::BTreeSet::new();
        for ch in '\0'..='\u{ffff}' {
            if let Some(byte) = encoding.byte(ch) {
                assert!(seen.insert(byte), "duplicate encoding for {ch:?}");
            }
        }
        assert_eq!(seen.len(), 251);
        for undefined in [0x81, 0x8d, 0x8f, 0x90, 0x9d] {
            assert!(!seen.contains(&undefined));
        }
    }
}
