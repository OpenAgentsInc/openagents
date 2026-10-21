//! Which values the Grid terminal may send as `X-OpenAgents-Pylon-Model`.

/// The longest plain model name the page sends.
const MAX_NAME: usize = 64;

/// Whether `model` can go in the model header: a plain name of at most
/// 64 visible ASCII characters, or one exact artifact, `NAME@sha256:` and
/// 64 lowercase hex digits, such as a Flint pin (#12436). The site's Pylon
/// door decides which of these it honours.
pub fn sendable(model: &str) -> bool {
    if model.is_empty() {
        return false;
    }
    if model.len() <= MAX_NAME {
        return model.bytes().all(|b| b.is_ascii_graphic());
    }
    model.split_once("@sha256:").is_some_and(|(name, hex)| {
        !name.is_empty()
            && name.len() <= MAX_NAME
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
            && hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[cfg(test)]
mod tests {
    use super::sendable;

    #[test]
    fn a_flint_artifact_pin_fits_the_header() {
        let pin = format!("flint@sha256:{}", "9c".repeat(32));
        assert_eq!(pin.len(), 77);
        assert!(sendable(&pin));
        assert!(sendable("flint"));
        assert!(sendable("deepseek-v4.1-flash"));
    }

    #[test]
    fn other_long_or_unprintable_values_stay_out() {
        for refused in [
            String::new(),
            "a".repeat(65),
            format!("flint@sha256:{}", "9C".repeat(32)),
            format!("flint@sha256:{}", "9c".repeat(31)),
            format!("flint@sha256:{}0", "9c".repeat(32)),
            format!("Flint@sha256:{}", "9c".repeat(32)),
            format!("@sha256:{}{}", "9c".repeat(32), "x".repeat(10)),
            format!("{}@sha256:{}", "f".repeat(65), "9c".repeat(32)),
            "two words".into(),
            "line\nbreak".into(),
        ] {
            assert!(!sendable(&refused), "{refused}");
        }
    }
}
