//! Blob reference story.

use maud::{Markup, html};
use ui::blackbox::atoms::blob_ref;

use super::shared::{code_block, item, row, section, section_title};

pub fn blob_ref_story() -> Markup {
    html! {
        h1 class="text-2xl font-bold mb-2 pb-2 border-b border-border" { "Blob Ref" }
        p class="text-sm text-muted-foreground mb-6" { "Blob reference with size and optional MIME type." }

        (section_title("Variants"))
        (section(row(html! {
            (item("Small", blob_ref("a1b2c3d4e5f6", 1024, Some("text/plain"))))
            (item("Medium", blob_ref("f1a2b3c4d5e6", 12847, Some("text/markdown"))))
            (item("Large", blob_ref("deadbeef1234", 1048576, Some("application/octet-stream"))))
        })))

        (section_title("Usage"))
        (code_block(r#"use ui::blackbox::atoms::blob_ref;

blob_ref("a1b2c3d4e5f6", 1024, Some("text/plain"))
blob_ref("f1a2b3c4d5e6", 12847, Some("text/markdown"))
blob_ref("deadbeef1234", 1048576, Some("application/octet-stream"))"#))
    }
}
