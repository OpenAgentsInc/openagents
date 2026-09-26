//! A public synthetic fixture for platform feasibility, with no task authority.
//!
//! Native views consume the same ATIF steps as the Rust-rendered HTML sample.
//! This is a bounded probe, not a production cache, transport, or credential store.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_keystore;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "ios")]
mod ios_keychain;

use atif::{Source, Step};
use std::fmt::Write;

/// The fixed size of the long-transcript fixture.
pub const STEP_COUNT: usize = 2_000;

/// A complete synthetic ATIF transcript, including Unicode and hostile HTML.
#[must_use]
pub fn fixture() -> Vec<Step> {
    (0..STEP_COUNT)
        .map(|index| {
            let mut step = Step::said(
                if index % 2 == 0 {
                    Source::User
                } else {
                    Source::Agent
                },
                &format!(
                    "Fixture step {index:04}: inspect source, retain the complete result.\n\
                     Unicode: café, 日本語, العربية, 👩🏽‍💻.\n\
                     Literal text: <script>alert('fixture')</script> & \"quoted\".\n\
                     This is synthetic evidence. Execution: not started. Checks: not run. Cost: unknown."
                ),
            );
            step.at = 1_790_424_000_000 + index as u64;
            step
        })
        .collect()
}

/// Render every step without summarizing or truncating its contents.
#[must_use]
pub fn transcript(steps: &[Step]) -> String {
    let mut text = String::from(
        "Coder platform probe\nSynthetic, read-only task\n\
         No network, credentials, execution, or control.\n\
         Cost: unknown · Checks: not run\n\n",
    );
    for (index, step) in steps.iter().enumerate() {
        let _ = writeln!(text, "[{index:04}] {:?}\n{}\n", step.source, step.message);
    }
    text.push_str("END OF COMPLETE SYNTHETIC TRANSCRIPT\n");
    text
}

/// Render semantic HTML in Rust without executable client code.
#[must_use]
pub fn html(steps: &[Step]) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Coder platform probe</title>\
         <style>body{{font:1rem system-ui;max-width:70rem;margin:auto;padding:1rem}}\
         pre{{white-space:pre-wrap;overflow-wrap:anywhere}}\
         input{{font:inherit;width:95%;padding:.5rem}}</style>\
         <main><h1>Coder platform probe</h1>\
         <p>Synthetic, read-only fixture. This field sends nothing.</p>\
         <label for=\"draft\">Input and IME probe</label>\
         <input id=\"draft\" autocomplete=\"off\" placeholder=\"日本語 · café · 👩🏽‍💻\">\
         <details><summary>Full transcript ({} steps)</summary><pre>{}</pre></details>\
         </main></html>",
        steps.len(),
        escape_html(&transcript(steps)),
    )
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Export the fixture through the existing ATIF document implementation.
#[must_use]
pub fn atif_document(steps: &[Step]) -> serde_json::Value {
    let session = atif::Session::opening(
        "coder-mobile-synthetic-v1",
        "none-synthetic",
        "platform-probe",
        "/synthetic/workspace",
        env!("CARGO_PKG_VERSION"),
    );
    atif::document(&session, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_transcript_keeps_last_step_and_unicode() {
        let steps = fixture();
        let rendered = transcript(&steps);
        assert_eq!(steps.len(), STEP_COUNT);
        assert!(rendered.contains("Fixture step 1999"));
        assert!(rendered.contains("日本語, العربية, 👩🏽‍💻"));
        assert!(rendered.ends_with("END OF COMPLETE SYNTHETIC TRANSCRIPT\n"));
        assert_eq!(
            atif_document(&steps)["steps"].as_array().unwrap().len(),
            STEP_COUNT
        );
    }

    #[test]
    fn html_preserves_untrusted_text_without_executing_it() {
        let rendered = html(&fixture());
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;alert(&#39;fixture&#39;)&lt;/script&gt;"));
        assert!(rendered.contains("Fixture step 1999"));
        assert!(rendered.contains("label for=\"draft\""));
    }
}
