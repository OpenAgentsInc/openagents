#![no_main]
use layertree_tee_lab::tree::Leaf;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(leaf) = Leaf::parse(data) {
        assert_eq!(&leaf.serialize()[..], data);
        let _ = leaf.validate();
    }
});
