#![no_main]
use layertree_tee_lab::rules::Record;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(record) = Record::decode(data) {
        assert_eq!(record.encode(), data);
        let _ = record.hash();
    }
});
