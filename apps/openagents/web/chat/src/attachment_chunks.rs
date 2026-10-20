use base64::{Engine as _, engine::general_purpose::STANDARD};

pub const MAX_BYTES: usize = 24 * 1024 * 1024;

pub fn chunks(bytes: &[u8]) -> Vec<String> {
    // Each chunk decodes independently and fits in one relay frame.
    bytes
        .chunks(510_000)
        .map(|part| STANDARD.encode(part))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_twenty_mb_upload_round_trips_in_bounded_chunks() {
        let bytes = vec![137; 20 * 1024 * 1024];
        assert!(bytes.len() < MAX_BYTES);
        let parts = chunks(&bytes);
        assert!(parts.len() > 1);
        let decoded: Vec<u8> = parts
            .iter()
            .flat_map(|part| {
                assert!(part.len() <= 680_000);
                STANDARD.decode(part).unwrap()
            })
            .collect();
        assert_eq!(decoded, bytes);
    }
}
