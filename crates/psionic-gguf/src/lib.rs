//! Narrow GGUF load path for in-process OpenAgents inference.
//!
//! Parse metadata without mapping weights, then mmap and optionally wrap the
//! mapping as a Metal shared buffer. This crate is not the Qwen 3.8 decoder.

pub mod admit;
pub mod context;
pub mod format;
pub mod generate;
pub mod metal_wrap;
pub mod mmap;
pub mod progress;
pub mod tokenizer;

pub use admit::{admit, translate_tensor_name, Admission, AdmitError, FAMILY_QWEN35};
pub use context::{plan_caches, runtime_n_ctx, CachePlan, DEFAULT_RUNTIME_CTX};
pub use format::{
    ggml_type_name, parse_bytes, parse_path, write_qwen35_fixture, GgufMeta, ParseError,
    TensorInfo, MAGIC,
};
pub use mmap::{map_file, MappedWeights};
pub use progress::{format_bar, should_emit};
pub use tokenizer::{load_tokenizer, render_chat, TokenizerTables};

pub const PROVENANCE_PIN: &str = "issue-352-ctx.done";

pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KIB * KIB * KIB {
        format!("{:.1} GiB", b / (KIB * KIB * KIB))
    } else if b >= KIB * KIB {
        format!("{:.1} MiB", b / (KIB * KIB))
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fixture_parses_as_qwen35() {
        let dir = std::env::temp_dir().join("psionic-gguf-fixture");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("qwen35.gguf");
        write_qwen35_fixture(&path).unwrap();
        let meta = parse_path(&path).unwrap();
        assert_eq!(meta.architecture(), Some("qwen35"));
        assert_eq!(meta.n_tensors, 4);
        let tok = load_tokenizer(&meta).unwrap();
        assert_eq!(tok.n_tokens, 4);
        let header = std::fs::read(&path).unwrap();
        admit::admit(&meta, &header[..meta.data_offset as usize]).unwrap();
        let mapped = map_file(&path, &meta).unwrap();
        assert!(mapped.tensors.contains_key("token_embd.weight"));
    }

    #[test]
    fn rejects_bad_magic() {
        let dir = std::env::temp_dir().join("psionic-gguf-fixture");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not.gguf");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"XXXX\x03\x00\x00\x00").unwrap();
        match parse_path(&path) {
            Err(ParseError::Magic { got }) => assert_eq!(&got, b"XXXX"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn translates_ollama_mtp_names() {
        assert_eq!(translate_tensor_name("mtp.0.weight"), "blk.0.nextn.weight");
        assert_eq!(
            translate_tensor_name("token_embd.weight"),
            "token_embd.weight"
        );
    }
}
