#![no_main]
use layertree_tee_lab::unroll::UnrollPackage;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(pkg) = UnrollPackage::decode(data) {
        assert_eq!(pkg.encode(), data);
    }
});
