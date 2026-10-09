use super::*;
use knowledge::search::EmbedError;

/// Bag-of-words vectors: deterministic, offline, and enough to rank the
/// fixtures. Only tests use it; production ranks with a real embedding
/// model.
pub(super) struct Words;

impl Embed for Words {
    fn model(&self) -> &str {
        "words-64"
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<(Vec<Vec<f32>>, Option<f64>), EmbedError> {
        let vectors = inputs
            .iter()
            .map(|text| {
                let mut vector = vec![0.0_f32; 64];
                for word in knowledge::search::words(text) {
                    let bucket = word
                        .bytes()
                        .fold(7_usize, |h, b| h.wrapping_mul(31).wrapping_add(b.into()));
                    vector[bucket % 64] += 1.0;
                }
                vector
            })
            .collect();
        Ok((vectors, Some(0.0)))
    }
}

impl ProductKb for ProductKnowledge<Words> {
    fn available(&self) -> bool {
        !self.corpus.base.entries.is_empty()
    }
    fn recipients(&self) -> Vec<String> {
        vec![self.recipient.clone()]
    }
    fn ground<'a>(&'a self, lookup: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async move { self.find(lookup).await.map(|found| found.grounding) })
    }
}

/// A judge that finds the entry titled `title` relevant at `relevance`,
/// every other entry at 0.1, and picks its answer at `pick`.
struct Sure {
    title: String,
    relevance: f64,
    pick: f64,
}

impl Judge for Sure {
    fn judge(
        &self,
        request: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        let state = request.state.to_value();
        let entries = state["entries"].as_object().cloned().unwrap_or_default();
        let target = entries
            .iter()
            .find(|(_, e)| e["title"] == self.title.as_str())
            .map(|(k, _)| k.clone());
        let mut answers = serde_json::Map::new();
        for key in entries.keys() {
            let n = key.trim_start_matches("entry_");
            let p = if Some(key) == target.as_ref() {
                self.relevance
            } else {
                0.1
            };
            answers.insert(format!("relevant_{n}"), json!({"type": "noul", "noul": p}));
        }
        if let Some(jev::Question::Choice(choice)) = request.questions.get("answer") {
            let chosen = target.clone().unwrap_or_else(|| "none".to_string());
            let options: Vec<String> = choice.criteria.keys().cloned().collect();
            let probabilities: serde_json::Map<String, Value> = options
                .iter()
                .map(|o| {
                    let p = if *o == chosen {
                        self.pick
                    } else {
                        (1.0 - self.pick) / (options.len() - 1) as f64
                    };
                    (o.clone(), json!(p))
                })
                .collect();
            answers.insert(
                "answer".to_string(),
                json!({"type": "choice", "choice": chosen, "confidence": self.pick, "probabilities": probabilities}),
            );
        }
        let bytes = json!({"model": "jev-test", "answers": answers})
            .to_string()
            .into_bytes();
        Box::pin(async move {
            jev::SystemOneResponse::decode(jev::RawResponse {
                status: 200,
                headers: Default::default(),
                bytes,
            })
            .map_err(|e| e.to_string())
        })
    }
}

/// A judge that always fails.
struct Down;

impl Judge for Down {
    fn judge(
        &self,
        _: jev::SystemOneRequest,
    ) -> BoxFuture<'_, Result<jev::SystemOneResponse, String>> {
        Box::pin(async { Err("connection refused".to_string()) })
    }
}

pub(super) fn committed() -> Corpus {
    Corpus::load(&product::default_dir(), Some(&product::repository())).expect("the corpus loads")
}

fn kb(judge: impl Judge + 'static) -> ProductKnowledge<Words> {
    ProductKnowledge::new(committed(), Words, "Test embeddings", Arc::new(judge))
}

fn lookup(message: &str) -> Lookup {
    Lookup {
        message: message.to_string(),
        transcript: vec![Message {
            role: Role::User,
            text: message.to_string(),
        }],
    }
}

#[tokio::test]
async fn a_sure_entry_carries_its_reviewed_answer() {
    let kb = kb(Sure {
        title: "Why amounts show as whole ₿ numbers".into(),
        relevance: 0.95,
        pick: 0.9,
    });
    assert!(kb.available());
    assert_eq!(kb.recipients(), ["Test embeddings"]);
    let grounding = kb
        .ground(&lookup(
            "why does the wallet show amounts like ₿10,000 instead of BTC",
        ))
        .await
        .expect("a grounding");
    let first = &grounding.passages[0];
    assert_eq!(first.id, "openagents.wallet-amounts@1");
    assert!(first.relevance >= KB_ANSWER_CONFIDENCE);
    assert!(
        first
            .answer
            .as_deref()
            .is_some_and(|a| a.contains("BIP 177"))
    );
    assert_eq!(first.source, "docs/breez/amounts.md");
    assert!(
        grounding
            .passages
            .iter()
            .skip(1)
            .all(|p| p.answer.is_none())
    );
    assert!(grounding.passages.len() <= KEEP);
}

#[tokio::test]
async fn an_unsure_pick_grounds_the_model_instead_of_answering() {
    let kb = kb(Sure {
        title: "Why amounts show as whole ₿ numbers".into(),
        relevance: 0.95,
        pick: 0.6,
    });
    let found = kb
        .find(&lookup(
            "why does the wallet show amounts like ₿10,000 instead of BTC",
        ))
        .await
        .expect("found");
    assert_eq!(
        found.grounding.passages[0].id,
        "openagents.wallet-amounts@1"
    );
    assert!(found.grounding.passages[0].answer.is_none());
    assert_eq!(
        found.answer.as_ref().map(|(id, _)| id.as_str()),
        Some("openagents.wallet-amounts")
    );
    assert_eq!(found.candidates.len(), CANDIDATES);
}

#[tokio::test]
async fn nothing_relevant_keeps_nothing() {
    let kb = kb(Sure {
        title: "no such entry".into(),
        relevance: 0.9,
        pick: 0.9,
    });
    let grounding = kb
        .ground(&lookup("how do I turn on dark mode"))
        .await
        .expect("a grounding");
    assert!(grounding.passages.is_empty());
    assert_eq!(
        instructions(&grounding),
        knowledge::product::NO_DOCUMENTED_ANSWER
    );
}

#[tokio::test]
async fn a_failed_judge_is_a_failure_that_names_no_message_text() {
    let kb = kb(Down);
    let error = kb
        .ground(&lookup("my secret project name is bluebird"))
        .await
        .expect_err("the judge is down");
    let SeamError::Failed(why) = error else {
        panic!("a failure, not unavailable");
    };
    assert!(!why.contains("bluebird"), "{why}");
}

#[test]
fn the_questions_ask_relevance_for_each_candidate_and_one_answer_choice() {
    let corpus = committed();
    let entries: Vec<&knowledge::Entry> = corpus.base.entries.iter().take(3).collect();
    let questions = questions(&entries);
    assert_eq!(questions.len(), 4);
    for n in 1..=3 {
        assert!(matches!(
            questions.get(&format!("relevant_{n}")),
            Some(jev::Question::Noul(_))
        ));
    }
    let Some(jev::Question::Choice(choice)) = questions.get("answer") else {
        panic!("an answer choice");
    };
    assert_eq!(
        choice.criteria.keys().collect::<Vec<_>>(),
        ["entry_1", "entry_2", "entry_3", "none"]
    );
    questions.validate().expect("valid questions");
}

#[test]
fn the_state_carries_the_latest_message_earlier_turns_and_the_candidates() {
    let corpus = committed();
    let entries: Vec<&knowledge::Entry> = corpus.base.entries.iter().take(2).collect();
    let lookup = Lookup {
        message: "and on Android?".into(),
        transcript: vec![
            Message {
                role: Role::User,
                text: "how do I back up my wallet".into(),
            },
            Message {
                role: Role::Assistant,
                text: "x".repeat(1_000),
            },
            Message {
                role: Role::User,
                text: "and on Android?".into(),
            },
        ],
    };
    let state = state(&lookup, &entries);
    assert_eq!(state["latest_message"], "and on Android?");
    let earlier = state["earlier_turns"].as_array().expect("earlier turns");
    assert_eq!(earlier.len(), 2);
    assert_eq!(earlier[0]["text"], "how do I back up my wallet");
    assert_eq!(
        earlier[1]["text"].as_str().map(|t| t.chars().count()),
        Some(TURN_CHARS)
    );
    assert_eq!(
        state["entries"]["entry_1"]["title"],
        entries[0].title.as_str()
    );
    assert!(state["entries"]["entry_2"]["answer"].is_string());
}

#[test]
fn a_grounded_reply_is_checked_against_the_passages_it_was_given() {
    let grounding = Grounding {
        passages: vec![Passage {
            id: "openagents.wallet-send@1".into(),
            title: "Sending bitcoin".into(),
            text: "Send takes an invoice.".into(),
            source: "bins/openagents-ios/README.md".into(),
            relevance: 0.9,
            answer: None,
            off_computer: false,
        }],
        commit: None,
        needs_dispatch: false,
    };
    let text = instructions(&grounding);
    assert!(text.contains("<entry id=\"openagents.wallet-send\""));
    assert!(text.contains("Send takes an invoice."));
    let checked = cited(
        "Choose Send [openagents.wallet-send]. Fees are zero [openagents.fees].",
        &grounding,
    );
    assert_eq!(checked.known, ["openagents.wallet-send"]);
    assert_eq!(checked.unknown, ["openagents.fees"]);
}

#[test]
fn the_held_out_questions_name_only_committed_entries() {
    let corpus = committed();
    let questions = eval::questions();
    assert_eq!(
        questions.iter().filter(|q| !q.expect.is_empty()).count(),
        100
    );
    assert!(questions.iter().any(|q| q.expect.is_empty()));
    for question in &questions {
        for id in &question.expect {
            assert!(corpus.get(id).is_some(), "{}: {id}", question.id);
        }
    }
}

/// #11031: a reviewed answer is shown to the person as written, so no
/// committed entry's answer carries machine talk.
#[test]
fn reviewed_answers_have_no_machine_talk() {
    let corpus = committed();
    let mut hits = Vec::new();
    for entry in &corpus.base.entries {
        let Some(answer) = &entry.answer else {
            continue;
        };
        // The Test-Time Capabilities essay defines "admission" as one of
        // its terms; its entries answer questions about that essay.
        let allow: &[&str] = if entry.id.starts_with("openagents.ttc-") {
            &["admission", "admitted"]
        } else {
            &[]
        };
        for hit in oa_copy::violations(answer, allow) {
            hits.push(format!(
                "{}: {:?} in \"{}\"",
                entry.id, hit.term, hit.context
            ));
        }
    }
    assert!(
        hits.is_empty(),
        "machine talk in product answers:\n{}",
        hits.join("\n")
    );
}

/// A grounded reply's `[openagents.…]` citations, with or without a
/// version, never reach the phone: not in the final text and not in any
/// streamed piece, however the stream splits them, and a line that held
/// only citations goes with them. A bracket that is not a citation stays.
#[test]
fn product_citations_are_taken_out_as_the_reply_streams() {
    let cases = [
        (
            "We'll look that up for you. Open the desktop app and scan its code \
             [openagents.connect-computer@1].\n\n```bash\nopenagents connect invite\n```\n\
             [openagents.connect-computer@1]\n\nNo Tailscale is needed \
             [openagents.connect-computer@1, openagents.tailnet@2].",
            "We'll look that up for you. Open the desktop app and scan its code.\n\n```bash\n\
             openagents connect invite\n```\n\nNo Tailscale is needed.",
        ),
        (
            "Install it from [openagents.com] [openagents.get-the-app] and see [the guide](x).",
            "Install it from [openagents.com] and see [the guide](x).",
        ),
        (
            "Pair it [openagents.connect-computer] and go.",
            "Pair it and go.",
        ),
        (
            "Arrays like [1, 2] stay [openagents.cli@3]",
            "Arrays like [1, 2] stay",
        ),
        // A comma that only joined two citations goes with them (#10102).
        (
            "Admitted into the run [openagents.ttc-overview], [openagents.ttc-thesis]. Next \
             [openagents.a]; [openagents.b], and on [openagents.c], [x] too.",
            "Admitted into the run. Next, and on, [x] too.",
        ),
    ];
    for (reply, want) in cases {
        assert_eq!(tidy(reply), want, "{reply}");
        for size in 1..=9 {
            let mut stream = tidier();
            let chars: Vec<char> = reply.chars().collect();
            let mut out = String::new();
            for chunk in chars.chunks(size) {
                let shown = stream.push(&chunk.iter().collect::<String>());
                assert!(!shown.contains("[openagents.c") || shown.contains("[openagents.com]"));
                out.push_str(&shown);
            }
            out.push_str(&stream.finish());
            assert_eq!(out, want, "split every {size}");
        }
    }
    // The model's citations are still read before they are taken out.
    let grounding = Grounding {
        passages: vec![Passage {
            id: "openagents.connect-computer@1".into(),
            title: "Connecting".into(),
            text: "Scan the code.".into(),
            source: "knowledge/openagents/openagents.connect-computer.md".into(),
            relevance: 0.9,
            answer: None,
            off_computer: false,
        }],
        ..Grounding::default()
    };
    let read = cited(cases[0].0, &grounding);
    assert_eq!(read.known, vec!["openagents.connect-computer".to_string()]);
    assert_eq!(read.unknown, vec!["openagents.tailnet".to_string()]);
}

/// A copy lent to a job on the caller's keys (BYOK) reads our vectors and
/// never embeds the corpus itself: before ours are made, its lookups fail
/// (the router answers past the seam); after, it grounds with its own
/// embedder and judge.
#[tokio::test]
async fn a_lent_copy_reads_our_vectors_and_never_indexes_the_corpus() {
    let ours = kb(Down);
    let sure = || Sure {
        title: "Why amounts show as whole ₿ numbers".into(),
        relevance: 0.95,
        pick: 0.9,
    };
    let lent = ours.lent(Words, "Their embeddings", Arc::new(sure()));
    assert_eq!(lent.recipients(), ["Their embeddings"]);
    let message = "why does the wallet show amounts like ₿10,000 instead of BTC";
    assert!(
        lent.ground(&lookup(message)).await.is_err(),
        "a lent copy does not index our corpus on their keys"
    );
    ours.warm().await.expect("our vectors");
    let grounding = lent.ground(&lookup(message)).await.expect("a grounding");
    assert_eq!(grounding.passages[0].id, "openagents.wallet-amounts@1");
}
