#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value;

use super::encode::{
    ClefRequest, EncodedQuestion, EncodedRecord, RequestLimits, RequestRefusal, Truncation,
    encode_record,
};
use super::head::{ClefHeadStream, ClefHeadWeights};
use super::json;
use super::{QuestionType, answer_for};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/clef")
}

const LIMITS: RequestLimits = RequestLimits {
    max_questions: 64,
    max_options: 255,
};

fn request(text: &str) -> Result<ClefRequest, RequestRefusal> {
    ClefRequest::from_json(&json::parse(text).expect("json"), LIMITS)
}

/// One token per character, so spans are easy to read.
fn char_tokens(text: &str) -> Vec<u32> {
    text.chars().map(|c| c as u32).collect()
}

#[test]
fn tiny_head_matches_torch_reference() {
    let dir = fixtures().join("head/tiny");
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("case.json")).expect("case"))
            .expect("case json");
    let weights = ClefHeadWeights::from_safetensors(&dir, 64).expect("tiny head");
    let ids: Vec<u32> = case["input_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    let questions = case["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            let span = |v: &Value| {
                (
                    v[0].as_u64().unwrap() as usize,
                    v[1].as_u64().unwrap() as usize,
                )
            };
            EncodedQuestion {
                question_id: q["id"].as_str().unwrap().to_string(),
                question_type: q["type"].as_u64().unwrap() as usize,
                question_span: span(&q["question_span"]),
                option_spans: q["option_spans"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(span)
                    .collect(),
                option_ids: q["option_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect(),
            }
        })
        .collect();
    let record = EncodedRecord {
        input_ids: ids,
        questions,
        truncated_state_tokens: 0,
    };
    let rows = |key: &str| -> Vec<Vec<f32>> {
        case[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                row.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap() as f32)
                    .collect()
            })
            .collect()
    };
    let hidden = rows("hidden");
    let embedding = rows("embedding");
    let mut stream = ClefHeadStream::new(&weights, &record);
    for (index, row) in hidden.iter().enumerate() {
        stream.push(index, row).expect("push");
    }
    let lexical = |ids: &[u32]| {
        Ok(ids
            .iter()
            .map(|id| embedding[*id as usize].clone())
            .collect())
    };
    let logits = stream.finish(&record, &lexical).expect("head");
    let expected = rows("logits");
    let mut worst = 0.0f32;
    for (got, want) in logits.iter().zip(&expected) {
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            worst = worst.max((g - w).abs());
        }
    }
    assert!(worst <= 1e-4, "max |Δlogit| {worst}");
}

#[test]
fn tiny_head_refuses_unknown_and_drifting_heads() {
    let dir = fixtures().join("head/tiny");
    let error = ClefHeadWeights::from_safetensors(&dir, 128).unwrap_err();
    assert!(error.0.contains("hidden size"), "{error}");
}

#[test]
fn prompt_pieces_and_spans_follow_the_reference() {
    let req = request(
        r#"{"model":"clef-flash","state":{"b":1.0,"a":"x"},"questions":{
            "urgent":{"type":"noul"},
            "team":{"type":"choice","instructions":{"q":"who"},"criteria":{"tech":null,"billing":"money"}}}}"#,
    )
    .expect("request");
    let record = encode_record(&req, &char_tokens, usize::MAX).expect("encode");
    let text: String = record
        .input_ids
        .iter()
        .map(|id| char::from_u32(*id).unwrap())
        .collect();
    let expected = concat!(
        "<|im_start|>system\nRead the complete state and schema. Decide every field jointly. Each answer must be exactly one of that field's allowed options.<|im_end|>\n<|im_start|>user\nSTATE:\n",
        "{\"a\":\"x\",\"b\":1.0}",
        "\n\nSCHEMA FIELDS:\n",
        "\nFIELD 1\nID: urgent\nTYPE: noul\nINSTRUCTION: urgent\nALLOWED OPTIONS:\n",
        "OPTION 1: {\"description\":\"The proposition is true or the answer is yes.\",\"option_id\":\"true\"}\n",
        "OPTION 2: {\"description\":\"The proposition is false or the answer is no.\",\"option_id\":\"false\"}\n",
        "END FIELD\n",
        "\nFIELD 2\nID: team\nTYPE: choice\nINSTRUCTION: {\"q\":\"who\"}\nALLOWED OPTIONS:\n",
        "OPTION 1: {\"description\":\"money\",\"option_id\":\"billing\"}\n",
        "OPTION 2: {\"option_id\":\"tech\"}\n",
        "END FIELD\n",
        "\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\nJOINT SCHEMA DECISIONS:",
    );
    assert_eq!(text, expected);
    let span_text = |(start, end): (usize, usize)| -> String {
        record.input_ids[start..end]
            .iter()
            .map(|id| char::from_u32(*id).unwrap())
            .collect()
    };
    assert_eq!(span_text(record.questions[0].question_span), "urgent");
    assert_eq!(
        span_text(record.questions[1].question_span),
        "{\"q\":\"who\"}"
    );
    assert_eq!(
        span_text(record.questions[1].option_spans[1]),
        "{\"option_id\":\"tech\"}"
    );
    assert_eq!(record.questions[1].option_ids, ["billing", "tech"]);
    assert_eq!(record.questions[1].question_type, 1);
}

#[test]
fn over_budget_is_refused_unless_state_tail_is_asked() {
    let body = r#"{"model":"m","state":"abcdefghij","questions":{"q":{"type":"noul"}}}"#;
    let req = request(body).expect("request");
    let full = encode_record(&req, &char_tokens, usize::MAX).expect("encode");
    let budget = full.input_ids.len() - 4;
    let refusal = encode_record(&req, &char_tokens, budget).unwrap_err();
    assert_eq!(refusal.prompt_tokens, full.input_ids.len());
    let mut tail = req.clone();
    tail.truncation = Truncation::StateTail;
    let cut = encode_record(&tail, &char_tokens, budget).expect("truncated");
    assert_eq!(cut.input_ids.len(), budget);
    assert_eq!(cut.truncated_state_tokens, 4);
    assert_eq!(
        cut.questions[0].question_span.0 + 4,
        full.questions[0].question_span.0
    );
    // A schema that alone is over the budget is refused even with state_tail.
    assert!(encode_record(&tail, &char_tokens, 10).is_err());
}

#[test]
fn refusals_separate_bad_questions_from_door_limits() {
    let invalid = |body: &str| matches!(request(body), Err(RequestRefusal::Invalid(_)));
    let not_admitted = |body: &str| matches!(request(body), Err(RequestRefusal::NotAdmitted(_)));
    assert!(invalid(
        r#"{"model":"m","questions":{"q":{"type":"noul"}}}"#
    ));
    assert!(invalid(r#"{"model":"m","state":"s","questions":{}}"#));
    assert!(invalid(
        r#"{"model":"m","state":"s","questions":{"q":{"type":"maybe"}}}"#
    ));
    assert!(invalid(
        r#"{"model":"m","state":"s","questions":{"q":{"type":"choice","criteria":{}}}}"#
    ));
    assert!(invalid(
        r#"{"model":"m","state":"s","questions":{"q":{"type":"choice","criteria":{"only":null}}}}"#
    ));
    assert!(invalid(
        r#"{"model":"m","state":"s","truncation":"middle","questions":{"q":{"type":"noul"}}}"#
    ));
    let many: String = (0..256)
        .map(|i| format!("\"o{i}\":null"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(not_admitted(&format!(
        r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"choice","criteria":{{{many}}}}}}}}}"#
    )));
    let ok: String = (0..255)
        .map(|i| format!("\"o{i}\":null"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(request(&format!(
        r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"choice","criteria":{{{ok}}}}}}}}}"#
    ))
    .is_ok());
    let questions: String = (0..65)
        .map(|i| format!("\"q{i}\":{{\"type\":\"noul\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(not_admitted(&format!(
        r#"{{"model":"m","state":"s","questions":{{{questions}}}}}"#
    )));
    assert!(not_admitted(
        r#"{"model":"m","state":"s","images":["aGk="],"questions":{"q":{"type":"noul"}}}"#
    ));
}

#[test]
fn answers_follow_the_reference_shape() {
    let req = request(
        r#"{"model":"m","state":"s","questions":{
            "u":{"type":"noul"},
            "t":{"type":"choice","criteria":{"b":null,"a":null,"c":null}},
            "l":{"type":"score","criteria":["low",{"what":"mid"},"high"]}}}"#,
    )
    .expect("request");
    let by = |pairs: &[(&'static str, f64)]| pairs.iter().copied().collect();
    let noul = answer_for(&req.questions[0], &by(&[("true", 0.75), ("false", 0.25)]));
    assert_eq!(noul, serde_json::json!({"type": "noul", "noul": 0.75}));
    // A tie goes to the first option in request order, as Python's max() does.
    let choice = answer_for(
        &req.questions[1],
        &by(&[("a", 0.4), ("b", 0.4), ("c", 0.2)]),
    );
    assert_eq!(choice["choice"], "b");
    assert_eq!(choice["confidence"], 0.4);
    let keys: Vec<&String> = choice["probabilities"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(keys, ["b", "a", "c"], "probabilities keep request order");
    let score = answer_for(
        &req.questions[2],
        &by(&[("0", 0.2), ("1", 0.3), ("2", 0.5)]),
    );
    assert!((score["score"].as_f64().unwrap() - 1.3).abs() < 1e-12);
    assert_eq!(score["confidence"], 0.5);
    assert_eq!(score["legend"]["1"], serde_json::json!({"what": "mid"}));
    assert_eq!(req.questions[2].kind, QuestionType::Score);
}

/// Zero differences against Cloudflare's `encode_record` on the public
/// corpus. Needs a Clef GGUF for its tokenizer: set `PSIONIC_CLEF_GGUF`.
#[test]
fn encoder_matches_reference_corpus() {
    let Ok(gguf) = std::env::var("PSIONIC_CLEF_GGUF") else {
        eprintln!("skipped: set PSIONIC_CLEF_GGUF to a Clef GGUF");
        return;
    };
    let content =
        psionic_models::GgufContent::read_path(std::path::Path::new(&gguf)).expect("gguf");
    let tokenizer = psionic_models::GgufRuntimeTokenizer::from_gguf(
        &content.load_tokenizer().expect("tokenizer"),
    )
    .expect("runtime tokenizer");
    let tokenize = |text: &str| -> Vec<u32> {
        tokenizer
            .encode_with_special_tokens(text, false, false)
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect()
    };
    let dir = fixtures().join("encoder");
    let corpus = std::fs::read_to_string(dir.join("corpus.jsonl")).expect("corpus");
    // Split in two to keep each fixture file under the repo's 1 MB cap.
    let expected = ["expected-000.jsonl", "expected-100.jsonl"]
        .iter()
        .map(|name| std::fs::read_to_string(dir.join(name)).expect("expected"))
        .collect::<String>();
    let mut records = 0;
    let mut failures = Vec::new();
    for (index, (body, want)) in corpus.lines().zip(expected.lines()).enumerate() {
        let req = request(body).expect("corpus request");
        let got = encode_record(&req, &tokenize, usize::MAX).expect("encode");
        let want: Value = serde_json::from_str(want).expect("expected json");
        let want_ids: Vec<u32> = want["input_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        let mut problems = Vec::new();
        if got.input_ids != want_ids {
            let first = got
                .input_ids
                .iter()
                .zip(&want_ids)
                .position(|(a, b)| a != b)
                .unwrap_or(got.input_ids.len().min(want_ids.len()));
            problems.push(format!(
                "ids differ at {first} (len {} vs {})",
                got.input_ids.len(),
                want_ids.len()
            ));
        }
        for (q, wq) in got
            .questions
            .iter()
            .zip(want["questions"].as_array().unwrap())
        {
            let span = |v: &Value| {
                (
                    v[0].as_u64().unwrap() as usize,
                    v[1].as_u64().unwrap() as usize,
                )
            };
            if q.question_id != wq["id"].as_str().unwrap()
                || q.question_type as u64 != wq["type"].as_u64().unwrap()
                || q.question_span != span(&wq["question_span"])
                || q.option_spans
                    != wq["option_spans"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(span)
                        .collect::<Vec<_>>()
                || q.option_ids
                    != wq["option_ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect::<Vec<_>>()
            {
                problems.push(format!("question {} differs", q.question_id));
            }
        }
        if !problems.is_empty() {
            failures.push(format!("record {index}: {}", problems.join("; ")));
        }
        records += 1;
    }
    eprintln!(
        "encoder parity: {records} records, {} differ",
        failures.len()
    );
    assert!(records >= 60, "corpus has {records} records");
    assert!(
        failures.is_empty(),
        "{} of {records} records differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Dumps what the head parity check needs for one public corpus record:
/// the prompt, spans, every final hidden row from this backbone, the
/// option tokens' LM-head rows, and the logits of the GGUF head and (with
/// `PSIONIC_CLEF_HF_HEAD`) the Hugging Face head on those same rows. The
/// torch f32 reference head runs on the dump in
/// `fixtures/clef/tools/head_parity.py`. Needs `PSIONIC_CLEF_GGUF` and
/// `PSIONIC_CLEF_DUMP_DIR`; `PSIONIC_CLEF_DUMP_RECORD` picks the line of
/// `fixtures/clef/e2e/requests.jsonl` (default 0).
#[test]
fn dump_head_parity_inputs() {
    let (Ok(gguf), Ok(out)) = (
        std::env::var("PSIONIC_CLEF_GGUF"),
        std::env::var("PSIONIC_CLEF_DUMP_DIR"),
    ) else {
        eprintln!("skipped: set PSIONIC_CLEF_GGUF and PSIONIC_CLEF_DUMP_DIR");
        return;
    };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).expect("dump dir");
    let index: usize = std::env::var("PSIONIC_CLEF_DUMP_RECORD")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let body = std::fs::read_to_string(fixtures().join("e2e/requests.jsonl"))
        .expect("requests")
        .lines()
        .nth(index)
        .expect("record")
        .to_string();
    let lane = super::ClefDecisionLane::load(
        std::path::Path::new(&gguf),
        super::ClefHeadSource::Embedded,
        super::ClefLimits::default(),
    )
    .expect("lane");
    let req = lane.parse_request(&body).expect("request");
    let record = lane.encode(&req).expect("encode");
    let mut hidden: Vec<f32> = Vec::new();
    let gguf_logits = lane
        .logits_with_rows(&record, &mut |_, row| hidden.extend_from_slice(row))
        .expect("logits");
    let bytes: Vec<u8> = hidden.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(out.join("hidden.f32"), bytes).expect("hidden");
    let mut option_tokens: Vec<u32> = record
        .questions
        .iter()
        .flat_map(|q| {
            q.option_spans
                .iter()
                .flat_map(|(s, e)| record.input_ids[*s..*e].to_vec())
        })
        .collect();
    option_tokens.sort_unstable();
    option_tokens.dedup();
    let rows = lane
        .backbone
        .output_embedding_rows(&option_tokens)
        .expect("rows");
    let lexical: serde_json::Map<String, Value> = option_tokens
        .iter()
        .zip(&rows)
        .map(|(id, row)| (id.to_string(), serde_json::json!(row)))
        .collect();
    let mut hf_logits = Value::Null;
    if let Ok(hf) = std::env::var("PSIONIC_CLEF_HF_HEAD") {
        let head =
            ClefHeadWeights::from_safetensors(std::path::Path::new(&hf), 4096).expect("hf head");
        let mut stream = ClefHeadStream::new(&head, &record);
        let d = head.config.hidden_size;
        for (index, row) in hidden.chunks(d).enumerate() {
            stream.push(index, row).expect("push");
        }
        let table: std::collections::HashMap<u32, Vec<f32>> = option_tokens
            .iter()
            .copied()
            .zip(rows.iter().cloned())
            .collect();
        let lexical_rows = |ids: &[u32]| Ok(ids.iter().map(|id| table[id].clone()).collect());
        hf_logits = serde_json::json!(stream.finish(&record, &lexical_rows).expect("hf logits"));
    }
    let dump = serde_json::json!({
        "input_ids": record.input_ids,
        "hidden_size": 4096,
        "questions": record.questions.iter().map(|q| serde_json::json!({
            "id": q.question_id, "type": q.question_type,
            "question_span": [q.question_span.0, q.question_span.1],
            "option_spans": q.option_spans.iter().map(|(s, e)| [*s, *e]).collect::<Vec<_>>(),
            "option_ids": q.option_ids,
        })).collect::<Vec<_>>(),
        "lexical": lexical,
        "gguf_head_logits": gguf_logits,
        "hf_head_logits": hf_logits,
    });
    std::fs::write(
        out.join("dump.json"),
        serde_json::to_vec(&dump).expect("json"),
    )
    .expect("dump");
}

/// Chunked prefill gives the same hidden rows as the token-at-a-time path.
/// Needs `PSIONIC_CLEF_GGUF` and `PSIONIC_CLEF_CHUNK_CHECK=1` (it runs the
/// slow path once).
#[test]
fn chunked_prefill_matches_token_at_a_time() {
    let (Ok(gguf), Ok(_)) = (
        std::env::var("PSIONIC_CLEF_GGUF"),
        std::env::var("PSIONIC_CLEF_CHUNK_CHECK"),
    ) else {
        eprintln!("skipped: set PSIONIC_CLEF_GGUF and PSIONIC_CLEF_CHUNK_CHECK");
        return;
    };
    let lane = super::ClefDecisionLane::load(
        std::path::Path::new(&gguf),
        super::ClefHeadSource::Embedded,
        super::ClefLimits::default(),
    )
    .expect("lane");
    let body = std::fs::read_to_string(fixtures().join("e2e/requests.jsonl"))
        .expect("requests")
        .lines()
        .nth(6)
        .expect("record")
        .to_string();
    let record = lane
        .encode(&lane.parse_request(&body).expect("request"))
        .expect("encode");
    let tokens: Vec<psionic_models::TokenId> = record
        .input_ids
        .iter()
        .map(|id| psionic_models::TokenId(*id))
        .collect();
    let rows = |chunk: usize| {
        let mut out = Vec::new();
        lane.backbone
            .stream_final_hidden_rows(&tokens, chunk, &mut |_, row| {
                out.extend_from_slice(row);
                Ok(())
            })
            .expect("rows");
        out
    };
    let one = rows(1);
    for chunk in [7, 64, 512] {
        let many = rows(chunk);
        let mut worst_cos = 1.0f64;
        for (a, b) in one.chunks(4096).zip(many.chunks(4096)) {
            let dot: f64 = a
                .iter()
                .zip(b)
                .map(|(x, y)| f64::from(*x) * f64::from(*y))
                .sum();
            let na: f64 = a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            let nb: f64 = b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            worst_cos = worst_cos.min(dot / (na * nb));
        }
        eprintln!("chunk {chunk}: min row cosine vs token-at-a-time {worst_cos:.8}");
        assert!(worst_cos > 0.9999, "chunk {chunk}: {worst_cos}");
    }
}

/// Per-layer parity dump: writes `l_out-N.f32` (every layer's residual
/// rows, `n x hidden`, before the output norm) and `result_norm.f32` for
/// the token ids in `PSIONIC_CLEF_LAYER_TOKENS` (whitespace-separated), in
/// the layout `lldump` writes for llama.cpp (see
/// `fixtures/clef/tools/layer_parity.py`). Needs `PSIONIC_CLEF_GGUF` and
/// `PSIONIC_CLEF_LAYER_DIR`; `PSIONIC_CLEF_LAYER_CHUNK` (default 256);
/// `PSIONIC_CLEF_LAYER_DEVICE=cuda` runs the CUDA trunk.
#[test]
fn dump_layer_rows() {
    let (Ok(gguf), Ok(tokens), Ok(out)) = (
        std::env::var("PSIONIC_CLEF_GGUF"),
        std::env::var("PSIONIC_CLEF_LAYER_TOKENS"),
        std::env::var("PSIONIC_CLEF_LAYER_DIR"),
    ) else {
        eprintln!("skipped: set PSIONIC_CLEF_GGUF, PSIONIC_CLEF_LAYER_TOKENS and PSIONIC_CLEF_LAYER_DIR");
        return;
    };
    let chunk: usize = std::env::var("PSIONIC_CLEF_LAYER_CHUNK")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(256);
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).expect("dump dir");
    let tokens: Vec<psionic_models::TokenId> = std::fs::read_to_string(tokens)
        .expect("tokens")
        .split_whitespace()
        .map(|value| psionic_models::TokenId(value.parse().expect("token id")))
        .collect();
    let mut layers: Vec<Vec<f32>> = Vec::new();
    let mut final_rows: Vec<f32> = Vec::new();
    let mut on_layer = |layer: usize, _: usize, rows: &[f32]| {
        if layers.len() <= layer {
            layers.resize_with(layer + 1, Vec::new);
        }
        layers[layer].extend_from_slice(rows);
    };
    if std::env::var("PSIONIC_CLEF_LAYER_DEVICE").as_deref() == Ok("cuda") {
        let lane = super::ClefDecisionLane::load(
            std::path::Path::new(&gguf),
            super::ClefHeadSource::Embedded,
            super::ClefLimits {
                device: gpu_device(),
                ..super::ClefLimits::default()
            },
        )
        .expect("lane");
        let trunk = lane.cuda.as_ref().expect("cuda trunk");
        let mut on_final = |_: usize, _: usize, rows: &[f32]| final_rows.extend_from_slice(rows);
        trunk
            .prefill(
                &lane.backbone,
                &tokens,
                &[],
                chunk,
                Some(&mut on_layer),
                Some(&mut on_final),
            )
            .expect("prefill");
    } else {
        let backbone =
            crate::CpuGgufQwen35TextGenerationService::from_gguf_path(&gguf).expect("load");
        backbone
            .stream_layer_rows(
                &tokens,
                chunk,
                &mut |_, row| {
                    final_rows.extend_from_slice(row);
                    Ok(())
                },
                &mut on_layer,
            )
            .expect("prefill");
    }
    let write = |name: String, values: &[f32]| {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(out.join(name), bytes).expect("write");
    };
    for (layer, rows) in layers.iter().enumerate() {
        write(format!("l_out-{layer}.f32"), rows);
    }
    write(String::from("result_norm.f32"), &final_rows);
}

/// The GPU the device tests run on: `PSIONIC_CLEF_TEST_DEVICE=metal` on a
/// Mac, CUDA otherwise.
fn gpu_device() -> super::ClefDevice {
    match std::env::var("PSIONIC_CLEF_TEST_DEVICE").as_deref() {
        Ok("metal") => super::ClefDevice::Metal,
        _ => super::ClefDevice::Cuda,
    }
}

/// CUDA trunk: chunk sizes {whole, 2048, 512, 64} give the same argmax and
/// logits within the bounds below, a repeat is bitwise identical, and
/// with f32 accumulation the CUDA lane matches the CPU lane within the M1
/// tolerance (|dp| <= 0.02). Needs `PSIONIC_CLEF_GGUF` and
/// `PSIONIC_CLEF_CUDA_CHECK=1` and a CUDA device.
#[test]
fn cuda_chunks_and_cpu_agree() {
    let (Ok(gguf), Ok(_)) = (
        std::env::var("PSIONIC_CLEF_GGUF"),
        std::env::var("PSIONIC_CLEF_CUDA_CHECK"),
    ) else {
        eprintln!("skipped: set PSIONIC_CLEF_GGUF and PSIONIC_CLEF_CUDA_CHECK");
        return;
    };
    let path = std::path::Path::new(&gguf);
    let load = |device, accumulate_f16| {
        super::ClefDecisionLane::load(
            path,
            super::ClefHeadSource::Embedded,
            super::ClefLimits {
                device,
                accumulate_f16,
                ..super::ClefLimits::default()
            },
        )
        .expect("lane")
    };
    let corpus = std::fs::read_to_string(fixtures().join("encoder/corpus.jsonl")).expect("corpus");
    let requests = std::fs::read_to_string(fixtures().join("e2e/requests.jsonl")).expect("requests");
    let short = requests.lines().nth(6).expect("record").to_string();
    let worst = |a: &[Vec<f32>], b: &[Vec<f32>]| {
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max)
    };
    let argmax = |logits: &[Vec<f32>]| {
        logits
            .iter()
            .map(|q| {
                q.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>()
    };
    let mut failures = Vec::new();
    for accumulate_f16 in [true, false] {
        let lane = load(gpu_device(), accumulate_f16);
        // the longest corpus record that the default budget admits
        let long = corpus
            .lines()
            .filter_map(|line| {
                let record = lane.encode(&lane.parse_request(line).ok()?).ok()?;
                Some((record.input_ids.len(), line.to_string()))
            })
            .max_by_key(|(len, _)| *len)
            .expect("long record")
            .1;
        for body in [&short, &long] {
            let record = lane.encode(&lane.parse_request(body).expect("request")).expect("encode");
            let length = record.input_ids.len();
            let whole = lane.logits_at_chunk(&record, length, None, None).expect("whole");
            let again = lane.logits_at_chunk(&record, length, None, None).expect("repeat");
            assert_eq!(whole, again, "a repeat is bitwise identical");
            for chunk in [2048, 512, 64] {
                let logits = lane.logits_at_chunk(&record, chunk, None, None).expect("chunk");
                let delta = worst(&whole, &logits);
                eprintln!(
                    "accumulate_f16={accumulate_f16} tokens={length} chunk={chunk}: max |dlogit| {delta:.2e}"
                );
                // f32 accumulation: the M2 bound (1e-3) on short prompts,
                // 2e-3 on long ones (f16 KV and per-shape GEMM rounding);
                // f16 accumulation: 5e-2 (measured ~2e-2).
                let bound = if accumulate_f16 { 5e-2 } else if length > 2048 { 2e-3 } else { 1e-3 };
                if delta > bound || argmax(&whole) != argmax(&logits) {
                    failures.push(format!("accumulate_f16={accumulate_f16} tokens={length} chunk={chunk}: {delta}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
    // CUDA (f32 accumulate) vs the CPU lane on the short record.
    let cuda = load(gpu_device(), false);
    let cpu = load(super::ClefDevice::Cpu, false);
    let record = cpu.encode(&cpu.parse_request(&short).expect("request")).expect("encode");
    let a = cuda.logits(&record).expect("cuda");
    let b = cpu.logits(&record).expect("cpu");
    let dp = a
        .iter()
        .zip(&b)
        .flat_map(|(x, y)| {
            super::head::probabilities(x)
                .into_iter()
                .zip(super::head::probabilities(y))
                .map(|(p, q)| (p - q).abs())
        })
        .fold(0.0f64, f64::max);
    eprintln!("cuda vs cpu: max |dlogit| {:.2e}, max |dp| {dp:.2e}", worst(&a, &b));
    assert!(dp <= 0.02, "{dp}");
    assert_eq!(argmax(&a), argmax(&b));
}
