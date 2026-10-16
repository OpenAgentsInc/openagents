#![no_main]
use layertree_tee_lab::codec::dec64;
use layertree_tee_lab::tree::enc64;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(n) = dec64(data) {
        assert_eq!(&enc64(n)[..], data);
    }
    // Injectivity from the other side: distinct values encode distinctly.
    if data.len() >= 16 {
        let a = u64::from_le_bytes(data[..8].try_into().unwrap());
        let b = u64::from_le_bytes(data[8..16].try_into().unwrap());
        assert_eq!(a == b, enc64(a) == enc64(b));
        assert_eq!(dec64(&enc64(a)), Ok(a));
    }
});
