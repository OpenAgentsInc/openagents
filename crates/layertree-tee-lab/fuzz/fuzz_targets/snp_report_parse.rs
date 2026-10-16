#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(report) = att_platforms::snp::parse(data) {
        assert_eq!(data.len(), 1184);
        assert_eq!(report.measurement.len(), 96);
    }
});
