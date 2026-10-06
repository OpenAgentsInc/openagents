//! Where the parser stands, so a snapshot can carry the input it holds
//! unfinished.
//!
//! `vte` keeps its state private, so this mirrors its transitions closely
//! enough to know when it is back in its ground state with no partial
//! UTF-8 character. The bytes since that point are the continuation: fed
//! to a fresh parser, they leave it exactly where this one is, because
//! every sequence starts by clearing what the previous one collected.
//!
//! The tracker errs only one way. When it cannot tell whether a partial
//! character resolved, it keeps the bytes until the next ASCII byte, which
//! always resolves one. A longer continuation replays to the same parser
//! state; a shorter one would not.

/// The longest continuation a snapshot carries. NIP-TERM bounds the
/// `CONTINUATION` record the same way.
pub const CONTINUATION_MAX: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsIgnore,
    DcsPassthrough,
    Osc,
    /// SOS, PM, and APC strings, which only a cancel or an escape ends.
    Ignore,
}

/// Ground-state UTF-8 progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Utf8 {
    #[default]
    Clean,
    /// A valid lead byte arrived; `left` continuation bytes remain, the
    /// next in `low..=high`.
    Need { left: u8, low: u8, high: u8 },
    /// Unknown until the next ASCII byte.
    Dirty,
}

/// The parser's position and the bytes since it was last at rest.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tracker {
    phase: Phase,
    utf8: Utf8,
    pending: Vec<u8>,
    /// The unfinished input outgrew [`CONTINUATION_MAX`].
    overflow: bool,
}

impl Tracker {
    /// Follows `bytes`, which the parser just took.
    pub(crate) fn track(&mut self, bytes: &[u8]) {
        let mut rest = None;
        for (index, &byte) in bytes.iter().enumerate() {
            if self.at_rest() && byte < 0x80 && byte != 0x1b {
                // The common case: plain text and controls in ground.
                rest = Some(index + 1);
                continue;
            }
            self.step(byte);
            if self.at_rest() {
                rest = Some(index + 1);
            }
        }
        let tail = match rest {
            Some(end) => {
                self.pending.clear();
                self.overflow = false;
                &bytes[end..]
            }
            None => bytes,
        };
        if self.overflow {
            return;
        }
        if self.pending.len() + tail.len() > CONTINUATION_MAX {
            self.pending.clear();
            self.overflow = true;
        } else {
            self.pending.extend_from_slice(tail);
        }
    }

    /// The input the parser holds unfinished: empty at rest, and `None`
    /// when it is too long to carry.
    pub(crate) fn continuation(&self) -> Option<&[u8]> {
        (!self.overflow).then_some(self.pending.as_slice())
    }

    fn at_rest(&self) -> bool {
        self.phase == Phase::Ground && self.utf8 == Utf8::Clean
    }

    fn step(&mut self, byte: u8) {
        // A cancel ends any sequence, and an escape starts a new one.
        if self.phase != Phase::Ground {
            match byte {
                0x18 | 0x1a => {
                    self.phase = Phase::Ground;
                    return;
                }
                0x1b => {
                    self.phase = Phase::Escape;
                    return;
                }
                _ => {}
            }
        }
        self.phase = match self.phase {
            Phase::Ground => {
                self.ground(byte);
                return;
            }
            Phase::Escape => match byte {
                0x20..=0x2f => Phase::EscapeIntermediate,
                0x50 => Phase::DcsEntry,
                0x58 | 0x5e | 0x5f => Phase::Ignore,
                0x5b => Phase::Csi,
                0x5d => Phase::Osc,
                0x30..=0x7e => Phase::Ground,
                _ => Phase::Escape,
            },
            Phase::EscapeIntermediate => match byte {
                0x30..=0x7e => Phase::Ground,
                _ => Phase::EscapeIntermediate,
            },
            Phase::Csi => match byte {
                0x40..=0x7e => Phase::Ground,
                _ => Phase::Csi,
            },
            Phase::DcsEntry => match byte {
                0x20..=0x2f => Phase::DcsIntermediate,
                0x30..=0x3f => Phase::DcsParam,
                0x40..=0x7e => Phase::DcsPassthrough,
                _ => Phase::DcsEntry,
            },
            Phase::DcsParam => match byte {
                0x20..=0x2f => Phase::DcsIntermediate,
                0x3c..=0x3f => Phase::DcsIgnore,
                0x40..=0x7e => Phase::DcsPassthrough,
                _ => Phase::DcsParam,
            },
            Phase::DcsIntermediate => match byte {
                0x30..=0x3f => Phase::DcsIgnore,
                0x40..=0x7e => Phase::DcsPassthrough,
                _ => Phase::DcsIntermediate,
            },
            Phase::DcsPassthrough => match byte {
                0x9c => Phase::Ground,
                _ => Phase::DcsPassthrough,
            },
            Phase::Osc => match byte {
                0x07 => Phase::Ground,
                _ => Phase::Osc,
            },
            phase @ (Phase::DcsIgnore | Phase::Ignore) => phase,
        };
    }

    fn ground(&mut self, byte: u8) {
        if byte < 0x80 {
            // An ASCII byte finishes or abandons any partial character.
            self.utf8 = Utf8::Clean;
            if byte == 0x1b {
                self.phase = Phase::Escape;
            }
            return;
        }
        self.utf8 = match self.utf8 {
            Utf8::Clean => match byte {
                0xc2..=0xdf => Utf8::Need {
                    left: 1,
                    low: 0x80,
                    high: 0xbf,
                },
                0xe0 => Utf8::Need {
                    left: 2,
                    low: 0xa0,
                    high: 0xbf,
                },
                0xed => Utf8::Need {
                    left: 2,
                    low: 0x80,
                    high: 0x9f,
                },
                0xe1..=0xef => Utf8::Need {
                    left: 2,
                    low: 0x80,
                    high: 0xbf,
                },
                0xf0 => Utf8::Need {
                    left: 3,
                    low: 0x90,
                    high: 0xbf,
                },
                0xf4 => Utf8::Need {
                    left: 3,
                    low: 0x80,
                    high: 0x8f,
                },
                0xf1..=0xf3 => Utf8::Need {
                    left: 3,
                    low: 0x80,
                    high: 0xbf,
                },
                _ => Utf8::Dirty,
            },
            Utf8::Need { left, low, high } if (low..=high).contains(&byte) => {
                if left == 1 {
                    Utf8::Clean
                } else {
                    Utf8::Need {
                        left: left - 1,
                        low: 0x80,
                        high: 0xbf,
                    }
                }
            }
            Utf8::Need { .. } | Utf8::Dirty => Utf8::Dirty,
        };
    }
}

/// A performer that does nothing: replaying a continuation restores the
/// parser's position, and the effects of those bytes are already in the
/// restored state.
pub(crate) struct Ignore;

impl vte::Perform for Ignore {}
