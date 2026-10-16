#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(quote) = att_platforms::tdx::parse(data) {
        assert_eq!(quote.body.len(), 584);
        assert_eq!(quote.signed.len(), 48 + 584);
        assert_eq!(quote.quote_sig.len(), 64);
        assert_eq!(quote.attest_pub.len(), 64);
        assert_eq!(quote.qe_report.len(), 384);
    }
});
