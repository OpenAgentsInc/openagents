//! An audio chunk: mono 16-bit samples at one rate, for a recorded sound
//! that can't be made in the page. Every sound shipped this way is ours or
//! CC0, and its source is recorded beside it in the game's sound list.
//!
//! ```text
//! "ICA1"  u32 sample rate  u32 samples  i16 sample x samples
//! ```

const MAGIC: &[u8; 4] = b"ICA1";

/// A decoded sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Audio {
    pub rate: u32,
    pub samples: Vec<i16>,
}

/// Packs a sound.
#[must_use]
pub fn encode(audio: &Audio) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + audio.samples.len() * 2);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&audio.rate.to_le_bytes());
    out.extend_from_slice(&(audio.samples.len() as u32).to_le_bytes());
    for s in &audio.samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Unpacks a sound.
///
/// # Errors
/// The bytes aren't an audio chunk, or end early.
pub fn decode(bytes: &[u8]) -> Result<Audio, String> {
    if bytes.len() < 12 || &bytes[..4] != MAGIC {
        return Err("not an audio chunk".into());
    }
    let word = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let rate = word(4);
    let n = word(8) as usize;
    let data = bytes.get(12..12 + n * 2).ok_or("the sound ends early")?;
    Ok(Audio {
        rate,
        samples: data
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sound_round_trips() {
        let audio = Audio {
            rate: 22_050,
            samples: (0..1000).map(|i| ((i * 37) % 2000 - 1000) as i16).collect(),
        };
        let bytes = encode(&audio);
        assert_eq!(decode(&bytes).unwrap(), audio);
        assert!(decode(&bytes[..20]).is_err());
    }
}
