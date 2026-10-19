//! Fallbacks for two MSVC STL helpers that ONNX Runtime's prebuilt library
//! (linked for local voice typing) calls but MSVC before 14.44 (VS 17.14)
//! doesn't ship. `build.rs` names them with `/ALTERNATENAME`, so the linker
//! uses these only when the toolchain's own definitions are missing.

/// Position of the first haystack element that equals any needle element,
/// or `usize::MAX` (`npos`) when there is none.
unsafe fn first_of<T: Copy + PartialEq>(
    haystack: *const T,
    haystack_len: usize,
    needle: *const T,
    needle_len: usize,
) -> usize {
    if haystack_len == 0 || needle_len == 0 {
        return usize::MAX;
    }
    // SAFETY: the STL passes valid ranges of these lengths.
    let (haystack, needle) = unsafe {
        (
            std::slice::from_raw_parts(haystack, haystack_len),
            std::slice::from_raw_parts(needle, needle_len),
        )
    };
    haystack
        .iter()
        .position(|c| needle.contains(c))
        .unwrap_or(usize::MAX)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn openagents_find_first_of_trivial_pos_1(
    haystack: *const u8,
    haystack_len: usize,
    needle: *const u8,
    needle_len: usize,
) -> usize {
    unsafe { first_of(haystack, haystack_len, needle, needle_len) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn openagents_find_first_of_trivial_pos_2(
    haystack: *const u16,
    haystack_len: usize,
    needle: *const u16,
    needle_len: usize,
) -> usize {
    unsafe { first_of(haystack, haystack_len, needle, needle_len) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_the_first_match_or_npos() {
        let hay = b"hello, world";
        let pos = |needle: &[u8]| unsafe {
            super::openagents_find_first_of_trivial_pos_1(
                hay.as_ptr(),
                hay.len(),
                needle.as_ptr(),
                needle.len(),
            )
        };
        assert_eq!(pos(b"ow"), 4);
        assert_eq!(pos(b",d"), 5);
        assert_eq!(pos(b"xyz"), usize::MAX);
        assert_eq!(pos(b""), usize::MAX);
        let wide: Vec<u16> = r"a/b\c".encode_utf16().collect();
        let seps: Vec<u16> = r"\/".encode_utf16().collect();
        let found = unsafe {
            super::openagents_find_first_of_trivial_pos_2(
                wide.as_ptr(),
                wide.len(),
                seps.as_ptr(),
                seps.len(),
            )
        };
        assert_eq!(found, 1);
    }
}
