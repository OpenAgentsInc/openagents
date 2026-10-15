//! Device-local accessibility choices; the operating system sets the first default.

pub const SAVE_KEY: &str = "snap.access.v1";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Access {
    pub big: bool,
    pub reduced: bool,
    pub marks: bool,
}
impl Access {
    pub fn encode(self) -> String {
        format!("v1 {} {} {}", self.big, self.reduced, self.marks)
    }
    pub fn decode(text: &str) -> Option<Self> {
        if text.len() > 64 {
            return None;
        }
        let p: Vec<_> = text.split_whitespace().collect();
        match p.as_slice() {
            ["v1", a, b, c] => Some(Self {
                big: a.parse().ok()?,
                reduced: b.parse().ok()?,
                marks: c.parse().ok()?,
            }),
            _ => None,
        }
    }
    pub fn big_words(self) -> &'static str {
        if self.big {
            "Big buttons: on"
        } else {
            "Big buttons: off"
        }
    }
    pub fn motion_words(self) -> &'static str {
        if self.reduced {
            "Less motion: on"
        } else {
            "Less motion: off"
        }
    }
    pub fn mark_words(self) -> &'static str {
        if self.marks {
            "Piece marks: on"
        } else {
            "Piece marks: off"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_round_trip_and_read_plainly() {
        for flag in [true, false] {
            let a = Access {
                big: flag,
                reduced: flag,
                marks: flag,
            };
            assert_eq!(Access::decode(&a.encode()), Some(a));
            for s in [a.big_words(), a.motion_words(), a.mark_words()] {
                assert!(oa_copy::violations(s, &[]).is_empty());
            }
        }
        assert_eq!(Access::decode("v1 maybe true false"), None);
    }
}
