//! The conversation view. It lives in `openagents_chat_view::transcript`,
//! shared with the web; the desktop draws its media through
//! [`crate::transcript_media::DesktopMedia`].

pub use openagents_chat_view::transcript::*;

/// The desktop's transcript.
pub type Transcript = TranscriptView<crate::transcript_media::DesktopMedia>;

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::ops::Range;
    use std::rc::Rc;
    use std::sync::{Arc, Weak};
    use std::time::{Duration, Instant};

    use gpui::{
        AnyElement, Bounds, ClipboardItem, ContentMask, Context, ListAlignment, ListOffset,
        ListScrollEvent, ListState, MouseButton, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels,
        Point, SharedString, StyledText, Subscription, Task, TextAlign, TextRun, Window, canvas,
        div, list, point, prelude::*, px, size,
    };

    use openagents_doc::{MessageRole, MessageStatus, SessionMessageEntry, SubagentStatus};
    use openagents_proto::ToolCall;

    use crate::markdown::parser::{
        Block, BlockTree, IncrementalParser, InlineRun, InlineStyle, parse_full,
    };
    use crate::markdown::render::{self, RenderCache, RenderOptions};
    use crate::markdown::veil::RowVeil;
    use crate::motion::{self, AnimationExt as _};
    use crate::notice::{NoticeChipIcon::Tile, notice_chip};
    use crate::state::AppState;
    use crate::syntax_cache::{DocumentHighlightKey, SyntaxHighlightCache};
    use crate::theme::Theme;
    use crate::transcript::host::TranscriptHost;
    use crate::transcript_media::DesktopMedia;
    use gpui::Entity;
    use openagents_syntax::LanguageId as Lang;

    #[test]
    fn jump_button_stays_available_when_scrolling_down_until_near_bottom() {
        let mut shown = false;
        for distance in [500.0, 330.0, 319.0, 200.0, 100.0] {
            shown = jump_visibility(shown, distance);
            assert!(shown, "button vanished with {distance}px remaining");
        }
        assert!(!jump_visibility(shown, AT_BOTTOM_PX));
        assert!(!jump_visibility(false, 319.0));
        assert!(jump_visibility(false, 321.0));
    }

    #[gpui::test]
    fn departing_transcript_is_retained_only_until_hidden(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
            let state = cx.new(|_| AppState::new());
            let transcript =
                cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
            transcript.update(cx, |transcript, cx| {
                transcript.retain_for_route_exit();
                transcript.chat_id = Some("departing".into());
                transcript.last_source = None;
                transcript.rows = vec![viewport_row("row", "message")];
                transcript.list.reset(1);
                transcript.sync(cx);
                assert_eq!(transcript.rows.len(), 1);
                assert!(transcript.route_exit_pending(cx));
                transcript.finish_route_exit(cx);
                assert!(transcript.rows.is_empty());
                assert!(transcript.chat_id.is_none());
                assert!(!transcript.route_exit_pending(cx));
            });
        });
    }
    use openagents_doc::MessagePart;

    fn with_tool_group_navigation(
        cx: &mut gpui::TestAppContext,
        run: impl FnOnce(Entity<AppState>, Entity<Transcript>, &mut gpui::App),
    ) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
            let state = cx.new(|_| AppState::new());
            let transcript =
                cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
            transcript.update(cx, |this, _| this.retain_for_route_exit());
            replay_tool_group(&state, &transcript, "chat-a", cx);
            run(state, transcript, cx);
        });
    }

    fn replay_tool_group(
        state: &Entity<AppState>,
        transcript: &Entity<Transcript>,
        chat: &str,
        cx: &mut gpui::App,
    ) {
        state.update(cx, |state, _| {
            state.selected_chat = Some(chat.into());
            state.transcript_replayed = true;
            state.transcript = vec![assistant(
                "tools",
                MessageStatus::Complete,
                vec![tool_part("call", "pwd")],
            )];
            state.transcript_revision += 1;
        });
        transcript.update(cx, |this, cx| this.sync(cx));
    }

    fn assert_replayed_group_is_closed(transcript: &Entity<Transcript>, cx: &mut gpui::App) {
        transcript.update(cx, |this, cx| {
            let row = this
                .rows
                .iter()
                .find(|row| matches!(row.kind, RowKind::ToolGroup { .. }))
                .unwrap()
                .clone();
            let RowKind::ToolGroup {
                tools,
                auto_open,
                summary,
                ..
            } = &row.kind
            else {
                unreachable!()
            };
            assert!(!auto_open);
            let _ = this.render_tool_group(
                &row.id,
                tools,
                summary,
                *auto_open,
                None,
                false,
                &Theme::dark(),
                cx,
            );
            let reveal = &this.tool_group_reveals[&row.id];
            assert_eq!(
                reveal.rendered_open,
                Some(false),
                "history flashed open on its first render"
            );
            assert_eq!(
                reveal.rendered_height, 0.0,
                "history replayed a stale closing tween"
            );
            assert!(reveal.header_started_at.is_none());
            assert!(reveal.starts.iter().all(Option::is_none));
        });
    }

    #[gpui::test]
    fn tool_groups_stay_closed_on_populated_chat_attach(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            for chat in ["chat-b", "chat-a", "chat-b"] {
                // Selection and cached replay can coalesce into a single sync.
                replay_tool_group(&state, &transcript, chat, cx);
                assert_replayed_group_is_closed(&transcript, cx);
            }
        });
    }

    #[gpui::test]
    fn tool_groups_stay_closed_after_rapid_new_chat_navigation(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            for finish_exit in [false, true] {
                transcript.update(cx, |this, _| {
                    // Navigate away during the previous close animation.
                    this.folds.insert(
                        "tools#g0".into(),
                        FoldState {
                            open: Some(false),
                            from: 120.0,
                            toggled_at: Some(Instant::now()),
                            disclosure_at: Some(Instant::now()),
                            ..Default::default()
                        },
                    );
                });
                state.update(cx, |state, cx| state.select_chat(None, cx));
                transcript.update(cx, |this, cx| {
                    this.sync(cx);
                    if finish_exit {
                        this.finish_route_exit(cx);
                    }
                });
                state.update(cx, |state, cx| state.select_chat(Some("chat-a".into()), cx));
                transcript.update(cx, |this, cx| this.sync(cx));
                replay_tool_group(&state, &transcript, "chat-a", cx);
                assert_replayed_group_is_closed(&transcript, cx);
            }
        });
    }

    #[gpui::test]
    fn tool_group_revisit_skips_batched_history_but_animates_live_arrivals(
        cx: &mut gpui::TestAppContext,
    ) {
        use openagents_doc::transcript_delta::{TranscriptFrame, diff_transcript};

        with_tool_group_navigation(cx, |state, transcript, cx| {
            let apply_frame = |frame, replay_baseline, cx: &mut gpui::App| {
                state.update(cx, |state, cx| {
                    state
                        .receive_transcript_update(
                            openagents_doc::TranscriptUpdate {
                                frame,
                                replay_baseline,
                                context_usage: None,
                            },
                            cx,
                        )
                        .unwrap();
                });
                transcript.update(cx, |this, cx| this.sync(cx));
            };
            let cached = vec![assistant(
                "tools",
                MessageStatus::Streaming,
                vec![tool_part("call", "pwd")],
            )];
            apply_frame(
                TranscriptFrame::reset(&cached),
                Some(openagents_doc::TranscriptBaseline::capture(&cached)),
                cx,
            );
            state.update(cx, |state, cx| state.select_chat(Some("chat-b".into()), cx));
            transcript.update(cx, |this, cx| this.sync(cx));

            // The agent does this work while A is away. On return, a stale
            // local snapshot can arrive before remote catch-up deltas. Keep
            // the same streaming message/group throughout: its status alone
            // cannot distinguish historical tools from new live arrivals.
            let mut first_batch = cached.clone();
            first_batch[0].parts.extend([
                tool_part("away-1", "ls"),
                tool_part("away-2", "git status --short"),
            ]);
            let mut second_batch = first_batch.clone();
            second_batch[0].parts.extend([
                tool_part("away-3", "git diff --stat"),
                tool_part("away-4", "git log -1"),
            ]);

            state.update(cx, |state, cx| state.select_chat(Some("chat-a".into()), cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            assert!(
                !transcript.read(cx).rows.is_empty(),
                "revisited transcript must have visible rows before the new watch responds"
            );
            apply_frame(
                TranscriptFrame::reset(&cached),
                Some(openagents_doc::TranscriptBaseline::capture(&cached)),
                cx,
            );
            let row_id: SharedString = "tools#g0".into();
            {
                let reveal = &transcript.read(cx).tool_group_reveals[&row_id];
                assert_eq!(reveal.starts.len(), 1);
                assert!(reveal.header_started_at.is_none());
                assert!(reveal.starts.iter().all(Option::is_none));
            }

            // Drive the production update reducer with the historical cutoff
            // supplied by the engine on each backfill batch. Inspect epochs
            // without sleeping or advancing
            // frames, so slow test machines cannot hide a replayed entrance.
            let mut historical_entrances = Vec::new();
            for (previous, next) in [(&cached, &first_batch), (&first_batch, &second_batch)] {
                let frame = diff_transcript(previous, next);
                assert!(matches!(&frame, TranscriptFrame::Delta { .. }));
                apply_frame(
                    frame,
                    Some(openagents_doc::TranscriptBaseline::capture(next)),
                    cx,
                );
                let reveal = &transcript.read(cx).tool_group_reveals[&row_id];
                assert_eq!(reveal.starts.len(), next[0].parts.len());
                assert!(reveal.header_started_at.is_none());
                historical_entrances.push(reveal.starts.iter().flatten().count());
            }

            // Once caught up, a genuinely new tool in that same group still
            // needs its entrance. Check this before reporting accumulated
            // history failures, so both halves of the contract are exercised.
            let mut live = second_batch.clone();
            live[0].parts.push(tool_part("live-call", "git diff"));
            apply_frame(diff_transcript(&second_batch, &live), None, cx);
            let reveal = &transcript.read(cx).tool_group_reveals[&row_id];
            assert_eq!(reveal.starts.len(), 6);
            assert!(
                reveal.starts[5].is_some(),
                "a new live tool must retain its entrance after catch-up"
            );
            assert!(reveal.header_started_at.is_none());
            assert_eq!(
                historical_entrances,
                vec![0, 0],
                "tools accumulated while away must not acquire entrance animations in either catch-up batch"
            );
        });
    }

    #[test]
    fn background_preparation_reuses_unchanged_rows_and_replaces_same_length_text() {
        let mut worker = TranscriptPreparation::default();
        let original = vec![
            assistant(
                "a",
                MessageStatus::Complete,
                vec![MessagePart::Text {
                    id: "p".into(),
                    text: "alpha".into(),
                }],
            ),
            assistant("b", MessageStatus::Complete, vec![tool_part("t", "pwd")]),
        ];
        let first = worker
            .prepare(&openagents_doc::TranscriptUpdate {
                frame: openagents_doc::TranscriptFrame::reset(&original),
                context_usage: None,
                replay_baseline: Some(openagents_doc::TranscriptBaseline::capture(&original)),
            })
            .unwrap();
        let mut changed = original.clone();
        changed[0].parts = vec![MessagePart::Text {
            id: "p".into(),
            text: "omega".into(),
        }];
        let next = worker
            .prepare(&openagents_doc::TranscriptUpdate {
                frame: openagents_doc::diff_transcript(&original, &changed),
                context_usage: None,
                replay_baseline: None,
            })
            .unwrap();
        assert!(Arc::ptr_eq(&first.rows["b"], &next.rows["b"]));
        assert!(!Arc::ptr_eq(&first.rows["a"], &next.rows["a"]));
        assert!(diff_rows(&first.rows["a"], &next.rows["a"]).is_some());
    }

    #[gpui::test]
    fn prepared_whale_open_and_revisit_do_not_build_rows_on_ui(cx: &mut gpui::TestAppContext) {
        let (update, prepared, preparation_ms) = std::thread::spawn(|| {
            let entries = if let Ok(path) = openagents_brand::env_var("OPENAGENTS_WHALE_SNAPSHOT") {
                let doc = openagents_doc::SessionDoc::init("fixture").unwrap();
                doc.doc().import(&std::fs::read(path).unwrap()).unwrap();
                openagents_doc::join_continuation_entries(doc.read_entries().unwrap())
            } else {
                vec![assistant("whale-turn", MessageStatus::Complete, (0..5000).map(|i| {
                    MessagePart::Text { id: format!("part-{i}"), text: format!("## Result {i}\n\n**Markdown** with `code` and [links](https://example.com).\n") }
                }).collect())]
            };
            let update = openagents_doc::TranscriptUpdate {
                replay_baseline: Some(openagents_doc::TranscriptBaseline::capture(&entries)),
                frame: openagents_doc::TranscriptFrame::Reset { reset: entries },
                context_usage: None,
            };
            let start = Instant::now();
            let prepared = TranscriptPreparation::default().prepare(&update).unwrap();
            (update, prepared, start.elapsed().as_millis())
        }).join().unwrap();
        with_tool_group_navigation(cx, |state, transcript, cx| {
            state.update(cx, |state, cx| state.select_chat(Some("whale".into()), cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            FORBID_ROW_PREPARATION.with(|flag| flag.set(true));
            let start = Instant::now();
            state.update(cx, |state, cx| {
                state
                    .receive_opening_transcript_update(update, false, cx)
                    .unwrap();
                state
                    .prepared_transcripts
                    .insert("whale".into(), prepared.clone());
            });
            transcript.update(cx, |this, cx| this.sync(cx));
            let open_ms = start.elapsed().as_millis();
            assert!(!transcript.read(cx).rows.is_empty());
            assert!(
                transcript
                    .read(cx)
                    .tool_group_reveals
                    .values()
                    .all(|r| r.starts.iter().all(Option::is_none))
            );
            let start = Instant::now();
            state.update(cx, |state, cx| state.select_chat(None, cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            state.update(cx, |state, cx| state.select_chat(Some("whale".into()), cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            let revisit_ms = start.elapsed().as_millis();
            assert!(Arc::ptr_eq(
                &state.read(cx).prepared_transcripts["whale"],
                &prepared
            ));
            assert!(
                transcript
                    .read(cx)
                    .tool_group_reveals
                    .values()
                    .all(|r| r.starts.iter().all(Option::is_none))
            );
            FORBID_ROW_PREPARATION.with(|flag| flag.set(false));
            eprintln!(
                "background_preparation_ms={preparation_ms} ui_open_ms={open_ms} ui_revisit_ms={revisit_ms}"
            );
        });
    }

    #[gpui::test]
    fn opening_tail_full_history_and_cached_revisit_never_replay_tool_entrances(
        cx: &mut gpui::TestAppContext,
    ) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            state.update(cx, |state, cx| state.select_chat(Some("whale".into()), cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            let preview = vec![assistant(
                "turn",
                MessageStatus::Streaming,
                vec![tool_part("tail-tool", "pwd")],
            )];
            let full = vec![assistant(
                "turn",
                MessageStatus::Streaming,
                vec![
                    tool_part("older-tool", "ls"),
                    MessagePart::Text {
                        id: "separator".into(),
                        text: "Earlier result".into(),
                    },
                    tool_part("tail-tool", "pwd"),
                ],
            )];
            let apply = |entries: &[SessionMessageEntry], pending, cx: &mut gpui::App| {
                state.update(cx, |state, cx| {
                    state
                        .receive_opening_transcript_update(
                            openagents_doc::TranscriptUpdate {
                                frame: openagents_doc::TranscriptFrame::reset(entries),
                                replay_baseline: Some(openagents_doc::TranscriptBaseline::capture(
                                    entries,
                                )),
                                context_usage: None,
                            },
                            pending,
                            cx,
                        )
                        .unwrap();
                });
                transcript.update(cx, |this, cx| this.sync(cx));
            };
            let assert_settled = |expected_tools, cx: &mut gpui::App| {
                let this = transcript.read(cx);
                assert_eq!(
                    this.tool_group_reveals
                        .values()
                        .map(|r| r.starts.len())
                        .sum::<usize>(),
                    expected_tools
                );
                for reveal in this.tool_group_reveals.values() {
                    assert!(
                        reveal.header_started_at.is_none(),
                        "historical header replayed"
                    );
                    assert!(
                        reveal.starts.iter().all(Option::is_none),
                        "historical tool replayed"
                    );
                }
            };
            apply(&preview, true, cx);
            assert_settled(1, cx);
            // Prepending history moves the tail tool from group 0 to group 1.
            // Its new row identity must not turn it into a live entrance.
            apply(&full, false, cx);
            assert_settled(2, cx);
            for _ in 0..3 {
                state.update(cx, |state, cx| state.select_chat(Some("away".into()), cx));
                transcript.update(cx, |this, cx| this.sync(cx));
                state.update(cx, |state, cx| state.select_chat(Some("whale".into()), cx));
                transcript.update(cx, |this, cx| this.sync(cx));
                assert_settled(2, cx);
                apply(&preview, true, cx); // ignored because the cache is complete
                assert_settled(2, cx);
                apply(&full, false, cx);
                assert_settled(2, cx);
            }
            let mut live = full.clone();
            live[0].parts.push(tool_part("new-live-tool", "git status"));
            state.update(cx, |state, cx| {
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame: openagents_doc::diff_transcript(&full, &live),
                            replay_baseline: None,
                            context_usage: None,
                        },
                        cx,
                    )
                    .unwrap();
            });
            transcript.update(cx, |this, cx| this.sync(cx));
            let this = transcript.read(cx);
            let tail = &this.tool_group_reveals[&SharedString::from("turn#g1")];
            assert!(tail.header_started_at.is_none());
            assert!(
                tail.starts[0].is_none(),
                "existing tool must remain settled"
            );
            assert!(tail.starts[1].is_some(), "new live tool must still animate");
        });
    }

    #[gpui::test]
    fn tool_group_replay_cutoff_survives_coalesced_live_updates(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            let history = vec![assistant(
                "tools",
                MessageStatus::Streaming,
                vec![tool_part("call", "pwd"), tool_part("away", "ls")],
            )];
            let mut live = history.clone();
            live[0].parts.push(tool_part("live", "git diff"));
            live.push(assistant(
                "new-live-group",
                MessageStatus::Streaming,
                vec![tool_part("new", "git status")],
            ));
            state.update(cx, |state, cx| {
                let frame = openagents_doc::diff_transcript(&state.transcript, &history);
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame,
                            context_usage: None,
                            replay_baseline: Some(openagents_doc::TranscriptBaseline::capture(
                                &history,
                            )),
                        },
                        cx,
                    )
                    .unwrap();
                // Both updates land before the transcript observes/render them.
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame: openagents_doc::diff_transcript(&history, &live),
                            context_usage: None,
                            replay_baseline: None,
                        },
                        cx,
                    )
                    .unwrap();
            });
            transcript.update(cx, |this, cx| {
                this.sync(cx);
                let reveal = &this.tool_group_reveals[&SharedString::from("tools#g0")];
                assert!(reveal.header_started_at.is_none());
                assert!(reveal.starts[..2].iter().all(Option::is_none));
                assert!(
                    reveal.starts[2].is_some(),
                    "live tail was swallowed by the replay"
                );
                let new_group = &this.tool_group_reveals[&SharedString::from("new-live-group#g0")];
                assert!(new_group.header_started_at.is_some());
                assert!(new_group.starts[0].is_some());
            });
        });
    }

    #[gpui::test]
    fn tool_group_interleaved_history_preserves_live_animation_epochs(
        cx: &mut gpui::TestAppContext,
    ) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            let apply = |entries: &[SessionMessageEntry], baseline, cx: &mut gpui::App| {
                state.update(cx, |state, cx| {
                    let frame = openagents_doc::diff_transcript(&state.transcript, entries);
                    state
                        .receive_transcript_update(
                            openagents_doc::TranscriptUpdate {
                                frame,
                                replay_baseline: baseline,
                                context_usage: None,
                            },
                            cx,
                        )
                        .unwrap();
                });
                transcript.update(cx, |this, cx| this.sync(cx));
            };
            // Two live tools are already animating when older work arrives
            // before and between them in the same group.
            let mut entries = vec![assistant(
                "mixed",
                MessageStatus::Streaming,
                vec![tool_part("live-a", "pwd"), tool_part("live-b", "pwd")],
            )];
            apply(&entries, None, cx);
            let row: SharedString = "mixed#g0".into();
            let starts = transcript.read(cx).tool_group_reveals[&row].starts.clone();
            assert!(starts.iter().all(Option::is_some));
            let header = transcript.read(cx).tool_group_reveals[&row].header_started_at;
            entries[0].parts.insert(0, tool_part("old-a", "pwd"));
            entries[0].parts.insert(2, tool_part("old-b", "pwd"));
            let mut historical = entries.clone();
            historical[0].parts.retain(|p| p.id().starts_with("old"));
            apply(
                &entries,
                Some(openagents_doc::TranscriptBaseline::capture(&historical)),
                cx,
            );
            let reveal = &transcript.read(cx).tool_group_reveals[&row];
            assert_eq!(reveal.starts, vec![None, starts[0], None, starts[1]]);
            assert_eq!(reveal.header_started_at, header);
            // A further mixed frame must animate only its new live activity.
            entries[0].parts.push(tool_part("old-c", "pwd"));
            historical[0].parts.push(tool_part("old-c", "pwd"));
            entries[0].parts.push(tool_part("live-c", "pwd"));
            apply(
                &entries,
                Some(openagents_doc::TranscriptBaseline::capture(&historical)),
                cx,
            );
            let reveal = &transcript.read(cx).tool_group_reveals[&row];
            assert_eq!(reveal.starts[..5], [None, starts[0], None, starts[1], None]);
            assert!(reveal.starts[5].is_some());
        });
    }

    #[gpui::test]
    fn tool_group_first_live_arrival_after_empty_replay_animates(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            state.update(cx, |state, cx| {
                state.select_chat(Some("new-chat".into()), cx);
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame: openagents_doc::TranscriptFrame::reset(&[]),
                            context_usage: None,
                            replay_baseline: Some(Default::default()),
                        },
                        cx,
                    )
                    .unwrap();
            });
            transcript.update(cx, |this, cx| this.sync(cx));
            let live = vec![assistant(
                "first",
                MessageStatus::Streaming,
                vec![tool_part("first-call", "pwd")],
            )];
            state.update(cx, |state, cx| {
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame: openagents_doc::diff_transcript(&[], &live),
                            context_usage: None,
                            replay_baseline: None,
                        },
                        cx,
                    )
                    .unwrap();
            });
            transcript.update(cx, |this, cx| {
                this.sync(cx);
                let reveal = &this.tool_group_reveals[&SharedString::from("first#g0")];
                assert!(reveal.header_started_at.is_some());
                assert!(reveal.starts[0].is_some());
            });
        });
    }

    #[gpui::test]
    fn tool_group_live_reset_does_not_become_history(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            let mut live = state.read(cx).transcript.clone();
            for ix in 0..5 {
                live.push(assistant(
                    &format!("live-{ix}"),
                    MessageStatus::Streaming,
                    vec![tool_part("call", "pwd")],
                ));
            }
            state.update(cx, |state, cx| {
                let frame = openagents_doc::diff_transcript(&state.transcript, &live);
                assert!(matches!(
                    &frame,
                    openagents_doc::TranscriptFrame::Reset { .. }
                ));
                state
                    .receive_transcript_update(
                        openagents_doc::TranscriptUpdate {
                            frame,
                            context_usage: None,
                            replay_baseline: None,
                        },
                        cx,
                    )
                    .unwrap();
            });
            transcript.update(cx, |this, cx| {
                this.sync(cx);
                for ix in 0..5 {
                    let reveal =
                        &this.tool_group_reveals[&SharedString::from(format!("live-{ix}#g0"))];
                    assert!(reveal.header_started_at.is_some());
                    assert!(reveal.starts[0].is_some());
                }
            });
        });
    }

    #[gpui::test]
    fn tool_group_navigation_keeps_user_pins_and_new_arrivals(cx: &mut gpui::TestAppContext) {
        with_tool_group_navigation(cx, |state, transcript, cx| {
            transcript.update(cx, |this, _| {
                this.folds.insert(
                    "tools#g0".into(),
                    FoldState {
                        open: Some(true),
                        ..Default::default()
                    },
                );
            });
            state.update(cx, |state, cx| state.select_chat(None, cx));
            transcript.update(cx, |this, cx| this.sync(cx));
            // Cached replay can land without an intervening pending frame.
            replay_tool_group(&state, &transcript, "chat-a", cx);
            transcript.update(cx, |this, cx| {
                let row = this.rows[0].clone();
                let RowKind::ToolGroup {
                    tools,
                    auto_open,
                    summary,
                    ..
                } = &row.kind
                else {
                    panic!("expected tools")
                };
                let _ = this.render_tool_group(
                    &row.id,
                    tools,
                    summary,
                    *auto_open,
                    None,
                    false,
                    &Theme::dark(),
                    cx,
                );
                assert_eq!(this.folds[&row.id].open, Some(true));
                assert_eq!(this.tool_group_reveals[&row.id].rendered_open, Some(true));
                assert!(
                    this.tool_group_reveals[&row.id]
                        .starts
                        .iter()
                        .all(Option::is_none)
                );
            });
            state.update(cx, |state, _| {
                state.transcript.push(assistant(
                    "live-tools",
                    MessageStatus::Streaming,
                    vec![tool_part("new-call", "ls")],
                ));
                state.transcript_revision += 1;
            });
            transcript.update(cx, |this, cx| {
                this.sync(cx);
                let row = this.rows.last().unwrap().clone();
                let RowKind::ToolGroup {
                    tools,
                    auto_open,
                    summary,
                    ..
                } = &row.kind
                else {
                    panic!("expected tools")
                };
                assert!(*auto_open);
                let _ = this.render_tool_group(
                    &row.id,
                    tools,
                    summary,
                    *auto_open,
                    None,
                    false,
                    &Theme::dark(),
                    cx,
                );
                let reveal = &this.tool_group_reveals[&row.id];
                assert!(reveal.header_started_at.is_some());
                assert!(reveal.starts.iter().all(Option::is_some));
                assert_eq!(reveal.rendered_open, Some(true));
            });
        });
    }

    #[test]
    fn resizing_details_does_not_restart_group_disclosure() {
        let now = Instant::now();
        let fold = FoldState {
            toggled_at: Some(now),
            ..Default::default()
        };
        assert_eq!(tool_disclosure_progress(true, fold, now), 1.0);
        assert_eq!(tool_disclosure_progress(false, fold, now), 0.0);
    }

    #[test]
    fn connector_intersection_is_tessellated_only_once() {
        let mut path = PathBuilder::fill().with_style(gpui::PathStyle::Fill(
            gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
        ));
        activity_ribbon(
            &mut path,
            &[point(px(0.0), px(0.0)), point(px(0.0), px(10.0))],
        );
        activity_ribbon(
            &mut path,
            &[point(px(-5.0), px(5.0)), point(px(5.0), px(5.0))],
        );
        let path = path.build().unwrap();
        let area: f32 = path
            .vertices
            .chunks_exact(3)
            .map(|triangle| {
                let a = triangle[0].xy_position;
                let b = triangle[1].xy_position;
                let c = triangle[2].xy_position;
                (f32::from(b.x - a.x) * f32::from(c.y - a.y)
                    - f32::from(b.y - a.y) * f32::from(c.x - a.x))
                .abs()
                    / 2.0
            })
            .sum();
        assert!(
            (area - 19.0).abs() < 0.001,
            "overlap must contribute once: {area}"
        );
    }

    #[test]
    fn file_badge_icon_well_counter_shades_each_appearance() {
        for (mut theme, base) in [
            (Theme::dark(), crate::theme::grey(48)),
            (Theme::light(), crate::theme::grey(230)),
        ] {
            theme.surface_treatment = openagents_theme::SurfaceTreatment::Opaque;
            let badge = crate::theme::flatten(theme.ink(0.06), base);
            let icon_well = crate::theme::flatten(crate::file_icons::well_bg(&theme), badge);
            let contrast = crate::theme::contrast_ratio(icon_well, badge);

            match theme.appearance {
                crate::theme::Appearance::Dark => assert!(icon_well.l < badge.l),
                crate::theme::Appearance::Light => assert!(icon_well.l > badge.l),
            }
            assert!(contrast > 1.04, "icon well must remain visible: {contrast}");
            assert!(
                contrast < 1.20,
                "icon well must not become a harsh split surface: {contrast}"
            );
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn file_badge_icon_well_uses_more_coverage_on_frost() {
        for mut theme in [Theme::dark(), Theme::light()] {
            theme.surface_treatment = openagents_theme::SurfaceTreatment::Opaque;
            let opaque_alpha = crate::file_icons::well_bg(&theme).a;
            theme.surface_treatment = openagents_theme::SurfaceTreatment::Frosted;
            let frosted = crate::file_icons::well_bg(&theme);
            let badge = crate::theme::flatten(theme.ink(0.06), theme.bg);
            let icon_well = crate::theme::flatten(frosted, badge);
            let contrast = crate::theme::contrast_ratio(icon_well, badge);

            assert!(opaque_alpha > 0.0 && opaque_alpha < 1.0);
            assert!(frosted.a > opaque_alpha && frosted.a < 1.0);
            assert!(
                contrast > 1.03,
                "frosted icon well must remain visible: {contrast}"
            );
            assert!(
                contrast < 1.20,
                "frosted icon well must not become a harsh split surface: {contrast}"
            );
        }
    }

    #[test]
    fn selection_scroll_ramps_at_viewport_edges() {
        let bounds = Bounds::new(
            gpui::point(px(10.0), px(20.0)),
            gpui::size(px(300.0), px(200.0)),
        );
        assert_eq!(
            selection_scroll_step(bounds, gpui::point(px(20.0), px(120.0))),
            0.0
        );
        assert!(selection_scroll_step(bounds, gpui::point(px(20.0), px(20.0))) < 0.0);
        assert!(selection_scroll_step(bounds, gpui::point(px(20.0), px(220.0))) > 0.0);
        assert!(
            selection_scroll_step(bounds, gpui::point(px(20.0), px(220.0)))
                > selection_scroll_step(bounds, gpui::point(px(20.0), px(200.0)))
        );
    }

    // ---- streaming parse wiring (the transcript side, not the parser) ----

    #[test]
    fn live_row_parse_work_is_bounded_per_commit() {
        // Drive the EXACT wiring `rows_for` uses (`parse_for_row`) with the
        // prefix-extending commit snapshots the doc watch delivers, and prove
        // the per-commit parse work stays O(reparsed tail): a full-reparse
        // wiring would feed ~N/2 × final_len bytes through the parser across N
        // commits; the incremental path stays within a small multiple of the
        // final length regardless of N.
        let mut live_parsers = HashMap::new();
        let mut tree_cache = HashMap::new();
        let paragraph = "A paragraph of streaming prose that keeps arriving.\n\n";
        let commits = 120usize;
        let mut text = String::new();
        let mut total_parsed = 0usize;
        for i in 0..commits {
            // Each commit appends ~half a paragraph (crosses block boundaries).
            let chunk = &paragraph[..paragraph.len() / 2];
            text.push_str(if i % 2 == 0 {
                chunk
            } else {
                &paragraph[paragraph.len() / 2..]
            });
            let (tree, outcome) =
                parse_for_row(true, "e1#p1", &text, &mut live_parsers, &mut tree_cache);
            assert!(!tree.blocks.is_empty());
            let ParseOutcome::Incremental {
                parsed_bytes,
                stable_prefix_blocks,
            } = outcome
            else {
                panic!("streaming commit must take the incremental path");
            };
            total_parsed += parsed_bytes;
            // Per commit: never a full reparse once the doc has grown past the
            // tail window (last two complete blocks + the partial trailing
            // one + the delta ≤ 3 paragraphs here).
            assert!(
                parsed_bytes <= 3 * paragraph.len(),
                "commit {i}: parsed {parsed_bytes} bytes — not bounded by the tail window"
            );
            // The stable prefix grows with the doc — settled blocks are never
            // re-touched (this is what keeps render caches valid).
            assert!(stable_prefix_blocks + 2 >= tree.blocks.len().saturating_sub(1));
        }
        // Across the whole stream: work is commits × O(tail), an order of
        // magnitude under the ~commits × len/2 a full-reparse wiring costs.
        let final_len = text.len();
        let full_reparse_cost = commits * final_len / 2;
        assert!(total_parsed <= commits * 3 * paragraph.len());
        assert!(
            total_parsed * 10 < full_reparse_cost,
            "total parsed {total_parsed} vs full-reparse ~{full_reparse_cost}"
        );

        // Live→complete handoff: the completed part adopts the live parser's
        // exact tree without parsing a single byte.
        let (_, outcome) = parse_for_row(false, "e1#p1", &text, &mut live_parsers, &mut tree_cache);
        assert_eq!(outcome, ParseOutcome::Handoff);
        // And the settled cache serves repeats with no work at all.
        let (_, outcome) = parse_for_row(false, "e1#p1", &text, &mut live_parsers, &mut tree_cache);
        assert_eq!(outcome, ParseOutcome::Cached);
    }

    // ---- stick-to-bottom spring ----

    #[test]
    fn stationary_spring_does_not_keep_requesting_frames() {
        let mut spring = StickSpring::new();
        let mut pos = 600.0;
        for _ in 0..120 {
            let next = spring.step(pos, 600.0, 1.0);
            assert_eq!(next, pos);
            assert!(!StickSpring::needs_frame(600.0 - next));
            pos = next;
        }
        // Real growth must still wake and complete the same smooth glide.
        let target = 900.0;
        let mut moving_frames = 0;
        while StickSpring::needs_frame(target - pos) && moving_frames < 600 {
            let next = spring.step(pos, target, 1.0);
            assert!(next >= pos && next <= target);
            pos = next;
            moving_frames += 1;
        }
        assert_eq!(pos, target);
        assert!(moving_frames > 1 && moving_frames < 600);
        assert!(!StickSpring::needs_frame(0.0));
    }

    #[test]
    fn estimated_height_growth_at_the_bottom_cannot_keep_spring_awake() {
        let mut spring = StickSpring::new();
        // Virtualized height estimates can grow while the viewport remains
        // anchored to exactly the same final row. This previously kept the
        // feed-forward velocity and the redraw loop alive after completion.
        for frame in 0..120 {
            let target = 10000.0 + frame as f32 * 400.0;
            let next = spring.step(target, target, 1.0);
            assert_eq!(next, target);
            assert!(!StickSpring::needs_frame(target - next));
        }
        assert!(spring.target_vel() > 1.0, "exercise a nonzero estimate");
    }

    #[test]
    fn spring_converges_to_a_fixed_target() {
        let mut spring = StickSpring::new();
        let target = 400.0;
        let mut pos = 0.0;
        let mut frames = 0;
        while pos < target && frames < 600 {
            pos = spring.step(pos, target, 1.0);
            frames += 1;
        }
        assert_eq!(pos, target, "spring must land exactly on the target");
        assert!(
            frames < 300,
            "400px should converge within 5s of frames, took {frames}"
        );
        // Once landed it stays landed (and idles out).
        for _ in 0..120 {
            pos = spring.step(pos, target, 1.0);
            assert_eq!(pos, target);
        }
        assert!(spring.is_idle(), "no residual motion at rest");
    }

    #[test]
    fn spring_never_overshoots_or_oscillates() {
        let mut spring = StickSpring::new();
        let target = 250.0;
        let mut pos = 0.0;
        let mut last = pos;
        for _ in 0..600 {
            pos = spring.step(pos, target, 1.0);
            assert!(pos <= target, "overshoot: {pos} > {target}");
            assert!(
                pos >= last - 1e-3,
                "oscillation: position moved backwards {last} -> {pos}"
            );
            last = pos;
        }
        assert_eq!(pos, target);
    }

    #[test]
    fn spring_feed_forward_tracks_constant_growth() {
        // Target grows 2px/frame (≈120px/s — a typical stream). After warmup
        // the EMA feed-forward must carry the viewport at the same rate with a
        // bounded, stable lag — a glide, not 0,0,0,Npx steps.
        let growth = 2.0;
        let mut spring = StickSpring::new();
        let mut target = 600.0;
        let mut pos = 600.0;
        let mut deltas: Vec<f32> = Vec::new();
        for frame in 0..400 {
            target += growth;
            let next = spring.step(pos, target, 1.0);
            if frame >= 200 {
                deltas.push(next - pos);
            }
            pos = next;
        }
        // Steady state: per-frame movement ≈ growth rate…
        let mean = deltas.iter().sum::<f32>() / deltas.len() as f32;
        assert!(
            (mean - growth).abs() < 0.2,
            "steady-state speed {mean} should track growth {growth}"
        );
        // …with no stepping (every frame moves, none jumps).
        for d in &deltas {
            assert!(*d > 0.0, "viewport stalled mid-stream");
            assert!(*d < growth * 3.0, "viewport jumped: {d}px in one frame");
        }
        // The EMA growth estimate itself has locked on.
        assert!((spring.target_vel() - growth).abs() < 0.3);
        // Lag stays bounded by the chase lead.
        assert!(target - pos <= SPRING_CHASE_MAX_LEAD + growth);
    }

    #[test]
    fn spring_feed_forward_resets_when_target_shrinks() {
        let mut spring = StickSpring::new();
        let mut pos = 0.0;
        for i in 1..=50 {
            pos = spring.step(pos, 100.0 + i as f32 * 4.0, 1.0);
        }
        assert!(spring.target_vel() > 1.0);
        // A collapse (target shrinks by more than 1px) drops the estimate.
        spring.step(pos.min(120.0), 120.0, 1.0);
        assert_eq!(spring.target_vel(), 0.0);
    }

    #[test]
    fn spring_catchup_frames_glide_instead_of_teleporting() {
        // A 5-frame hitch advances roughly as far as 5 single steps would —
        // sub-stepped, still clamped at the target.
        let target = 300.0;
        let mut a = StickSpring::new();
        let mut pos_a = 0.0;
        for _ in 0..5 {
            pos_a = a.step(pos_a, target, 1.0);
        }
        let mut b = StickSpring::new();
        let pos_b = b.step(0.0, target, 5.0);
        assert!((pos_a - pos_b).abs() < 1.0, "{pos_a} vs {pos_b}");
        assert!(pos_b <= target);
    }

    #[test]
    fn restick_is_direction_aware() {
        // Scrolling away from the bottom never resticks, even inside the band
        // (a 20px wheel notch from the pinned bottom must break the pin).
        assert!(!Transcript::should_restick(20.0, 0.0));
        assert!(!Transcript::should_restick(69.0, 30.0));
        // Returning toward the bottom resticks once inside the 70px band…
        assert!(Transcript::should_restick(69.0, 120.0));
        assert!(Transcript::should_restick(0.0, 30.0));
        // …but not while still outside it.
        assert!(!Transcript::should_restick(200.0, 300.0));
        // No movement — leave the pin alone.
        assert!(!Transcript::should_restick(50.0, 50.0));
    }

    #[test]
    fn only_a_stream_at_the_bottom_gets_a_hard_end_anchor() {
        assert!(should_anchor_live_stream(true, 0.0, true));
        assert!(should_anchor_live_stream(true, AT_BOTTOM_PX, true));

        // A user who has moved away from the end keeps control of the
        // viewport, even if the transcript is still streaming.
        assert!(!should_anchor_live_stream(true, AT_BOTTOM_PX + 0.1, true));
        assert!(!should_anchor_live_stream(false, 0.0, true));

        // Ordinary transcript updates retain the existing spring behavior.
        assert!(!should_anchor_live_stream(true, 0.0, false));
    }

    fn viewport_row(id: &str, entry_id: &str) -> Row {
        Row {
            id: id.into(),
            version: 0,
            turn_start: true,
            kind: RowKind::ErrorChip {
                message: SharedString::default(),
            },
            entry_id: entry_id.into(),
            timestamp: None,
            copy_text: None,
            compact_fold: None,
        }
    }

    #[test]
    fn viewport_anchor_tracks_a_stable_row_across_replay() {
        let rows = vec![
            viewport_row("a", "entry-a"),
            viewport_row("b", "entry-b"),
            viewport_row("c", "entry-c"),
        ];
        let anchor = ViewportAnchor::capture(
            &rows,
            ListOffset {
                item_ix: 1,
                offset_in_item: px(23.0),
            },
        )
        .expect("visible row");

        let replay = vec![
            viewport_row("new", "entry-new"),
            viewport_row("a", "entry-a"),
            viewport_row("b", "entry-b"),
            viewport_row("c", "entry-c"),
        ];
        let restored = anchor.resolve(&replay).expect("restored row");
        assert_eq!(restored.item_ix, 2);
        assert_eq!(restored.offset_in_item, px(23.0));
    }

    #[test]
    fn viewport_anchor_has_entry_and_index_fallbacks() {
        let rows = vec![
            viewport_row("a", "entry-a"),
            viewport_row("b", "entry-b"),
            viewport_row("old-block", "entry-c"),
        ];
        let anchor = ViewportAnchor::capture(
            &rows,
            ListOffset {
                item_ix: 2,
                offset_in_item: px(31.0),
            },
        )
        .expect("visible row");

        let reshaped = vec![
            viewport_row("a", "entry-a"),
            viewport_row("b", "entry-b"),
            viewport_row("inserted", "entry-new"),
            viewport_row("new-block", "entry-c"),
        ];
        let same_entry = anchor.resolve(&reshaped).expect("entry fallback");
        assert_eq!(same_entry.item_ix, 3);
        assert_eq!(same_entry.offset_in_item, px(0.0));

        let entry_removed = vec![viewport_row("a", "entry-a"), viewport_row("b", "entry-b")];
        let clamped = anchor.resolve(&entry_removed).expect("index fallback");
        assert_eq!(clamped.item_ix, 1);
        assert_eq!(clamped.offset_in_item, px(0.0));
    }

    #[test]
    fn optimistic_echo_cannot_consume_a_historical_viewport_before_replay() {
        let history = vec![viewport_row("historical", "historical-entry")];
        let saved = SavedViewport::capture(&history, ListOffset::default(), false, 480.0, None)
            .expect("historical viewport");
        let echo_only = vec![viewport_row("echo", "echo-entry")];

        assert!(
            saved.resolve(&echo_only, false).is_none(),
            "an unrelated echo is not an authoritative index fallback"
        );
        assert_eq!(
            saved
                .resolve(&echo_only, true)
                .expect("populated replay may use an index fallback")
                .offset
                .item_ix,
            0
        );
        assert!(TranscriptReplayState::Empty.authoritative_empty());
        assert!(!TranscriptReplayState::Empty.allows_fallback());
        assert!(!TranscriptReplayState::Pending.allows_fallback());
        assert!(TranscriptReplayState::Populated.allows_fallback());

        let echo_viewport =
            SavedViewport::capture(&echo_only, ListOffset::default(), false, 0.0, None)
                .expect("echo viewport");
        assert!(
            echo_viewport.resolve(&echo_only, false).is_some(),
            "the exact optimistic row is safe before replay"
        );
    }

    #[test]
    fn saved_viewport_preserves_and_releases_an_active_turn_runway() {
        let rows = vec![viewport_row("prompt", "prompt")];
        let own_turn = OwnTurnAnchor {
            chat_id: "chat-a".into(),
            message_id: "prompt".into(),
            held: true,
            positioned: true,
            seen_prompt: true,
        };
        let saved = SavedViewport::capture(
            &rows,
            ListOffset {
                item_ix: 0,
                offset_in_item: px(0.0),
            },
            false,
            0.0,
            Some(&own_turn),
        )
        .expect("active chat viewport");
        let SavedViewport::Anchored {
            own_turn: Some(saved_turn),
            ..
        } = &saved
        else {
            panic!("an active turn must keep its runway with the viewport");
        };
        assert!(saved_turn.held);
        assert!(saved_turn.positioned);

        let restored = saved
            .resolve(&rows, false)
            .expect("exact queued echo survives an empty replay");
        let restored_turn = restored.own_turn.expect("valid restored runway");
        assert!(!restored_turn.held);
        assert!(!restored_turn.positioned);
        assert!(restored_turn.seen_prompt);

        let list_state = ListState::new(rows.len(), ListAlignment::Bottom, px(0.0));
        list_state.reset(0);
        list_state.splice(0..0, rows.len());
        list_state.scroll_to(restored.offset);
        assert_eq!(list_state.logical_scroll_top().item_ix, 0);
        assert_eq!(list_state.logical_scroll_top().offset_in_item, px(0.0));

        assert!(
            SavedViewport::capture(&[], ListOffset::default(), false, 0.0, Some(&own_turn))
                .is_none(),
            "an empty rapid-switch replay must not overwrite the older snapshot"
        );
    }

    #[test]
    fn own_turn_waits_for_its_first_echo_then_retires_if_it_disappears() {
        let mut turn = OwnTurnAnchor {
            chat_id: "chat-a".into(),
            message_id: "prompt".into(),
            held: true,
            positioned: false,
            seen_prompt: false,
        };

        assert!(turn.observe_prompt(false), "fresh send waits one state gap");
        assert!(turn.observe_prompt(true), "echo activates the runway");
        assert!(turn.seen_prompt);
        assert!(
            !turn.observe_prompt(false),
            "failed echo retires the activated runway"
        );
    }

    #[test]
    fn queued_turn_waits_for_its_bubble_without_replacing_the_live_turn() {
        let live_rows = vec![viewport_row("live", "live-prompt")];
        let mut pending = PendingQueuedTurns::default();
        pending.register("chat-a".into(), "queued-prompt".into());

        assert_eq!(
            pending.take_latest_materialized("chat-a", &live_rows),
            None,
            "queue-panel insertion alone must not claim a transcript anchor"
        );
        assert_eq!(pending.len(), 1, "the queued id remains armed");

        let materialized = vec![
            viewport_row("live", "live-prompt"),
            viewport_row("queued", "queued-prompt"),
        ];
        assert_eq!(
            pending.take_latest_materialized("chat-a", &materialized),
            Some("queued-prompt".into()),
            "the stable id promotes only when its real bubble appears"
        );
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn newest_materialized_queue_turn_owns_a_batched_transcript_frame() {
        let mut pending = PendingQueuedTurns::default();
        pending.register("chat-a".into(), "queued-a".into());
        pending.register("chat-b".into(), "other-chat".into());
        pending.register("chat-a".into(), "queued-b".into());
        let rows = vec![viewport_row("a", "queued-a"), viewport_row("b", "queued-b")];

        assert_eq!(
            pending.take_latest_materialized("chat-a", &rows),
            Some("queued-b".into())
        );
        assert_eq!(pending.len(), 1, "another chat's candidate is preserved");
    }

    #[test]
    fn restored_viewport_discards_a_failed_optimistic_turn() {
        let outgoing = vec![viewport_row("prompt", "prompt")];
        let own_turn = OwnTurnAnchor {
            chat_id: "chat-a".into(),
            message_id: "prompt".into(),
            held: true,
            positioned: true,
            seen_prompt: true,
        };
        let saved = SavedViewport::capture(
            &outgoing,
            ListOffset::default(),
            false,
            420.0,
            Some(&own_turn),
        )
        .expect("outgoing viewport");

        // The failed echo vanished while A was hidden. The ordinary viewport
        // still restores by index, but no stale runway may intercept jump.
        let replay = vec![viewport_row("older", "older")];
        let restored = saved.resolve(&replay, true).expect("index fallback");
        assert!(restored.own_turn.is_none());
        assert_eq!(restored.offset.item_ix, 0);
        assert_eq!(restored.distance_from_bottom, 420.0);
    }

    #[test]
    fn pinned_viewports_follow_tail_and_the_cache_is_bounded() {
        let rows = vec![viewport_row("row", "entry")];
        let pinned = SavedViewport::capture(&rows, ListOffset::default(), true, 999.0, None)
            .expect("pinned viewport");
        assert!(matches!(pinned, SavedViewport::FollowTail));

        let mut cache = SavedViewportCache::default();
        for ix in 0..MAX_SAVED_VIEWPORTS + 8 {
            cache.insert(format!("chat-{ix}"), SavedViewport::FollowTail);
        }
        assert_eq!(cache.len(), MAX_SAVED_VIEWPORTS);
        assert!(cache.get_cloned_and_touch("chat-0").is_none());
        assert!(
            cache
                .get_cloned_and_touch(&format!("chat-{}", MAX_SAVED_VIEWPORTS + 7))
                .is_some()
        );
    }

    #[test]
    fn reopening_the_oldest_cached_chat_protects_it_from_the_next_eviction() {
        let mut cache = SavedViewportCache::default();
        for ix in 0..MAX_SAVED_VIEWPORTS {
            cache.insert(format!("chat-{ix}"), SavedViewport::FollowTail);
        }

        assert!(cache.get_cloned_and_touch("chat-0").is_some());
        cache.insert("outgoing-new".into(), SavedViewport::FollowTail);

        assert!(cache.by_chat.contains_key("chat-0"));
        assert!(!cache.by_chat.contains_key("chat-1"));
        assert!(cache.by_chat.contains_key("outgoing-new"));
    }

    #[test]
    fn viewport_finalization_waits_for_current_generation_and_stable_layout() {
        let token = ViewportFinalizeToken {
            generation: 7,
            layout_revision: 11,
        };
        assert!(token.still_current(7));
        assert!(!token.still_current(8));
        assert!(token.layout_settled(11));
        assert!(!token.layout_settled(12));
    }

    fn parse(_: &str, text: &str) -> Arc<BlockTree> {
        Arc::new(parse_full(text))
    }

    fn assistant(id: &str, status: MessageStatus, parts: Vec<MessagePart>) -> SessionMessageEntry {
        SessionMessageEntry {
            id: id.into(),
            role: MessageRole::Assistant,
            parts,
            created_at: 0,
            device_id: "dev".into(),
            status: Some(status),
            continuation_of: None,
            duration_ms: None,
        }
    }

    fn text_part(id: &str, text: &str) -> MessagePart {
        MessagePart::Text {
            id: id.into(),
            text: text.into(),
        }
    }

    fn reasoning_part(id: &str, text: &str) -> MessagePart {
        MessagePart::Reasoning {
            id: id.into(),
            text: text.into(),
        }
    }

    #[test]
    fn generated_image_owner_candidates_and_same_length_corrections() {
        assert_eq!(
            generated_image_devices("owner", &["host".into(), "local".into(), "owner".into()]),
            vec!["owner", "host", "local"]
        );
        assert_eq!(
            generated_image_devices("", &["host".into(), "host".into(), "".into()]),
            vec!["host"]
        );
        let entries: Vec<SessionMessageEntry> =
            serde_json::from_str(include_str!("../tests/fixtures/generated-images.json")).unwrap();
        let entry = &entries[0];
        let original = entry_fingerprint(entry, false);
        let row_version = rows_for_entry(entry, false, false, &mut parse)[0].version;
        for field in 0..4 {
            let mut changed = entry.clone();
            if field == 0 {
                changed.device_id = "another-owner".into();
            } else if let MessagePart::Image {
                path,
                name,
                mime_type,
                ..
            } = &mut changed.parts[0]
            {
                match field {
                    1 => *path = path.replace("loaded", "edited"),
                    2 => *name = "corrected.png".into(),
                    _ => *mime_type = "image/gif".into(),
                }
            }
            assert_ne!(entry_fingerprint(&changed, false), original);
            assert_ne!(
                rows_for_entry(&changed, false, false, &mut parse)[0].version,
                row_version
            );
        }
        for entry in entries {
            let rows = rows_for_entry(&entry, false, false, &mut parse);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].turn_start);
            assert_eq!(
                rows[0].timestamp.is_some(),
                entry.status == Some(MessageStatus::Complete)
            );
            assert!(rows[0].copy_text.is_none());
        }
    }

    #[gpui::test]
    fn expanded_tool_call_text_drag_selects(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
        });
        let command = "cat /workspace/src/main.rs";
        struct Detail;
        impl Render for Detail {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let theme = Theme::of(cx).clone();
                div()
                    .w(px(600.0))
                    .child(render::selection_frame_reset_for(7))
                    .child(detail_body::<DesktopMedia>(
                        &ToolDetail::Output {
                            lines: vec!["cat /workspace/src/main.rs".into()],
                            truncated_by: 0,
                        },
                        None,
                        "probe",
                        &theme,
                    ))
            }
        }
        let (_, cx) = cx.add_window_view(|_, _| Detail);
        cx.run_until_parked();
        let bounds = render::selection_test_bounds("probe-0");
        let start = bounds.origin + gpui::point(px(1.0), bounds.size.height / 2.0);
        let end = start + gpui::point(bounds.size.width + px(40.0), px(0.0));
        cx.simulate_mouse_down(start, MouseButton::Left, gpui::Modifiers::default());
        cx.simulate_mouse_move(end, Some(MouseButton::Left), gpui::Modifiers::default());
        cx.simulate_mouse_up(end, MouseButton::Left, gpui::Modifiers::default());
        assert_eq!(
            crate::markdown::selection::selected_text().as_deref(),
            Some(command)
        );
        crate::markdown::selection::clear_if_owner("probe-0");
    }

    #[gpui::test]
    fn generated_image_click_opens_lightbox_and_escape_restores_focus(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
        });
        let state = cx.new(|_| AppState::new());
        let transcript =
            cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(64, 48)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        crate::attachments::store_loaded_for(
            &crate::attachments::AttachmentKey::new(
                "preview-owner",
                "/fixture/preview.png",
                Some("image/png"),
            ),
            "generated.png".into(),
            Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Png,
                bytes.into_inner(),
            )),
        );
        struct PreviewFixture(Entity<Transcript>);
        impl Render for PreviewFixture {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                if self.0.read(cx).media.attachment_preview.is_some() {
                    self.0.clone().into_any_element()
                } else {
                    self.0.update(cx, |this, cx| {
                        DesktopMedia::render_generated_image(
                            this,
                            &"preview".into(),
                            "preview-owner",
                            "/fixture/preview.png",
                            "generated.png",
                            "image/png",
                            cx,
                        )
                    })
                }
            }
        }
        let (fixture, cx) = cx.add_window_view(|_, _| PreviewFixture(transcript.clone()));
        cx.run_until_parked();
        cx.simulate_click(gpui::point(px(20.0), px(20.0)), gpui::Modifiers::default());
        let return_focus = transcript.read_with(cx, |this, _| {
            assert!(this.media.attachment_preview.is_some());
            this.media
                .attachment_preview_return_focus
                .clone()
                .expect("preview remembers the image button focus")
        });
        // Paint the production transcript, including its shared lightbox and
        // close callback, so Escape exercises actual focus restoration.
        fixture.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        transcript.read_with(cx, |this, _| {
            assert!(this.media.attachment_preview.is_none())
        });
        cx.update(|window, _| assert!(return_focus.is_focused(window)));
    }

    #[gpui::test]
    fn generated_image_fixture_uses_cache_and_retries(cx: &mut gpui::TestAppContext) {
        use crate::attachments::*;
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
            let state = cx.new(|_| AppState::new());
            let transcript =
                cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
            let entries: Vec<SessionMessageEntry> =
                serde_json::from_str(include_str!("../tests/fixtures/generated-images.json"))
                    .unwrap();
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgba8(64, 48)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            let image = Arc::new(gpui::Image::from_bytes(
                gpui::ImageFormat::Png,
                bytes.into_inner(),
            ));
            store_loaded_for(
                &AttachmentKey::new(
                    "fixture-owner",
                    "/fixture/generated-loaded.png",
                    Some("image/png"),
                ),
                "generated.png".into(),
                image,
            );
            assert!(begin_load_for(&AttachmentKey::new(
                "fixture-owner",
                "/fixture/generated-loading.png",
                Some("image/png")
            )));
            store_error_for(&AttachmentKey::new(
                "fixture-owner",
                "/fixture/generated-unavailable.png",
                Some("image/png"),
            ));
            transcript.update(cx, |this, cx| {
                this.rows = entries
                    .iter()
                    .flat_map(|entry| rows_for_entry(entry, false, false, &mut parse))
                    .collect();
                for (ix, expected) in ["loaded", "loading", "error"].into_iter().enumerate() {
                    let RowKind::GeneratedImage {
                        owner,
                        path,
                        mime_type,
                        ..
                    } = &this.rows[ix].kind
                    else {
                        panic!("image row")
                    };
                    let (owner, path, mime_type) = (owner.clone(), path.clone(), mime_type.clone());
                    let devices = DesktopMedia::generated_attachment_device_ids(this, &owner, cx);
                    let snapshot =
                        DesktopMedia::attachment_state(this, &devices, &path, Some(&mime_type), cx);
                    match expected {
                        "loaded" => assert!(matches!(snapshot, AttachmentSnapshot::Loaded(_))),
                        "loading" => assert!(matches!(snapshot, AttachmentSnapshot::Loading)),
                        _ => assert!(matches!(snapshot, AttachmentSnapshot::Error { .. })),
                    }
                }
                assert!(
                    this.media.attachment_loads.is_empty(),
                    "cache repaint must not fetch"
                );
                assert_eq!(this.media.attachment_retries.len(), 1);
                let keys = DesktopMedia::protected_attachment_keys(this, cx);
                for entry in &entries {
                    let MessagePart::Image { path, .. } = &entry.parts[0] else {
                        unreachable!()
                    };
                    assert!(!keys.contains(&("fixture-owner".into(), path.clone())));
                }
                DesktopMedia::refresh_protected_attachments(this, cx);
                this.rows.clear();
                assert!(DesktopMedia::protected_attachment_keys(this, cx).is_empty());
                DesktopMedia::refresh_protected_attachments(this, cx);
            });
        });
    }

    #[test]
    fn generated_image_builds_one_row_outside_tools_and_copy() {
        let image = MessagePart::Image {
            id: "i:image".into(),
            path: "/uploads/i.png".into(),
            name: "generated.png".into(),
            mime_type: "image/png".into(),
        };
        let entry = assistant("a", MessageStatus::Complete, vec![image.clone()]);
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].turn_start);
        assert!(rows[0].copy_text.is_none());
        assert!(matches!(&rows[0].kind, RowKind::GeneratedImage { owner, .. } if owner == "dev"));
        let entry = assistant(
            "a",
            MessageStatus::Complete,
            vec![
                text_part("t", "before"),
                tool_part("i", "generate"),
                image,
                text_part("t2", "after"),
            ],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 4);
        assert!(matches!(rows[1].kind, RowKind::ToolGroup { .. }));
        assert!(matches!(rows[2].kind, RowKind::GeneratedImage { .. }));
    }

    #[test]
    fn reasoning_joins_the_tool_group_accordion() {
        // Thought → tool → thought → tool folds into ONE group row (user
        // request: the thought process lives inside the combined accordion),
        // and the collapsed summary names the thinking.
        let entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![
                reasoning_part("r0", "planning the first step"),
                tool_part("t1", "ls"),
                reasoning_part("r2", "now the second step"),
                tool_part("t3", "pwd"),
            ],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1, "one combined accordion row");
        let RowKind::ToolGroup { tools, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(tools.len(), 4);
        assert_eq!(tools[0].kind, ToolItemKind::Thought);
        assert_eq!(tools[2].kind, ToolItemKind::Thought);
        assert_eq!(tools[1].kind, ToolItemKind::Call);
        assert_eq!(tools[3].kind, ToolItemKind::Call);
        // Thought chips carry their text as a styled-line detail with an
        // ANALYTIC height, so the group's fold tween covers them.
        assert!(matches!(
            tools[0].detail.as_deref(),
            Some(ToolDetail::Thought { lines, .. }) if !lines.is_empty()
        ));
        let summary = tool_group_summary(&tools);
        assert!(summary.starts_with("Thought 2 times"), "{summary}");
        assert!(summary.contains("2 commands"), "{summary}");

        // A lone thought is still an accordion (with the group tween), named
        // plainly.
        let entry = assistant(
            "a2",
            MessageStatus::Complete,
            vec![reasoning_part("r0", "just thinking")],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1);
        let RowKind::ToolGroup { tools, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(tool_group_summary(&tools), "Thought process");

        // Empty reasoning renders nothing.
        let entry = assistant(
            "a3",
            MessageStatus::Complete,
            vec![reasoning_part("r0", "   ")],
        );
        assert!(rows_for_entry(&entry, false, false, &mut parse).is_empty());
    }

    #[test]
    fn live_thought_streams_open_and_settles_closed() {
        let entry = assistant(
            "a1",
            MessageStatus::Streaming,
            vec![reasoning_part("r0", "thinking hard")],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::ToolGroup {
            tools, auto_open, ..
        } = &rows[0].kind
        else {
            panic!("expected a tool group");
        };
        // The live tail auto-opens the group; the chip itself is unresolved
        // (defaults open) until the part stops being the tail.
        assert!(*auto_open);
        assert!(!tools[0].resolved);

        let entry = assistant(
            "a2",
            MessageStatus::Streaming,
            vec![
                reasoning_part("r0", "thinking hard"),
                text_part("t1", "answer"),
            ],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::ToolGroup { tools, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert!(tools[0].resolved, "a followed thought is settled");
    }

    fn compact_visible(rows: &[Row]) -> Vec<&Row> {
        rows.iter()
            .filter(|row| row.compact_fold.is_none())
            .collect()
    }

    #[test]
    fn compact_mode_folds_the_whole_turn_into_one_collapsed_group() {
        // Collapsed: work header + reply. Expanded: header + the compact-off
        // layout for work parts (thoughts/tools as groups, narration as
        // markdown) + reply.
        let entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![
                reasoning_part("r0", "thinking about it"),
                tool_part("t1", "ls"),
                text_part("n1", "checking the layout now"),
                tool_part("t2", "pwd"),
                text_part("r1", "All done — here is the answer."),
            ],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let visible = compact_visible(&rows);
        assert_eq!(visible.len(), 2, "collapsed: work header + the reply");
        let RowKind::ToolGroup {
            tools,
            auto_open,
            compact_shell,
            ..
        } = &visible[0].kind
        else {
            panic!("expected a tool group");
        };
        assert!(*compact_shell);
        assert!(!*auto_open, "the work group never opens itself");
        assert_eq!(tools.len(), 4);
        assert_eq!(tools[0].kind, ToolItemKind::Thought);
        assert_eq!(tools[1].kind, ToolItemKind::Call);
        assert_eq!(tools[2].kind, ToolItemKind::Note);
        assert_eq!(tools[3].kind, ToolItemKind::Call);
        assert!(matches!(visible[1].kind, RowKind::Markdown { .. }));
        let summary = tool_group_summary(tools);
        assert!(summary.contains("wrote a note"), "{summary}");
        assert!(summary.contains("Ran 2 commands"), "{summary}");
        assert!(summary.contains("Thought process"), "{summary}");

        assert!(rows.iter().any(|row| {
            row.compact_fold.is_some() && matches!(row.kind, RowKind::Markdown { .. })
        }));
        assert!(rows.iter().any(|row| {
            row.compact_fold.is_some()
                && matches!(
                    &row.kind,
                    RowKind::ToolGroup {
                        compact_shell: false,
                        ..
                    }
                )
        }));
        let RowKind::ToolGroup { worked_secs, .. } = &visible[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(*worked_secs, None, "no duration stamped on the fixture");
    }

    #[test]
    fn compact_mode_carries_worked_for_duration_on_the_work_group() {
        let mut entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![tool_part("t0", "ls"), text_part("r0", "done")],
        );
        entry.duration_ms = Some(310_000);
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let RowKind::ToolGroup { worked_secs, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(*worked_secs, Some(310));
        assert_eq!(worked_for_label(310), "Worked for 5m 10s");
        assert_eq!(worked_for_label(95), "Worked for 1m 35s");

        let streaming = assistant("a1", MessageStatus::Streaming, vec![tool_part("t0", "ls")]);
        let mut streaming = streaming;
        streaming.duration_ms = Some(310_000);
        let rows = rows_for_entry(&streaming, false, true, &mut parse);
        let RowKind::ToolGroup { worked_secs, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(
            *worked_secs, None,
            "live compact groups keep the working trailer, not a settled duration"
        );

        let mut zero = assistant(
            "a1",
            MessageStatus::Complete,
            vec![tool_part("t0", "ls"), text_part("r0", "done")],
        );
        zero.duration_ms = Some(0);
        let rows = rows_for_entry(&zero, false, true, &mut parse);
        let RowKind::ToolGroup { worked_secs, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(
            *worked_secs, None,
            "a zero stamp is a real finish, not a missing field to guess from"
        );

        let missing = assistant(
            "a1",
            MessageStatus::Complete,
            vec![tool_part("t0", "ls"), text_part("r0", "done")],
        );
        let rows = rows_for_entry(&missing, false, true, &mut parse);
        let RowKind::ToolGroup { worked_secs, .. } = &rows[0].kind else {
            panic!("expected a tool group");
        };
        assert_eq!(
            *worked_secs, None,
            "pre-durationMs history keeps the tool summary"
        );
    }

    #[test]
    fn compact_mode_keeps_reply_rows_and_skips_empty_groups() {
        // A reply-only turn renders exactly like the ordinary split — no
        // empty accordion.
        let entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![text_part("t0", "just an answer")],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0].kind, RowKind::Markdown { .. }));

        // Consecutive trailing texts all stay visible — only earlier
        // narration folds.
        let entry = assistant(
            "a2",
            MessageStatus::Complete,
            vec![
                tool_part("t0", "ls"),
                text_part("r0", "first half"),
                text_part("r1", "second half"),
            ],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let visible = compact_visible(&rows);
        assert_eq!(visible.len(), 3, "work header + two reply parts");
        assert!(matches!(visible[0].kind, RowKind::ToolGroup { .. }));
        assert!(matches!(visible[1].kind, RowKind::Markdown { .. }));
        assert!(matches!(visible[2].kind, RowKind::Markdown { .. }));
        assert!(
            rows.iter().any(|row| {
                row.compact_fold.is_some()
                    && matches!(
                        &row.kind,
                        RowKind::ToolGroup {
                            compact_shell: false,
                            ..
                        }
                    )
            }),
            "expanded work restores the ordinary tool group"
        );
    }

    #[test]
    fn compact_mode_stays_collapsed_while_streaming() {
        // Streaming surfaces nothing under compact mode — in-progress text is
        // narration until the turn settles, so it folds with the rest and the
        // whole turn is one closed accordion.
        let entry = assistant(
            "a1",
            MessageStatus::Streaming,
            vec![
                reasoning_part("r0", "thinking"),
                tool_part("t1", "ls"),
                text_part("r1", "streaming the answer"),
            ],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let visible = compact_visible(&rows);
        assert_eq!(visible.len(), 1, "the streaming text tail folds too");
        let RowKind::ToolGroup {
            tools, auto_open, ..
        } = &visible[0].kind
        else {
            panic!("expected a tool group");
        };
        assert!(!*auto_open);
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[2].kind, ToolItemKind::Note);
        assert!(rows.iter().any(|row| {
            row.compact_fold.is_some()
                && matches!(
                    row.kind,
                    RowKind::LiveMarkdown { .. } | RowKind::Markdown { .. }
                )
        }));

        // On settle the same parts surface the reply as its own row.
        let done = assistant("a1", MessageStatus::Complete, entry.parts.clone());
        let rows = rows_for_entry(&done, false, true, &mut parse);
        let visible = compact_visible(&rows);
        assert_eq!(visible.len(), 2);
        assert!(matches!(visible[0].kind, RowKind::ToolGroup { .. }));
        assert!(matches!(visible[1].kind, RowKind::Markdown { .. }));
    }

    #[test]
    fn compact_mode_keeps_input_and_error_chips_visible() {
        // Interactive/error rows are not work steps — they stay outside the
        // fold so a blocking question or a failure still surfaces.
        let entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![
                tool_part("t0", "ls"),
                MessagePart::Error {
                    id: "e1".into(),
                    message: "boom".into(),
                },
                tool_part("t1", "pwd"),
                text_part("r0", "the answer"),
            ],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let visible = compact_visible(&rows);
        assert_eq!(visible.len(), 3);
        assert!(matches!(visible[0].kind, RowKind::ToolGroup { .. }));
        assert!(matches!(visible[1].kind, RowKind::ErrorChip { .. }));
        assert!(matches!(visible[2].kind, RowKind::Markdown { .. }));
        assert!(visible[1].compact_fold.is_none());
        let RowKind::ToolGroup { tools, .. } = &visible[0].kind else {
            unreachable!();
        };
        assert_eq!(tools.len(), 2, "the error didn't split the work group");
    }

    #[test]
    fn compact_fold_geometry_reports_prefix_own_and_total() {
        // tool + narration + tool → three folded body rows (group, markdown,
        // group) between the shell header and the reply.
        let entry = assistant(
            "a1",
            MessageStatus::Complete,
            vec![
                tool_part("t1", "ls"),
                text_part("n1", "checking the layout"),
                tool_part("t2", "pwd"),
                text_part("r0", "the answer"),
            ],
        );
        let rows = rows_for_entry(&entry, false, true, &mut parse);
        let work_id: SharedString = "a1#work".into();
        let folded_ixs: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.compact_fold.is_some())
            .map(|(ix, _)| ix)
            .collect();
        assert_eq!(folded_ixs.len(), 3, "fixture produces three body rows");
        let mut heights = HashMap::new();
        for (ix, row_ix) in folded_ixs.iter().enumerate() {
            heights.insert(
                rows[*row_ix].id.clone(),
                Rc::new(Cell::new(100.0 * (ix + 1) as f32)),
            );
        }
        let (prefix, own, total) = compact_fold_geometry(&rows, &heights, &work_id, folded_ixs[1]);
        assert_eq!(prefix, 100.0);
        assert_eq!(own, 200.0);
        assert_eq!(total, 600.0);
        // A row tagged to a DIFFERENT fold is not part of the run.
        let mut rows = rows.clone();
        rows.push(Row {
            compact_fold: Some("other#work".into()),
            ..rows[folded_ixs[2]].clone()
        });
        let (_, _, total) = compact_fold_geometry(&rows, &heights, &work_id, folded_ixs[1]);
        assert_eq!(total, 600.0);
    }

    #[test]
    fn compact_body_height_tweens_from_seeded_height_to_target() {
        let now = Instant::now();
        let opening = FoldState {
            open: Some(true),
            from: 0.0,
            toggled_at: Some(now),
            ..Default::default()
        };
        assert_eq!(compact_body_height(opening, 300.0, false, now), 0.0);
        let mid = compact_body_height(opening, 300.0, false, now + TOOL_FOLD.total() / 2);
        assert!(mid > 150.0, "ease-out runs ahead of linear: {mid}");
        assert!(mid < 300.0);
        let landed = now + TOOL_FOLD.total() + Duration::from_millis(1);
        assert_eq!(compact_body_height(opening, 300.0, false, landed), 300.0);
        assert_eq!(
            compact_body_height(opening, 300.0, true, now),
            300.0,
            "reduced motion jumps straight to the target"
        );
        // A reversal seeds `from` with the budget in flight; the tween then
        // walks it down rather than restarting at the full height.
        let closing = FoldState {
            open: Some(false),
            from: 120.0,
            toggled_at: Some(now),
            ..Default::default()
        };
        let mid = compact_body_height(closing, 300.0, false, now + TOOL_FOLD.total() / 2);
        assert!(mid > 0.0 && mid < 120.0, "mid-close: {mid}");
        assert_eq!(compact_body_height(closing, 300.0, false, landed), 0.0);
    }

    #[gpui::test]
    fn compact_fold_keeps_body_mounted_through_the_close_tween(cx: &mut gpui::TestAppContext) {
        let mut handle = None;
        with_tool_group_navigation(cx, |state, transcript, cx| {
            state.update(cx, |state, _| {
                state.transcript = vec![assistant(
                    "a1",
                    MessageStatus::Complete,
                    vec![
                        reasoning_part("r0", "thinking"),
                        tool_part("t1", "ls"),
                        text_part("r0", "the reply"),
                    ],
                )];
                state.transcript_revision += 1;
            });
            transcript.update(cx, |this, cx| {
                this.compact_mode = true;
                this.last_source = None;
                this.sync(cx);
            });
            let work_id: SharedString = "a1#work".into();
            let folded = |this: &Transcript| {
                this.rows
                    .iter()
                    .filter(|row| row.compact_fold.is_some())
                    .count()
            };
            transcript.update(cx, |this, _| {
                assert_eq!(folded(this), 0, "collapsed fold mounts no body rows")
            });
            transcript.update(cx, |this, cx| {
                this.toggle_fold(work_id.clone(), 0.0, false, cx);
                assert_eq!(this.folds[&work_id].open, Some(true));
                assert_eq!(this.folds[&work_id].from, 0.0);
                assert!(folded(this) > 0, "opening mounts the body rows");
                // A settled-open fold carries its measured total into the
                // close tween's start.
                for row in this.rows.iter().filter(|row| row.compact_fold.is_some()) {
                    this.compact_fold_heights
                        .insert(row.id.clone(), Rc::new(Cell::new(100.0)));
                }
                let total = 100.0 * folded(this) as f32;
                this.folds.get_mut(&work_id).unwrap().toggled_at =
                    Some(Instant::now() - FOLD_TWEEN_WINDOW);
                this.toggle_fold(work_id.clone(), 0.0, false, cx);
                assert_eq!(this.folds[&work_id].open, Some(false));
                assert_eq!(
                    this.folds[&work_id].from, total,
                    "closing tweens from the body's measured height"
                );
                assert!(
                    folded(this) > 0,
                    "the body stays mounted through the close tween"
                );
                assert!(this.compact_fold_settle.is_some());
                // Reopening mid-tween continues from the budget in flight.
                this.folds.get_mut(&work_id).unwrap().toggled_at =
                    Some(Instant::now() - TOOL_FOLD.total() / 2);
                this.toggle_fold(work_id.clone(), 0.0, false, cx);
                let from = this.folds[&work_id].from;
                let expected = motion::lerp(total, 0.0, TOOL_FOLD.curve.eval(0.5));
                assert!(
                    (from - expected).abs() < 1.0,
                    "reversal resumes from the in-flight budget: {from} ≈ {expected}"
                );
                assert_eq!(this.folds[&work_id].open, Some(true));
            });
            // Close again, expire the retention window, and let the settle
            // sweep's sync drop the rows. The sweep's scheduled task needs the
            // app borrow, so it has to run after this update returns.
            transcript.update(cx, |this, cx| {
                this.toggle_fold(work_id.clone(), 0.0, false, cx);
                assert!(folded(this) > 0);
                this.folds.get_mut(&work_id).unwrap().toggled_at =
                    Some(Instant::now() - FOLD_TWEEN_WINDOW - Duration::from_millis(50));
            });
            handle = Some(transcript);
        });
        let transcript = handle.unwrap();
        cx.executor()
            .advance_clock(FOLD_TWEEN_WINDOW + Duration::from_millis(100));
        cx.run_until_parked();
        transcript.update(cx, |this, _| {
            let folded = this
                .rows
                .iter()
                .filter(|row| row.compact_fold.is_some())
                .count();
            assert_eq!(folded, 0, "the settle sweep drops closed rows")
        });
    }

    fn thought_of(text: &str) -> Vec<Vec<InlineRun>> {
        thought_lines(&parse_full(text))
    }

    fn line_chars(line: &[InlineRun]) -> usize {
        line.iter().map(|r| r.text.chars().count()).sum()
    }

    fn line_string(line: &[InlineRun]) -> String {
        line.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn codex_summary_paragraphs_render_as_separate_styled_lines() {
        let lines = thought_of("**Implementing file badges**\n\n**Preparing fixture screenshots**");
        assert_eq!(
            lines
                .iter()
                .map(|line| line_string(line))
                .collect::<Vec<_>>(),
            [
                "Implementing file badges",
                "",
                "Preparing fixture screenshots"
            ]
        );
        for ix in [0, 2] {
            assert!(
                lines[ix]
                    .iter()
                    .filter(|run| !run.text.is_empty())
                    .all(|run| run.style.bold)
            );
        }
    }

    #[test]
    fn thought_wrap_is_word_aware_and_bounded() {
        let lines = thought_of("one two three");
        assert_eq!(lines.len(), 1);
        assert_eq!(line_string(&lines[0]), "one two three");
        let long = "word ".repeat(200);
        let lines = thought_of(&long);
        assert!(lines.iter().all(|l| line_chars(l) <= THOUGHT_WRAP_COLS));
        assert!(lines.len() > 5);
        let pathological = "x".repeat(300);
        let lines = thought_of(&pathological);
        assert!(lines.iter().all(|l| line_chars(l) <= THOUGHT_WRAP_COLS));
        // A word glued across style boundaries wraps as ONE unit — no line
        // may split inside `**bold**tail`.
        let glued = format!("{} **bold**tail", "word ".repeat(30));
        let lines = thought_of(&glued);
        let joined: Vec<String> = lines.iter().map(|l| line_string(l)).collect();
        assert!(joined.iter().any(|l| l.ends_with("boldtail")), "{joined:?}");
    }

    #[test]
    fn thought_markdown_styles_instead_of_literal_markers() {
        // The exact user report: `**bold**` markers showed as glyphs.
        let lines = thought_of("**Planning rollback** then *checking* `parse` [docs](https://d)");
        assert_eq!(lines.len(), 1);
        let flat = line_string(&lines[0]);
        assert!(
            !flat.contains('*') && !flat.contains('`') && !flat.contains('['),
            "{flat}"
        );
        let line = &lines[0];
        assert!(
            line.iter()
                .any(|r| r.style.bold && r.text.contains("Planning rollback")),
            "bold run survives: {line:?}"
        );
        assert!(
            line.iter()
                .any(|r| r.style.italic && r.text.contains("checking"))
        );
        assert!(
            line.iter()
                .any(|r| r.style.code && r.text.contains("parse"))
        );
        assert!(
            line.iter()
                .any(|r| r.style.link.is_some() && r.text.contains("docs"))
        );
    }

    #[test]
    fn thought_blocks_flatten_structurally() {
        let lines = thought_of("# Head\n\npara\n\n- one\n- two\n\n```rust\nlet x = 1;\n```");
        let flat: Vec<String> = lines.iter().map(|l| line_string(l)).collect();
        // Heading renders bold, same size (one 18px row).
        assert!(
            lines[0]
                .iter()
                .any(|r| r.style.bold && r.text.contains("Head"))
        );
        // Blank separator rows between top-level blocks; tight list inside.
        assert_eq!(flat[1], "");
        assert_eq!(flat[2], "para");
        assert_eq!(flat[4], "• one");
        assert_eq!(flat[5], "• two");
        // Code lines verbatim, styled as code (mono at render).
        assert!(
            lines
                .last()
                .unwrap()
                .iter()
                .any(|r| r.style.code && r.text == "let x = 1;"),
            "{flat:?}"
        );
    }

    fn tool_part(id: &str, command: &str) -> MessagePart {
        MessagePart::Tool {
            id: id.into(),
            call: ToolCall::Exec {
                command: command.into(),
            },
            is_error: false,
            resolved: true,
            output: None,
            diff: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            diff_stats: None,
            subagent_ref: None,
            subagent_status: None,
            subagent_tail: None,
        }
    }

    const MD: &str = "# Title\n\npara one\n\n```rust\nlet x = 1;\n```";

    #[test]
    fn live_entry_splits_per_block_with_id_continuity() {
        // Live rows split per block exactly like completed ones (the list
        // virtualizes them — the fading tail is the only per-frame work).
        let live = assistant("m1", MessageStatus::Streaming, vec![text_part("t0", MD)]);
        let live_rows = rows_for_entry(&live, false, false, &mut parse);
        assert_eq!(live_rows.len(), 3, "one live row per top-level block");
        assert!(
            live_rows
                .iter()
                .all(|r| matches!(r.kind, RowKind::LiveMarkdown { .. }))
        );
        assert_eq!(live_rows[0].id.as_ref(), "m1#t0.0");
        assert_eq!(live_rows[2].id.as_ref(), "m1#t0.2");

        let done = assistant("m1", MessageStatus::Complete, vec![text_part("t0", MD)]);
        let done_rows = rows_for_entry(&done, false, false, &mut parse);
        assert_eq!(done_rows.len(), 3, "three top-level blocks");
        // Every block row keeps its id across the flip — no flicker on handoff.
        for (live, done) in live_rows.iter().zip(&done_rows) {
            assert_eq!(live.id, done.id);
            // The flip changes the version even at identical text (the
            // streaming bit), forcing a splice.
            assert_ne!(live.version, done.version);
        }
        assert!(matches!(
            done_rows[0].kind,
            RowKind::Markdown { block_ix: 0, .. }
        ));
    }

    #[test]
    fn live_commit_changes_only_tail_row_versions() {
        // Streaming commit: appending to the last block leaves every settled
        // block row's (id, version) untouched — the diff splices only the tail.
        let t1 = "para one\n\npara two\n\npara three";
        let t2 = "para one\n\npara two\n\npara three grows here";
        let live1 = assistant("m1", MessageStatus::Streaming, vec![text_part("t0", t1)]);
        let live2 = assistant("m1", MessageStatus::Streaming, vec![text_part("t0", t2)]);
        let r1 = rows_for_entry(&live1, false, false, &mut parse);
        let r2 = rows_for_entry(&live2, false, false, &mut parse);
        assert_eq!(r1.len(), 3);
        assert_eq!(r2.len(), 3);
        assert_eq!(r1[0].version, r2[0].version, "settled block untouched");
        assert_eq!(r1[1].version, r2[1].version, "settled block untouched");
        assert_ne!(r1[2].version, r2[2].version, "tail block respliced");
        assert_eq!(diff_rows(&r1, &r2), Some((2..3, 1)));
    }

    #[test]
    fn split_sibling_gaps_match_live_internal_spacing() {
        // The live row spaces its internal blocks by MD_BLOCK_GAP; after the
        // live→split handoff the same boundaries are inter-row gaps. They must
        // be identical or the whole message jumps at completion.
        let done = assistant(
            "m1",
            MessageStatus::Complete,
            vec![
                text_part("t0", MD),
                tool_part("a", "ls"),
                text_part("t1", "tail para"),
            ],
        );
        let rows = rows_for_entry(&done, false, false, &mut parse);
        // Rows: t0.0, t0.1, t0.2 (three MD blocks), g0, t1.0.
        assert_eq!(rows.len(), 5);
        // Sibling markdown blocks from the same part: md block gap.
        assert_eq!(top_gap_for(Some(&rows[0]), &rows[1]), render::MD_BLOCK_GAP);
        assert_eq!(top_gap_for(Some(&rows[1]), &rows[2]), render::MD_BLOCK_GAP);
        // Markdown → tool group and tool group → next part: larger boundary.
        assert_eq!(top_gap_for(Some(&rows[2]), &rows[3]), Theme::SPACE_MD);
        assert_eq!(top_gap_for(Some(&rows[3]), &rows[4]), Theme::SPACE_MD);
        // Turn starts get the turn gap regardless.
        assert_eq!(top_gap_for(None, &rows[0]), Theme::SPACE_LG);
    }

    #[test]
    fn consecutive_tools_fold_into_groups_between_text() {
        let entry = assistant(
            "m2",
            MessageStatus::Complete,
            vec![
                text_part("t0", "before"),
                tool_part("a", "ls"),
                tool_part("b", "pwd"),
                text_part("t1", "after"),
                tool_part("c", "make"),
            ],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_ref()).collect();
        assert_eq!(ids, ["m2#t0.0", "m2#g0", "m2#t1.0", "m2#g1"]);
        let RowKind::ToolGroup { tools, .. } = &rows[1].kind else {
            panic!("group expected")
        };
        assert_eq!(tools.len(), 2);
        assert!(rows[0].turn_start && !rows[1].turn_start);
    }

    fn agent_part(id: &str, description: &str) -> MessagePart {
        MessagePart::Tool {
            id: id.into(),
            call: ToolCall::Unknown {
                name: format!("Agent: {description}"),
                input: Some(serde_json::json!({ "description": description })),
            },
            is_error: false,
            resolved: true,
            output: None,
            diff: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            diff_stats: None,
            subagent_ref: Some(format!("chat--sub--{id}")),
            subagent_status: Some(SubagentStatus::Running),
            subagent_tail: None,
        }
    }

    #[test]
    fn agent_calls_split_out_of_ordinary_tool_groups() {
        // Agent/spawn chips must not share a collapse with Reads/Runs: a
        // lone Agent used to hide behind "Called 1 tool", and a mixed
        // group hid the running subagent until the user opened the fold.
        let entry = assistant(
            "m-agent",
            MessageStatus::Complete,
            vec![
                text_part("t0", "before"),
                tool_part("a", "ls"),
                tool_part("b", "pwd"),
                agent_part("s1", "Map URL import ingest path"),
                tool_part("c", "make"),
                agent_part("s2", "Audit the fold path"),
                agent_part("s3", "Verify the commit cadence"),
                text_part("t1", "after"),
            ],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_ref()).collect();
        assert_eq!(
            ids,
            [
                "m-agent#t0.0",
                "m-agent#g0",
                "m-agent#g1",
                "m-agent#g2",
                "m-agent#g3",
                "m-agent#t1.0",
            ]
        );

        let RowKind::ToolGroup {
            tools, auto_open, ..
        } = &rows[1].kind
        else {
            panic!("ordinary group expected")
        };
        assert_eq!(tools.len(), 2);
        assert!(tool_group_collapses(tools));
        assert!(!*auto_open);

        let RowKind::ToolGroup { tools, .. } = &rows[2].kind else {
            panic!("agent group expected")
        };
        assert_eq!(tools.len(), 1);
        assert!(!tool_group_collapses(tools));
        assert!(is_agent_tool(&tools[0]));

        let RowKind::ToolGroup { tools, .. } = &rows[3].kind else {
            panic!("ordinary group expected")
        };
        assert_eq!(tools.len(), 1);
        assert!(tool_group_collapses(tools));

        let RowKind::ToolGroup { tools, .. } = &rows[4].kind else {
            panic!("consecutive agents share a group")
        };
        assert_eq!(tools.len(), 2);
        assert!(!tool_group_collapses(tools));
        assert!(tools.iter().all(is_agent_tool));
    }

    #[test]
    fn stray_subagent_ref_on_a_run_chip_stays_an_ordinary_tool() {
        // Docs written before the claude-driver fix carry subagent refs on
        // ordinary Run chips (a background shell's task_notification was
        // mis-tagged as subagent traffic). The ref alone must not change the
        // chip's genus: it folds with its neighbors and renders as a plain
        // tool, never as a spawn link to a doc that was never created.
        let mut stray = tool_part("b", "git clone …");
        if let MessagePart::Tool {
            subagent_ref,
            subagent_status,
            ..
        } = &mut stray
        {
            *subagent_ref = Some("chat--sub--b".into());
            *subagent_status = Some(SubagentStatus::Done);
        }
        let entry = assistant(
            "m-stray",
            MessageStatus::Complete,
            vec![tool_part("a", "ls"), stray, tool_part("c", "make")],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1, "one folded group, no agent split");
        let RowKind::ToolGroup { tools, .. } = &rows[0].kind else {
            panic!("tool group expected")
        };
        assert_eq!(tools.len(), 3);
        assert!(tool_group_collapses(tools));
        assert!(tools.iter().all(|t| !is_agent_tool(t)));
        assert!(tools.iter().all(|t| !is_spawn_link(t)));
    }

    #[test]
    fn lone_completed_agent_stays_uncollapsed() {
        let entry = assistant(
            "m-lone",
            MessageStatus::Complete,
            vec![agent_part("s1", "scan repo")],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1);
        let RowKind::ToolGroup {
            tools, auto_open, ..
        } = &rows[0].kind
        else {
            panic!("agent group expected")
        };
        assert_eq!(tools.len(), 1);
        assert!(!tool_group_collapses(tools), "no 'Called 1 tool' wrap");
        assert!(
            !*auto_open,
            "auto_open is a streaming flag; agent rows ignore it at paint"
        );
    }

    #[test]
    fn pre_spawn_agent_name_is_enough_to_split() {
        // Before the engine stamps subagent_ref the chip is already named
        // "Agent: …" — that genus must split, or the spawn hides until the
        // first tagged event.
        let mut part = agent_part("s1", "scan repo");
        if let MessagePart::Tool {
            subagent_ref,
            subagent_status,
            ..
        } = &mut part
        {
            *subagent_ref = None;
            *subagent_status = None;
        }
        let entry = assistant(
            "m-pre",
            MessageStatus::Complete,
            vec![tool_part("a", "ls"), part],
        );
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 2);
        let RowKind::ToolGroup { tools, .. } = &rows[0].kind else {
            panic!()
        };
        assert!(tool_group_collapses(tools));
        let RowKind::ToolGroup { tools, .. } = &rows[1].kind else {
            panic!()
        };
        assert!(!tool_group_collapses(tools));
        assert!(is_agent_call(&tools[0].call));
    }

    #[test]
    fn trailing_group_auto_opens_only_while_streaming() {
        let parts = vec![text_part("t0", "hi"), tool_part("a", "ls")];
        let streaming = assistant("m3", MessageStatus::Streaming, parts.clone());
        let rows = rows_for_entry(&streaming, false, false, &mut parse);
        let RowKind::ToolGroup { auto_open, .. } = rows[1].kind else {
            panic!()
        };
        assert!(auto_open, "trailing group opens while streaming");

        let complete = assistant("m3", MessageStatus::Complete, parts);
        let rows = rows_for_entry(&complete, false, false, &mut parse);
        let RowKind::ToolGroup { auto_open, .. } = rows[1].kind else {
            panic!()
        };
        assert!(!auto_open);

        // A non-trailing group never auto-opens.
        let mid = assistant(
            "m4",
            MessageStatus::Streaming,
            vec![tool_part("a", "ls"), text_part("t0", "hi")],
        );
        let rows = rows_for_entry(&mid, false, false, &mut parse);
        let RowKind::ToolGroup { auto_open, .. } = rows[0].kind else {
            panic!()
        };
        assert!(!auto_open);
    }

    #[test]
    fn user_rows_and_echo_versions() {
        let mut entry = assistant("u1", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", "hello")];
        let confirmed = rows_for_entry(&entry, false, false, &mut parse);
        let echoed = rows_for_entry(&entry, true, false, &mut parse);
        assert_eq!(confirmed.len(), 1);
        assert_eq!(confirmed[0].id, echoed[0].id);
        // Pending → confirmed changes the version so the row re-renders.
        assert_ne!(confirmed[0].version, echoed[0].version);
        assert!(matches!(
            &echoed[0].kind,
            RowKind::User { pending: true, .. }
        ));
    }

    // Exercise the real Transcript handlers with GPUI's Linux headless
    // platform; no renderer, display server, or test-only dependency needed.
    #[cfg(target_os = "linux")]
    mod user_fold_scroll {
        use super::*;

        fn with_transcript(test: impl FnOnce(&mut Transcript, &mut Context<Transcript>) + 'static) {
            openagents_gpui_test::os_test_application().run(move |cx| {
                let state = cx.new(|_| AppState::new());
                let transcript = cx
                    .new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
                transcript.update(cx, test);
                // Quit after the platform loop starts; calloop resets its
                // stop flag on entry, so quitting in the launch hook hangs.
                cx.spawn(async move |cx| {
                    cx.update(|cx| cx.quit());
                })
                .detach();
            });
        }

        struct CachedTranscript(Entity<Transcript>);

        impl Render for CachedTranscript {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                self.0
                    .clone()
                    .cached(gpui::StyleRefinement::default().size_full())
            }
        }

        fn with_window(
            test: impl FnOnce(Entity<Transcript>, gpui::WindowHandle<CachedTranscript>, &mut gpui::App)
            + 'static,
        ) {
            openagents_gpui_test::os_test_application().run(move |cx| {
                cx.set_global(Theme::dark());
                let state = cx.new(|_| AppState::new());
                let window = cx
                    .open_window(
                        gpui::WindowOptions {
                            window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::new(
                                Point::default(),
                                gpui::size(px(1000.0), px(800.0)),
                            ))),
                            ..Default::default()
                        },
                        |_, cx| {
                            let transcript = cx.new(|cx| {
                                Transcript::new(crate::transcript_host::AppHost::shared(&state), cx)
                            });
                            cx.new(|_| CachedTranscript(transcript))
                        },
                    )
                    .unwrap();
                let transcript = window.entity(cx).unwrap().read(cx).0.clone();
                test(transcript, window, cx);
                cx.spawn(async move |cx| {
                    cx.update(|cx| cx.quit());
                })
                .detach();
            });
        }

        fn draw(window: gpui::WindowHandle<CachedTranscript>, cx: &mut gpui::App) {
            cx.update_window(window.into(), |_, window, cx| {
                window.refresh();
                let _ = window.draw(cx);
            })
            .unwrap();
        }

        #[test]
        fn conversation_width_reflows_streaming_text_without_restarting_animations() {
            with_window(|transcript, window, cx| {
                let dir = tempfile::tempdir().unwrap();
                crate::settings::init(Default::default(), dir.path(), cx);
                let text = "Streaming content should wrap at the configured conversation width. "
                    .repeat(80);
                let mut entries = vec![assistant(
                    "reply",
                    MessageStatus::Streaming,
                    vec![text_part("body", &text)],
                )];
                transcript.update(cx, |this, cx| {
                    feed(this, entries.clone(), cx);
                    this.rail_enabled = false;
                    this.pinned = false;
                    this.list.scroll_to(ListOffset {
                        item_ix: 0,
                        offset_in_item: px(0.0),
                    });
                });
                draw(window, cx);
                let original_veil =
                    transcript.read(cx).veils[&SharedString::from("reply#body.0")].clone();
                crate::settings::set_transcript_width(560.0, cx);
                draw(window, cx);
                let narrow = transcript
                    .read(cx)
                    .list
                    .bounds_for_item(0)
                    .unwrap()
                    .size
                    .height;
                crate::settings::set_transcript_width(1200.0, cx);
                draw(window, cx);
                let this = transcript.read(cx);
                let wide = this.list.bounds_for_item(0).unwrap().size.height;
                assert!(
                    wide < narrow,
                    "width change must remeasure wrapped rows: {wide:?} vs {narrow:?}"
                );
                assert!(Rc::ptr_eq(
                    &original_veil,
                    &this.veils[&SharedString::from("reply#body.0")]
                ));
                assert!(!this.pinned);
                assert_eq!(this.list.logical_scroll_top().item_ix, 0);
                assert!(this.list.logical_scroll_top().offset_in_item.abs() <= px(1.0));
                let bounds = render::selection_test_bounds("reply#body.0:0");
                assert!(
                    bounds.size.width <= px(904.0),
                    "content must fit a 1000px viewport with 48px gutters"
                );
                entries[0].parts.push(tool_part("live-tool", "pwd"));
                transcript.update(cx, |this, cx| {
                    feed(this, entries, cx);
                    assert!(
                        this.tool_group_reveals[&SharedString::from("reply#g0")].starts[0]
                            .is_some()
                    );
                    this.pinned = true;
                    this.list.scroll_to(ListOffset {
                        item_ix: this.list.item_count(),
                        offset_in_item: px(0.0),
                    });
                });
                crate::settings::set_transcript_width(560.0, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                assert!(transcript.read(cx).distance_from_bottom() <= 1.0);
            });
        }

        #[test]
        fn replay_keeps_unpainted_opening_text_seeded_and_new_text_live() {
            with_window(|transcript, window, cx| {
                let history = vec![assistant(
                    "reply",
                    MessageStatus::Streaming,
                    vec![text_part("body", "historial fuera de pantalla")],
                )];
                transcript.update(cx, |this, cx| {
                    this.state.update(cx, |state, cx| {
                        state.select_chat(Some("chat".into()), cx);
                        state
                            .receive_transcript_update(
                                openagents_doc::TranscriptUpdate {
                                    frame: openagents_doc::TranscriptFrame::reset(&history),
                                    context_usage: None,
                                    replay_baseline: Some(
                                        openagents_doc::TranscriptBaseline::capture(&history),
                                    ),
                                },
                                cx,
                            )
                            .unwrap();
                    });
                    this.sync(cx);
                    assert!(this.veils.is_empty(), "opening text has not been painted");
                });
                let recovered = assistant(
                    "recovered",
                    MessageStatus::Complete,
                    vec![text_part("old", "otro bloque histórico")],
                );
                let mut next = history.clone();
                next[0].parts.push(text_part("fresh", "texto en vivo"));
                next.push(recovered.clone());
                let mut cutoff = history.clone();
                cutoff.push(recovered);
                transcript.update(cx, |this, cx| {
                    this.state.update(cx, |state, cx| {
                        state
                            .receive_transcript_update(
                                openagents_doc::TranscriptUpdate {
                                    frame: openagents_doc::diff_transcript(&history, &next),
                                    context_usage: None,
                                    // The RPC must retain its opening cutoff when
                                    // publishing subsequent changed-part history.
                                    replay_baseline: Some(
                                        openagents_doc::TranscriptBaseline::capture(&cutoff),
                                    ),
                                },
                                cx,
                            )
                            .unwrap();
                    });
                    this.sync(cx);
                });
                cx.update_window(window.into(), |_, window, cx| {
                    transcript.update(cx, |this, cx| {
                        let _ = this.render_row(0, window, cx);
                        let _ = this.render_row(1, window, cx);
                        let old = &this.veils[&SharedString::from("reply#body.0")];
                        assert!(
                            old.borrow_mut()
                                .advance(0, "historial fuera de pantalla", Instant::now())
                                .is_empty()
                        );
                        let fresh = &this.veils[&SharedString::from("reply#fresh.0")];
                        let spans = fresh
                            .borrow_mut()
                            .advance(0, "texto en vivo", Instant::now());
                        assert_eq!(spans.len(), 1);
                        assert_eq!(spans[0].0, 0.."texto en vivo".len());
                    });
                })
                .unwrap();
            });
        }

        #[test]
        fn replay_text_prefix_is_visible_while_coalesced_live_suffix_fades() {
            with_window(|transcript, window, cx| {
                let history = vec![assistant(
                    "reply",
                    MessageStatus::Streaming,
                    vec![text_part("body", "café histórico")],
                )];
                let live = vec![assistant(
                    "reply",
                    MessageStatus::Streaming,
                    vec![text_part("body", "café histórico y nuevo\n\nOtro párrafo")],
                )];
                transcript.update(cx, |this, cx| {
                    this.state.update(cx, |state, cx| {
                        state.select_chat(Some("chat".into()), cx);
                        state
                            .receive_transcript_update(
                                openagents_doc::TranscriptUpdate {
                                    frame: openagents_doc::TranscriptFrame::reset(&history),
                                    context_usage: None,
                                    replay_baseline: Some(
                                        openagents_doc::TranscriptBaseline::capture(&history),
                                    ),
                                },
                                cx,
                            )
                            .unwrap();
                        state
                            .receive_transcript_update(
                                openagents_doc::TranscriptUpdate {
                                    frame: openagents_doc::diff_transcript(&history, &live),
                                    context_usage: None,
                                    replay_baseline: None,
                                },
                                cx,
                            )
                            .unwrap();
                    });
                    this.sync(cx);
                });
                cx.update_window(window.into(), |_, window, cx| {
                    transcript.update(cx, |this, cx| {
                        let _ = this.render_row(0, window, cx);
                        let _ = this.render_row(1, window, cx);
                        let first = &this.veils[&SharedString::from("reply#body.0")];
                        let spans =
                            first
                                .borrow_mut()
                                .advance(0, "café histórico y nuevo", Instant::now());
                        assert_eq!(spans.len(), 1);
                        assert_eq!(
                            spans[0].0,
                            "café histórico".len().."café histórico y nuevo".len()
                        );
                        let second = &this.veils[&SharedString::from("reply#body.1")];
                        let spans = second
                            .borrow_mut()
                            .advance(1, "Otro párrafo", Instant::now());
                        assert_eq!(spans.len(), 1);
                        assert_eq!(spans[0].0, 0.."Otro párrafo".len());
                    });
                })
                .unwrap();
                let veil = transcript.read(cx).veils[&SharedString::from("reply#body.0")].clone();
                // A second history batch in another entry must not restart
                // the fade of this already visible live suffix.
                let recovered = assistant(
                    "other",
                    MessageStatus::Complete,
                    vec![text_part("body", "otra respuesta histórica")],
                );
                let mut next = live.clone();
                next.push(recovered.clone());
                let mut next_history = history.clone();
                next_history.push(recovered);
                transcript.update(cx, |this, cx| {
                    this.state.update(cx, |state, cx| {
                        state
                            .receive_transcript_update(
                                openagents_doc::TranscriptUpdate {
                                    frame: openagents_doc::diff_transcript(&live, &next),
                                    context_usage: None,
                                    replay_baseline: Some(
                                        openagents_doc::TranscriptBaseline::capture(&next_history),
                                    ),
                                },
                                cx,
                            )
                            .unwrap();
                    });
                    this.sync(cx);
                    assert!(Rc::ptr_eq(
                        &veil,
                        &this.veils[&SharedString::from("reply#body.0")]
                    ));
                });
            });
        }

        #[test]
        fn text_streamed_under_reduced_motion_does_not_fade_in_when_motion_resumes() {
            fn render_rows(
                transcript: &Entity<Transcript>,
                window: gpui::WindowHandle<CachedTranscript>,
                cx: &mut gpui::App,
            ) {
                cx.update_window(window.into(), |_, window, cx| {
                    transcript.update(cx, |this, cx| {
                        for ix in 0..this.list.item_count() {
                            let _ = this.render_row(ix, window, cx);
                        }
                    });
                })
                .unwrap();
            }
            fn streaming(text: &str) -> Vec<SessionMessageEntry> {
                vec![
                    prompt("ask"),
                    assistant(
                        "reply",
                        MessageStatus::Streaming,
                        vec![text_part("body", text)],
                    ),
                ]
            }
            with_window(|transcript, window, cx| {
                let body = SharedString::from("reply#body.0");
                // Attach on the prompt alone, so the reply is live text rather
                // than an attach-time baseline.
                transcript.update(cx, |this, cx| feed(this, vec![prompt("ask")], cx));
                render_rows(&transcript, window, cx);

                // A row that first streams while motion is reduced.
                cx.set_reduce_motion(true);
                transcript.update(cx, |this, cx| feed(this, streaming("uno"), cx));
                render_rows(&transcript, window, cx);
                assert!(!transcript.read(cx).veils.contains_key(&body));
                cx.set_reduce_motion(false);
                render_rows(&transcript, window, cx);
                let veil = transcript.read(cx).veils[&body].clone();
                assert!(
                    veil.borrow_mut()
                        .advance(0, "uno", Instant::now())
                        .is_empty(),
                    "text painted under reduced motion must not dissolve in"
                );

                // A row already fading when motion pauses, then resumes.
                cx.set_reduce_motion(true);
                transcript.update(cx, |this, cx| feed(this, streaming("uno dos"), cx));
                render_rows(&transcript, window, cx);
                cx.set_reduce_motion(false);
                render_rows(&transcript, window, cx);
                let veil = transcript.read(cx).veils[&body].clone();
                assert!(
                    veil.borrow_mut()
                        .advance(0, "uno dos", Instant::now())
                        .is_empty(),
                    "text streamed while paused is already on screen"
                );
                let spans = veil.borrow_mut().advance(0, "uno dos tres", Instant::now());
                assert_eq!(spans.len(), 1);
                assert_eq!(spans[0].0, "uno dos".len().."uno dos tres".len());
            });
        }

        #[test]
        fn replay_growth_snaps_only_when_following_the_tail() {
            with_window(|transcript, window, cx| {
                let entries: Vec<_> = (0..30).map(|ix| prompt(&format!("prompt-{ix}"))).collect();
                transcript.update(cx, |this, cx| feed(this, entries.clone(), cx));
                draw(window, cx);
                let mut updated = entries;
                for pinned in [false, true] {
                    transcript.update(cx, |this, cx| {
                        this.pinned = pinned;
                        this.list.scroll_to(ListOffset {
                            item_ix: 5,
                            offset_in_item: px(12.0),
                        });
                        this.spring_kick = true;
                        updated.push(prompt(&format!("history-{}", updated.len())));
                        this.state.update(cx, |state, cx| {
                            let frame =
                                openagents_doc::diff_transcript(&state.transcript, &updated);
                            state
                                .receive_transcript_update(
                                    openagents_doc::TranscriptUpdate {
                                        frame,
                                        context_usage: None,
                                        replay_baseline: Some(
                                            openagents_doc::TranscriptBaseline::capture(&updated),
                                        ),
                                    },
                                    cx,
                                )
                                .unwrap();
                        });
                        this.sync(cx);
                        if !pinned {
                            let offset = this.list.logical_scroll_top();
                            assert_eq!(offset.item_ix, 5);
                            assert_eq!(offset.offset_in_item, px(12.0));
                        } else {
                            assert!(!this.spring_kick, "history must not start a scroll chase");
                        }
                    });
                    draw(window, cx);
                    if pinned {
                        assert!(transcript.read(cx).distance_from_bottom() <= 0.5);
                    }
                }
            });
        }

        // These exercise frame-by-frame geometry, including the first paint
        // after a row append. Eventual settling alone misses visible jumps.
        #[test]
        fn runway_short_chat_glides_from_its_bottom_aligned_position() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = vec![viewport_row("prompt", "prompt")];
                    this.list.reset(1);
                    this.rail_enabled = false;
                    cx.notify();
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                let mut previous = transcript.read(cx).list.bounds_for_item(0).unwrap().top();
                assert!(previous > px(400.0));
                for _ in 0..80 {
                    transcript.update(cx, |this, cx| {
                        this.own_turn_last_tick = Some(Instant::now() - Duration::from_millis(17));
                        this.step_own_turn(cx);
                    });
                    draw(window, cx);
                    let top = transcript.read(cx).list.bounds_for_item(0).unwrap().top();
                    assert!(top <= previous + px(0.5), "glide reversed");
                    assert!(
                        previous - top < px(150.0),
                        "glide jumped: {previous:?} -> {top:?}"
                    );
                    previous = top;
                }
                assert!(previous.abs() < px(1.0));
            });
        }

        fn append_runway_rows(this: &mut Transcript, count: usize, cx: &mut Context<Transcript>) {
            let old_last = this.rows.len() - 1;
            let mut rows = this.rows.clone();
            for ix in 0..count {
                rows.push(viewport_row(&format!("reply-{}", old_last + ix), "reply"));
            }
            // Isolate the row-splice layout boundary; the streaming tests
            // below exercise the real row builder and sync as well.
            this.list.splice(this.rows.len()..this.rows.len(), count);
            this.rows = rows;
            this.list.remeasure_items(old_last..old_last + 1);
            this.remeasure_last_row();
            this.own_turn_kick = true;
            cx.notify();
        }

        #[test]
        fn runway_append_consumes_space_before_the_first_paint() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = vec![viewport_row("prompt", "prompt")];
                    this.list.reset(1);
                    this.rail_enabled = false;
                    this.on_own_send("chat".into(), "prompt".into(), cx);
                    this.list.scroll_to(ListOffset::default());
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| this.step_own_turn(cx));
                draw(window, cx);
                let before = transcript.read(cx).list.max_offset_for_scrollbar().y;
                transcript.update(cx, |this, cx| append_runway_rows(this, 3, cx));
                draw(window, cx);
                let after = transcript.read(cx).list.max_offset_for_scrollbar().y;
                assert!(
                    (after - before).abs() <= px(1.0),
                    "append exposed blank scroll space: {before:?} -> {after:?}"
                );
            });
        }

        fn feed(
            this: &mut Transcript,
            entries: Vec<SessionMessageEntry>,
            cx: &mut Context<Transcript>,
        ) {
            this.state.update(cx, |state, _| {
                state.selected_chat = Some("chat".into());
                state.transcript_replayed = true;
                state.transcript = entries;
                state.transcript_revision += 1;
            });
            this.sync(cx);
        }

        fn prompt(id: &str) -> SessionMessageEntry {
            let mut entry = assistant(
                id,
                MessageStatus::Complete,
                vec![text_part("text", "Please explain this.")],
            );
            entry.role = MessageRole::User;
            entry
        }

        fn tick(
            transcript: &Entity<Transcript>,
            window: gpui::WindowHandle<CachedTranscript>,
            cx: &mut gpui::App,
        ) {
            transcript.update(cx, |this, cx| {
                this.own_turn_last_tick = Some(Instant::now() - Duration::from_millis(17));
                this.spring_last_tick = Some(Instant::now() - Duration::from_millis(17));
                if this.own_turn.is_some() {
                    this.step_own_turn(cx);
                }
                if this.pinned {
                    this.step_spring(cx);
                }
            });
            draw(window, cx);
        }

        fn wheel(window: gpui::WindowHandle<CachedTranscript>, delta: f32, cx: &mut gpui::App) {
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
                        position: gpui::point(px(500.0), px(400.0)),
                        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(delta))),
                        ..Default::default()
                    }),
                    cx,
                );
            })
            .unwrap();
        }

        #[test]
        fn selecting_live_tail_keeps_anchor_visible_during_a_stream_burst() {
            with_window(|transcript, window, cx| {
                let text = (0..40)
                    .map(|i| format!("Paragraph {i} has selectable response text.\n\n"))
                    .collect::<String>();
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![assistant(
                            "reply",
                            MessageStatus::Streaming,
                            vec![text_part("text", &text)],
                        )],
                        cx,
                    );
                    this.rail_enabled = false;
                });
                draw(window, cx);
                let bounds = render::selection_test_bounds("reply#text.39:39");
                let start = bounds.origin + gpui::point(px(1.0), px(8.0));
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                            button: MouseButton::Left,
                            position: start,
                            click_count: 1,
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
                assert!(crate::markdown::selection::is_dragging());
                assert!(
                    !transcript.read(cx).pinned,
                    "text mouse-down must release following"
                );
                assert!(
                    !transcript.read(cx).is_glued(),
                    "selection must materialize the viewport"
                );
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![assistant(
                            "reply",
                            MessageStatus::Streaming,
                            vec![text_part("text", &(text.clone() + &text))],
                        )],
                        cx,
                    );
                });
                draw(window, cx);
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                            position: start + gpui::point(px(90.0), px(0.0)),
                            pressed_button: Some(MouseButton::Left),
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
                assert!(
                    crate::markdown::selection::selected_text().is_some(),
                    "streaming must not virtualize the drag anchor before the first move"
                );
                crate::markdown::selection::end_active_drag();
                crate::markdown::selection::clear_if_owner("reply#text.39:39");
            });
        }

        #[test]
        fn active_reply_text_selection_survives_streaming_and_completion() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("prompt")], cx);
                    this.rail_enabled = false;
                    this.state.update(cx, |state, _| {
                        state.sessions.push(openagents_proto::Session {
                            last_completed_turn: None,
                            running_subagents: 0,
                            background_tasks: 0,
                            last_output_at: None,
                            chat_id: "chat".into(),
                            device_id: "test".into(),
                            status: openagents_proto::SessionStatus::Working,
                            started_at: Some(chrono::Utc::now()),
                            updated_at: chrono::Utc::now(),
                        })
                    });
                    this.on_own_send("chat".into(), "prompt".into(), cx);
                });
                let entries = |status, text: &str| {
                    vec![
                        prompt("prompt"),
                        assistant("reply", status, vec![text_part("text", text)]),
                    ]
                };
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        entries(MessageStatus::Streaming, "Selectable response text."),
                        cx,
                    )
                });
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let bounds = render::selection_test_bounds("reply#text.0:0");
                let start = bounds.origin + gpui::point(px(1.0), px(8.0));
                let end = start + gpui::point(px(90.0), px(0.0));
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                            button: MouseButton::Left,
                            position: start,
                            click_count: 1,
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
                assert!(crate::markdown::selection::is_dragging());
                draw(window, cx);
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                            position: end,
                            pressed_button: Some(MouseButton::Left),
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
                let selected =
                    crate::markdown::selection::selected_text().expect("active text must select");
                assert!(!selected.is_empty());
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        entries(
                            MessageStatus::Streaming,
                            "Selectable response text. More output.",
                        ),
                        cx,
                    )
                });
                draw(window, cx);
                assert_eq!(
                    crate::markdown::selection::selected_text(),
                    Some(selected.clone())
                );
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                            button: MouseButton::Left,
                            position: end,
                            ..Default::default()
                        }),
                        cx,
                    );
                })
                .unwrap();
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        entries(
                            MessageStatus::Complete,
                            "Selectable response text. More output.",
                        ),
                        cx,
                    )
                });
                draw(window, cx);
                assert_eq!(crate::markdown::selection::selected_text(), Some(selected));
                crate::markdown::selection::clear_if_owner("reply#text.0:0");
            });
        }

        #[test]
        fn runway_first_echo_starts_a_glide_in_an_empty_chat() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx); // notification before the optimistic echo
                transcript.update(cx, |this, cx| feed(this, vec![prompt("prompt")], cx));
                draw(window, cx);
                let mut previous = transcript.read(cx).list.bounds_for_item(0).unwrap().top();
                assert!(
                    previous > px(400.0),
                    "first echo skipped its glide: {previous:?}"
                );
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                    let top = transcript.read(cx).list.bounds_for_item(0).unwrap().top();
                    assert!(top <= previous + px(0.5));
                    assert!(previous - top < px(150.0));
                    previous = top;
                }
                assert!(previous.abs() <= px(1.0));
            });
        }

        #[test]
        fn runway_real_stream_consumes_reservation_then_follows_until_completion() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("prompt")], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let mut text = String::new();
                let mut handed_off = false;
                for chunk in 0..30 {
                    text.push_str(&format!("\n\nSection {chunk}. A paragraph to explain the result.\n\n```text\ncontent {chunk}\n```\n"));
                    transcript.update(cx, |this, cx| {
                        feed(
                            this,
                            vec![
                                prompt("prompt"),
                                assistant(
                                    "reply",
                                    MessageStatus::Streaming,
                                    vec![text_part("text", &text)],
                                ),
                            ],
                            cx,
                        )
                    });
                    draw(window, cx); // assert the first paint, before correction
                    let this = transcript.read(cx);
                    if this.own_turn.is_some() && !this.list.tail_reservation_filled() {
                        assert!(
                            this.list.max_offset_for_scrollbar().y <= px(2.5),
                            "provisional blank space after chunk {chunk}"
                        );
                    }
                    for _ in 0..50 {
                        tick(&transcript, window, cx);
                    }
                    let this = transcript.read(cx);
                    if this.own_turn.is_none() {
                        handed_off = true;
                        assert!(this.pinned, "overflow lost automatic following");
                        assert!(
                            this.distance_from_bottom() <= 1.0,
                            "stream stopped following at chunk {chunk}"
                        );
                    }
                }
                assert!(handed_off, "long output never retired the runway");
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![
                            prompt("prompt"),
                            assistant(
                                "reply",
                                MessageStatus::Complete,
                                vec![text_part("text", &text)],
                            ),
                        ],
                        cx,
                    )
                });
                draw(window, cx);
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                assert!(transcript.read(cx).distance_from_bottom() <= 1.0);
            });
        }

        #[test]
        fn runway_wheel_down_cannot_enter_a_temporary_gap_or_reverse() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = vec![viewport_row("prompt", "prompt")];
                    this.list.reset(1);
                    this.rail_enabled = false;
                    this.on_own_send("chat".into(), "prompt".into(), cx);
                    this.list.scroll_to(ListOffset::default());
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| append_runway_rows(this, 3, cx));
                draw(window, cx);
                let mut previous = px(0.0);
                for _ in 0..20 {
                    wheel(window, -60.0, cx);
                    assert!(
                        !transcript.read(cx).own_turn.as_ref().unwrap().held,
                        "real wheel event must release the hold"
                    );
                    tick(&transcript, window, cx);
                    let this = transcript.read(cx);
                    let top = this
                        .list
                        .bounds_for_item(0)
                        .map(|bounds| bounds.top())
                        .unwrap_or_else(|| {
                            // A genuine bottom pin uses GPUI's end sentinel, for
                            // which bounds_for_item intentionally returns None.
                            this.list.viewport_bounds().top()
                                + this.list.offset_for_item(0)
                                + this.list.scroll_px_offset_for_scrollbar().y
                        });
                    assert!(top >= px(-2.5), "wheel entered a blank runway: {top:?}");
                    assert!(
                        top <= previous + px(0.5),
                        "downward wheel reversed: {previous:?} -> {top:?}"
                    );
                    previous = top;
                }
            });
        }

        #[test]
        fn runway_background_burst_and_downward_input_reach_the_new_tail() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("prompt")], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                // Apply many commits with no layouts or animation frames,
                // just as when the native window has stopped requesting them.
                let mut text = String::new();
                for chunk in 0..40 {
                    text.push_str(&format!(
                        "\n\nSection {chunk}\n\n```text\nresult {chunk}\n```\n"
                    ));
                    transcript.update(cx, |this, cx| {
                        feed(
                            this,
                            vec![
                                prompt("prompt"),
                                assistant(
                                    "reply",
                                    MessageStatus::Streaming,
                                    vec![text_part("text", &text)],
                                ),
                            ],
                            cx,
                        )
                    });
                }
                draw(window, cx);
                let mut previous = -transcript.read(cx).list.scroll_px_offset_for_scrollbar().y;
                for _ in 0..100 {
                    wheel(window, -180.0, cx);
                    tick(&transcript, window, cx);
                    let this = transcript.read(cx);
                    let current = -this.list.scroll_px_offset_for_scrollbar().y;
                    assert!(
                        current >= previous - px(1.0),
                        "refocus wheel snapped backward: {previous:?} -> {current:?}"
                    );
                    previous = current;
                }
                let this = transcript.read(cx);
                assert!(this.own_turn.is_none());
                assert!(
                    this.distance_from_bottom() <= 1.0,
                    "downward input never reached new output"
                );
                assert!(previous > px(800.0));
            });
        }

        #[test]
        fn runway_second_send_and_steer_keep_the_previous_viewport_until_echo() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("first")], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "first".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let mut entries = vec![prompt("first")];
                for (ix, status) in [MessageStatus::Complete, MessageStatus::Streaming]
                    .into_iter()
                    .enumerate()
                {
                    entries.push(assistant(
                        &format!("reply-{ix}"),
                        status,
                        vec![text_part("text", "A short answer.")],
                    ));
                    transcript.update(cx, |this, cx| feed(this, entries.clone(), cx));
                    draw(window, cx);
                    for _ in 0..10 {
                        tick(&transcript, window, cx);
                    }
                    let before = transcript.read(cx).list.scroll_px_offset_for_scrollbar().y;
                    let id = format!("next-{ix}");
                    transcript.update(cx, |this, cx| {
                        this.on_own_send("chat".into(), id.clone(), cx)
                    });
                    draw(window, cx); // the echoed prompt has not landed yet
                    assert!(
                        (transcript.read(cx).list.scroll_px_offset_for_scrollbar().y - before)
                            .abs()
                            <= px(1.0),
                        "waiting for echo moved the viewport"
                    );
                    entries.push(prompt(&id));
                    transcript.update(cx, |this, cx| feed(this, entries.clone(), cx));
                    draw(window, cx);
                    let anchor = transcript.read(cx).own_turn_anchor_ix().unwrap();
                    let mut previous = transcript
                        .read(cx)
                        .list
                        .bounds_for_item(anchor)
                        .unwrap()
                        .top();
                    assert!(previous > px(Transcript::own_send_inset(anchor) + 40.0));
                    for _ in 0..80 {
                        tick(&transcript, window, cx);
                        let top = transcript
                            .read(cx)
                            .list
                            .bounds_for_item(anchor)
                            .unwrap()
                            .top();
                        assert!(top <= previous + px(0.5), "repeat send reversed");
                        assert!(
                            top >= px(Transcript::own_send_inset(anchor) - 2.5),
                            "repeat send overshot"
                        );
                        assert!(previous - top < px(150.0), "repeat send jumped");
                        previous = top;
                    }
                }
            });
        }

        #[test]
        fn runway_user_scroll_up_stays_released_when_output_overflows() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    let mut entries = vec![prompt("old")];
                    entries.push(assistant(
                        "history",
                        MessageStatus::Complete,
                        vec![text_part("text", &"History paragraph.\n\n".repeat(80))],
                    ));
                    entries.push(prompt("prompt"));
                    feed(this, entries, cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                wheel(window, 300.0, cx);
                draw(window, cx);
                assert!(!transcript.read(cx).own_turn.as_ref().unwrap().held);
                let before = transcript.read(cx).list.logical_scroll_top();
                transcript.update(cx, |this, cx| {
                    let mut entries = this.state.read(cx).transcript.clone();
                    entries.push(assistant(
                        "reply",
                        MessageStatus::Streaming,
                        vec![text_part("text", &"Long streamed reply.\n\n".repeat(80))],
                    ));
                    feed(this, entries, cx);
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let this = transcript.read(cx);
                assert!(!this.pinned, "background growth stole the user's viewport");
                let after = this.list.logical_scroll_top();
                assert_eq!(before.item_ix, after.item_ix);
                assert!((before.offset_in_item - after.offset_in_item).abs() <= px(1.0));
            });
        }

        #[test]
        fn runway_resizes_in_the_same_layout_without_a_provisional_gap() {
            struct SizedTranscript {
                transcript: Entity<Transcript>,
                height: f32,
            }
            impl Render for SizedTranscript {
                fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                    div()
                        .w_full()
                        .h(px(self.height))
                        .child(self.transcript.clone())
                }
            }
            with_window(|transcript, _, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = vec![viewport_row("prompt", "prompt")];
                    this.list.reset(1);
                    this.rail_enabled = false;
                    this.on_own_send("chat".into(), "prompt".into(), cx);
                    this.list.scroll_to(ListOffset::default());
                });
                let window = cx
                    .open_window(gpui::WindowOptions::default(), |_, cx| {
                        cx.new(|_| SizedTranscript {
                            transcript: transcript.clone(),
                            height: 600.0,
                        })
                    })
                    .unwrap();
                for height in [600.0, 900.0, 450.0, 800.0] {
                    window
                        .update(cx, |root, window, cx| {
                            root.height = height;
                            cx.notify();
                            window.refresh();
                        })
                        .unwrap();
                    cx.update_window(window.into(), |_, window, cx| {
                        let _ = window.draw(cx);
                    })
                    .unwrap();
                    let this = transcript.read(cx);
                    assert_eq!(this.list.viewport_bounds().size.height, px(height));
                    assert!(
                        (this.list.max_offset_for_scrollbar().y - px(2.0)).abs() <= px(0.5),
                        "resize exposed blank space"
                    );
                    assert!(this.list.bounds_for_item(0).unwrap().top().abs() <= px(0.5));
                }
            });
        }

        #[test]
        fn runway_direct_jump_to_an_unmeasured_tail_does_not_reserve_unknown_rows() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = (0..100)
                        .map(|ix| viewport_row(&format!("row-{ix}"), &format!("entry-{ix}")))
                        .collect();
                    this.list.reset(100);
                    this.list.scroll_to(ListOffset {
                        item_ix: 99,
                        offset_in_item: px(0.0),
                    });
                    this.pinned = false;
                    this.rail_enabled = false;
                    cx.notify();
                });
                draw(window, cx);
                let natural = transcript.read(cx).list.offset_for_item(100)
                    - transcript.read(cx).list.offset_for_item(99);
                transcript.update(cx, |this, cx| {
                    this.list.reset(100); // discard all prefix height hints
                    this.on_own_send("chat".into(), "entry-0".into(), cx);
                    this.release_own_turn_hold();
                    this.list.scroll_to(ListOffset {
                        item_ix: 99,
                        offset_in_item: px(0.0),
                    });
                });
                draw(window, cx);
                let this = transcript.read(cx);
                assert_eq!(
                    this.list.offset_for_item(100) - this.list.offset_for_item(99),
                    natural,
                    "unknown prefix rows created a blank tail"
                );
                assert!(this.distance_from_bottom() <= 1.0);
            });
        }

        #[test]
        fn runway_wheel_down_at_the_end_keeps_following_when_streaming_overflows() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("prompt")], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                wheel(window, -60.0, cx);
                // Let the real wheel handler's deferred ListState read finish
                // before delivering more output, as in the native event loop.
                cx.defer(move |cx| {
                    assert!(
                        transcript.read(cx).pinned,
                        "downward input at the runway end must retain follow intent"
                    );
                    let mut text = String::new();
                    for chunk in 0..20 {
                        text.push_str(&format!(
                            "\n\nSection {chunk}\n\n```text\ncontent {chunk}\n```\n"
                        ));
                        transcript.update(cx, |this, cx| {
                            feed(
                                this,
                                vec![
                                    prompt("prompt"),
                                    assistant(
                                        "reply",
                                        MessageStatus::Streaming,
                                        vec![text_part("text", &text)],
                                    ),
                                ],
                                cx,
                            )
                        });
                        draw(window, cx);
                        for _ in 0..40 {
                            tick(&transcript, window, cx);
                        }
                    }
                    let this = transcript.read(cx);
                    assert!(this.own_turn.is_none());
                    assert!(this.pinned);
                    assert!(
                        this.distance_from_bottom() <= 1.0,
                        "output stopped following after the runway filled"
                    );
                });
            });
        }

        #[test]
        fn runway_down_then_up_before_overflow_preserves_the_user_viewport() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![
                            prompt("old"),
                            assistant(
                                "history",
                                MessageStatus::Complete,
                                vec![text_part("text", &"History.\n\n".repeat(80))],
                            ),
                            prompt("prompt"),
                        ],
                        cx,
                    );
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                wheel(window, -60.0, cx);
                cx.defer(move |cx| {
                    assert!(transcript.read(cx).pinned);
                    draw(window, cx);
                    wheel(window, 300.0, cx);
                    assert!(
                        !transcript.read(cx).pinned,
                        "upward input must cancel the spring synchronously"
                    );
                    cx.defer(move |cx| {
                        assert!(!transcript.read(cx).pinned);
                        let before = transcript.read(cx).list.logical_scroll_top();
                        transcript.update(cx, |this, cx| {
                            let mut entries = this.state.read(cx).transcript.clone();
                            entries.push(assistant(
                                "reply",
                                MessageStatus::Streaming,
                                vec![text_part("text", &"New output.\n\n".repeat(80))],
                            ));
                            feed(this, entries, cx);
                        });
                        draw(window, cx);
                        for _ in 0..80 {
                            tick(&transcript, window, cx);
                        }
                        let this = transcript.read(cx);
                        assert!(!this.pinned);
                        let after = this.list.logical_scroll_top();
                        assert_eq!(before.item_ix, after.item_ix);
                        assert!((before.offset_in_item - after.offset_in_item).abs() <= px(1.0));
                    });
                });
            });
        }

        #[test]
        fn runway_absorbs_tail_shrinkage_in_the_same_layout() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    let mut row = viewport_row("prompt", "prompt");
                    row.kind = RowKind::ErrorChip {
                        message: "line\n".repeat(12).into(),
                    };
                    this.rows = vec![row];
                    this.list.reset(1);
                    this.pinned = false;
                    this.rail_enabled = false;
                    cx.notify();
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.list.scroll_to(ListOffset {
                        item_ix: 0,
                        offset_in_item: px(0.0),
                    });
                    this.step_own_turn(cx);
                });
                draw(window, cx);
                let before = transcript.read(cx).list.bounds_for_item(0).unwrap();
                transcript.update(cx, |this, cx| {
                    // Simulate completion removing content from the last row.
                    this.rows[0].kind = RowKind::ErrorChip {
                        message: "done".into(),
                    };
                    this.list.remeasure_items(0..1);
                    cx.notify();
                });
                // No controller tick between the change and this paint.
                draw(window, cx);
                let after = transcript.read(cx).list.bounds_for_item(0).unwrap();
                assert_eq!(
                    before.top(),
                    after.top(),
                    "completion must not move the prompt"
                );
                assert_eq!(
                    before.size.height, after.size.height,
                    "the runway absorbs the shrink"
                );
                transcript.update(cx, |this, cx| {
                    this.rows[0].kind = RowKind::ErrorChip {
                        message: "line\n".repeat(100).into(),
                    };
                    this.list.remeasure_items(0..1);
                    cx.notify();
                });
                draw(window, cx);
                let overflow_height = transcript
                    .read(cx)
                    .list
                    .bounds_for_item(0)
                    .unwrap()
                    .size
                    .height;
                transcript.update(cx, |this, cx| {
                    this.step_own_turn(cx);
                    assert!(
                        this.own_turn.is_none(),
                        "overflow must retire the reservation"
                    );
                    assert!(this.pinned, "a held turn hands off to tail-follow");
                });
                draw(window, cx);
                assert_eq!(
                    transcript
                        .read(cx)
                        .list
                        .bounds_for_item(0)
                        .unwrap()
                        .size
                        .height,
                    overflow_height,
                    "retiring the minimum must be height-neutral"
                );
            });
        }

        /// Run the next queued diagram synchronously, as the worker would.
        fn render_next_diagram(transcript: &Entity<Transcript>, cx: &mut gpui::App) -> bool {
            transcript.update(cx, |this, cx| {
                let Some(code) = this.media.diagrams.borrow_mut().next_job() else {
                    return false;
                };
                let palette = crate::markdown::mermaid::Palette::from_theme(Theme::of(cx));
                let result = crate::markdown::mermaid::render(&code, &palette).and_then(|svg| {
                    crate::image_media::decode_image("image/svg+xml", svg.into_bytes())
                });
                assert!(result.is_ok(), "fixture diagram must render");
                DesktopMedia::finish_diagram(
                    this,
                    code,
                    crate::theme::style_generation(),
                    result,
                    cx,
                );
                true
            })
        }

        const MERMAID_FENCE: &str =
            "```mermaid\nflowchart TD\n    A[Request] --> B[Plan]\n    B --> C[Done]\n```\n";

        #[test]
        fn mermaid_streaming_tail_keeps_source_until_the_reply_moves_past_it() {
            with_window(|transcript, window, cx| {
                let reply = |status, text: String| {
                    vec![
                        prompt("prompt"),
                        assistant("reply", status, vec![text_part("text", &text)]),
                    ]
                };
                let mut text = String::from("Intro.\n\n");
                // Token-sized commits through the open fence, then the closed
                // fence while it is still the reply's tail.
                for token in MERMAID_FENCE.split_inclusive([' ', '\n']) {
                    text.push_str(token);
                    transcript.update(cx, |this, cx| {
                        this.rail_enabled = false;
                        feed(this, reply(MessageStatus::Streaming, text.clone()), cx)
                    });
                    tick(&transcript, window, cx);
                    let this = transcript.read(cx);
                    assert!(
                        this.media.diagram_worker.is_none(),
                        "a streaming tail fence must not start a render"
                    );
                    assert!(this.media.diagrams.borrow_mut().next_job().is_none());
                }
                // Enough reply after the fence that the viewport can anchor on
                // it without clamping back to the end.
                text.push_str(&format!(
                    "\nAfter the diagram. {}",
                    "More detail. ".repeat(300)
                ));
                transcript.update(cx, |this, cx| {
                    feed(this, reply(MessageStatus::Streaming, text.clone()), cx)
                });
                draw(window, cx);
                assert!(
                    transcript.read(cx).diagram_worker.is_some(),
                    "a fence followed by more reply renders"
                );
                // A pinned end is glued, where rows report no bounds; read
                // them with the viewport anchored at the diagram row instead.
                transcript.update(cx, |this, cx| {
                    this.pinned = false;
                    let ix = this
                        .rows
                        .iter()
                        .position(|row| row.id.as_ref() == "reply#text.1")
                        .unwrap();
                    this.list.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: px(0.0),
                    });
                    cx.notify();
                });
                draw(window, cx);
                let fence_height = |cx: &gpui::App| {
                    let this = transcript.read(cx);
                    let ix = this
                        .rows
                        .iter()
                        .position(|row| row.id.as_ref() == "reply#text.1")
                        .unwrap();
                    this.list.bounds_for_item(ix).unwrap().size.height
                };
                let source_height = fence_height(cx);
                assert!(render_next_diagram(&transcript, cx));
                assert!(
                    !render_next_diagram(&transcript, cx),
                    "one request per source"
                );
                draw(window, cx);
                assert_ne!(
                    fence_height(cx),
                    source_height,
                    "the diagram replaced its source"
                );
                let frame: SharedString = "reply#text.1-mermaid-1".into();
                transcript.update(cx, |this, cx| {
                    assert_eq!(this.media.diagrams.borrow().ready_count(), 1);
                    this.media.diagrams.borrow_mut().toggle_source(&frame);
                    this.diagram_layout_changed(&["reply#text.1".into()], cx);
                });
                draw(window, cx);
                assert_eq!(
                    fence_height(cx),
                    source_height,
                    "the toggle restores the source"
                );
                // Completion keeps row identity, the rendered diagram and
                // the reader's source toggle; nothing renders again.
                transcript.update(cx, |this, cx| {
                    feed(this, reply(MessageStatus::Complete, text.clone()), cx)
                });
                draw(window, cx);
                let this = transcript.read(cx);
                assert!(this.media.diagrams.borrow().source_visible(&frame));
                assert!(this.media.diagrams.borrow_mut().next_job().is_none());
            });
        }

        #[test]
        fn mermaid_diagram_swap_keeps_a_pinned_stream_at_the_bottom() {
            with_window(|transcript, window, cx| {
                let intro = "A paragraph that explains the result in detail. ".repeat(12);
                let reply = |status, tail: &str| {
                    let mut text = String::new();
                    for section in 0..8 {
                        text.push_str(&format!("Section {section}. {intro}\n\n"));
                    }
                    text.push_str(MERMAID_FENCE);
                    text.push_str(tail);
                    vec![
                        prompt("prompt"),
                        assistant("reply", status, vec![text_part("text", &text)]),
                    ]
                };
                transcript.update(cx, |this, cx| {
                    this.rail_enabled = false;
                    feed(
                        this,
                        reply(MessageStatus::Streaming, "\nAfter the diagram."),
                        cx,
                    )
                });
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                let this = transcript.read(cx);
                assert!(this.pinned);
                assert!(
                    this.list.max_offset_for_scrollbar().y > px(100.0),
                    "must overflow"
                );
                assert!(this.distance_from_bottom() <= 1.0);
                assert!(render_next_diagram(&transcript, cx));
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                let this = transcript.read(cx);
                assert!(this.pinned, "a diagram swap is not user input");
                assert!(this.distance_from_bottom() <= 1.0, "pin follows the swap");
                // More streaming after the swap keeps following.
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        reply(
                            MessageStatus::Streaming,
                            &format!("\nAfter the diagram.\n\n{intro}\n\n{intro}"),
                        ),
                        cx,
                    )
                });
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                let this = transcript.read(cx);
                assert!(this.pinned);
                assert!(this.distance_from_bottom() <= 1.0);
                assert!(this.media.diagrams.borrow_mut().next_job().is_none());
            });
        }

        #[test]
        fn mermaid_completion_renders_the_tail_without_moving_the_runway_prompt() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    feed(this, vec![prompt("prompt")], cx);
                    this.rail_enabled = false;
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "prompt".into(), cx)
                });
                draw(window, cx);
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let reply = |status| {
                    vec![
                        prompt("prompt"),
                        assistant(
                            "reply",
                            status,
                            vec![text_part(
                                "text",
                                &format!("Here it is:\n\n{MERMAID_FENCE}"),
                            )],
                        ),
                    ]
                };
                transcript.update(cx, |this, cx| {
                    feed(this, reply(MessageStatus::Streaming), cx)
                });
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                assert!(transcript.read(cx).diagram_worker.is_none());
                transcript.update(cx, |this, cx| {
                    feed(this, reply(MessageStatus::Complete), cx)
                });
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                let prompt_top = |cx: &gpui::App| {
                    let this = transcript.read(cx);
                    assert!(this.own_turn.is_some(), "a short reply keeps its runway");
                    this.list.bounds_for_item(0).unwrap().top()
                };
                let before = prompt_top(cx);
                assert!(render_next_diagram(&transcript, cx));
                // No controller tick between the swap and this paint.
                draw(window, cx);
                assert_eq!(prompt_top(cx), before, "the diagram swap moved the prompt");
                let this = transcript.read(cx);
                assert!(
                    this.list.max_offset_for_scrollbar().y <= px(2.5),
                    "the swap opened blank space below the runway"
                );
                for _ in 0..50 {
                    tick(&transcript, window, cx);
                }
                assert_eq!(prompt_top(cx), before);

                // The lightbox frames an enlarged raster by the diagram's
                // natural size and releases it on close.
                let media = transcript
                    .read(cx)
                    .diagrams
                    .borrow()
                    .ready_media()
                    .expect("rendered diagram");
                cx.update_window(window.into(), |_, window, cx| {
                    transcript.update(cx, |this, cx| {
                        DesktopMedia::open_diagram_preview(this, media.clone(), window, cx)
                    });
                })
                .unwrap();
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    assert!(this.media.attachment_preview.is_some());
                    assert!(this.media.diagram_zoom.is_some());
                    DesktopMedia::close_diagram_zoom(this, cx);
                    assert!(this.media.attachment_preview.is_none());
                    assert!(this.media.diagram_zoom.is_none());
                });
                draw(window, cx);
            });
        }

        #[test]
        fn send_glide_never_crosses_the_prompt_during_remeasurement() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = (0..12)
                        .map(|ix| viewport_row(&format!("row-{ix}"), &format!("entry-{ix}")))
                        .collect();
                    this.list.reset(this.rows.len());
                    this.rail_enabled = false;
                    cx.notify();
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    this.on_own_send("chat".into(), "entry-11".into(), cx)
                });
                draw(window, cx);
                let start_top = transcript.read(cx).list.bounds_for_item(11).unwrap().top();
                assert!(
                    start_top > px(Transcript::own_send_inset(11) + 100.0),
                    "installing the runway must preserve the start of the glide"
                );
                let mut previous_top = start_top;
                for _ in 0..90 {
                    draw(window, cx);
                    transcript.update(cx, |this, cx| {
                        // Pending-echo changes can invalidate the prompt before
                        // the queued glide runs. Exercise that exact ordering.
                        this.remeasure_last_row();
                        this.own_turn_last_tick = Some(Instant::now() - Duration::from_millis(17));
                        this.step_own_turn(cx);
                    });
                    draw(window, cx);
                    let this = transcript.read(cx);
                    let bounds = this.list.bounds_for_item(11).unwrap();
                    assert!(
                        bounds.top() <= previous_top + px(0.5),
                        "the glide must not reverse"
                    );
                    previous_top = bounds.top();
                    let target =
                        this.list.viewport_bounds().top() + px(Transcript::own_send_inset(11));
                    assert!(
                        bounds.top() >= target - px(0.5),
                        "send overshot: {:?} < {:?}",
                        bounds.top(),
                        target
                    );
                }
                let this = transcript.read(cx);
                let bounds = this.list.bounds_for_item(11).unwrap();
                assert!(
                    (f32::from(bounds.top() - this.list.viewport_bounds().top())
                        - Transcript::own_send_inset(11))
                    .abs()
                        <= 1.0
                );
            });
        }

        #[test]
        fn background_overflow_retires_hold_before_the_tail_is_measured() {
            with_window(|transcript, window, cx| {
                transcript.update(cx, |this, cx| {
                    this.rows = (0..100)
                        .map(|ix| viewport_row(&format!("row-{ix}"), &format!("entry-{ix}")))
                        .collect();
                    this.list.reset(this.rows.len());
                    this.list.scroll_to(ListOffset {
                        item_ix: 0,
                        offset_in_item: px(0.0),
                    });
                    this.pinned = false;
                    this.rail_enabled = false;
                    this.own_turn = Some(OwnTurnAnchor {
                        chat_id: "chat".into(),
                        message_id: "entry-0".into(),
                        held: true,
                        positioned: true,
                        seen_prompt: true,
                    });
                    cx.notify();
                });
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    assert!(
                        this.list.bounds_for_item(99).is_none(),
                        "tail must remain virtualized"
                    );
                    this.step_own_turn(cx);
                    assert!(
                        this.own_turn.is_none(),
                        "a filled hold cannot wait on off-screen bounds"
                    );
                    assert!(this.pinned);
                });
            });
        }

        #[test]
        fn wheel_down_releases_stale_hold_before_a_background_frame_can_run() {
            with_transcript(|this, cx| {
                this.rows = vec![
                    viewport_row("prompt", "prompt"),
                    viewport_row("reply", "reply"),
                ];
                this.list.reset(2);
                this.own_turn = Some(OwnTurnAnchor {
                    chat_id: "chat".into(),
                    message_id: "prompt".into(),
                    held: true,
                    positioned: true,
                    seen_prompt: true,
                });
                this.own_turn_scheduled = true;
                // A downward scroll into output received while no layout/frame
                // callbacks were running. No preceding upward gesture.
                this.list.scroll_to(ListOffset {
                    item_ix: 1,
                    offset_in_item: px(20.0),
                });
                this.handle_scroll(
                    &ListScrollEvent {
                        visible_range: 1..2,
                        count: 2,
                        is_scrolled: true,
                        is_following_tail: false,
                    },
                    cx,
                );
                assert!(
                    !this.own_turn.as_ref().unwrap().held,
                    "input must cancel the queued hold synchronously"
                );
                let entity = cx.entity();
                cx.defer(move |cx| {
                    let this = entity.read(cx);
                    assert!(!this.own_turn.as_ref().unwrap().held);
                    assert_eq!(this.list.logical_scroll_top().item_ix, 1);
                    assert_eq!(this.list.logical_scroll_top().offset_in_item, px(20.0));
                });
            });
        }

        #[test]
        fn expanding_streaming_prompt_does_not_retire_its_runway() {
            with_window(|transcript, window, cx| {
                let mut user = prompt("prompt");
                user.parts = vec![text_part("text", &"A long prompt line.\n".repeat(80))];
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![
                            user.clone(),
                            assistant(
                                "reply",
                                MessageStatus::Streaming,
                                vec![text_part("text", "A short live reply.")],
                            ),
                        ],
                        cx,
                    );
                    this.rail_enabled = false;
                    this.on_own_send("chat".into(), "prompt".into(), cx);
                });
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                let before = transcript.read(cx).list.max_offset_for_scrollbar();
                let toggle = |this: &mut Transcript, cx: &mut Context<Transcript>| {
                    let full_h = this.user_heights["prompt"].get();
                    this.toggle_user_fold(
                        "prompt".into(),
                        0,
                        USER_LINE_HEIGHT * (USER_COLLAPSED_LINES + 1) as f32,
                        full_h,
                        true,
                    );
                    this.user_folds.get_mut("prompt").unwrap().toggled_at =
                        Some(Instant::now() - Duration::from_secs(5));
                    cx.notify();
                };
                transcript.update(cx, toggle);
                draw(window, cx);
                tick(&transcript, window, cx);
                assert!(
                    transcript.read(cx).own_turn.is_some(),
                    "Show more must not consume the runway as assistant output"
                );
                transcript.update(cx, toggle);
                draw(window, cx);
                tick(&transcript, window, cx);
                assert!(transcript.read(cx).own_turn.is_some());
                assert!(
                    (transcript.read(cx).list.max_offset_for_scrollbar().y - before.y).abs()
                        <= px(1.0),
                    "Show less must restore the original reservation"
                );
                transcript.update(cx, toggle);
                draw(window, cx);
                transcript.update(cx, |this, cx| {
                    feed(
                        this,
                        vec![
                            user,
                            assistant(
                                "reply",
                                MessageStatus::Streaming,
                                vec![text_part("text", &"More assistant output.\n\n".repeat(80))],
                            ),
                        ],
                        cx,
                    )
                });
                transcript.update(cx, |this, cx| this.jump_to_bottom(cx));
                for _ in 0..80 {
                    tick(&transcript, window, cx);
                }
                assert!(
                    transcript.read(cx).own_turn.is_none(),
                    "real output must still retire the reservation while the prompt is expanded"
                );
            });
        }

        #[test]
        fn folding_releases_sent_turn_hold_without_removing_reservation() {
            with_transcript(|transcript, _| {
                for reduced_motion in [false, true] {
                    for open in [false, true] {
                        transcript.own_turn = Some(OwnTurnAnchor {
                            chat_id: "chat".into(),
                            message_id: "prompt".into(),
                            held: true,
                            positioned: true,
                            seen_prompt: true,
                        });
                        transcript.pinned = true;
                        transcript.spring_kick = true;
                        transcript.own_turn_last_tick = Some(Instant::now());
                        transcript
                            .user_folds
                            .entry("prompt".into())
                            .or_default()
                            .open = Some(open);

                        // No row bounds yet: ownership must transfer even if
                        // geometry is unavailable during a list remeasurement.
                        transcript.toggle_user_fold(
                            "prompt".into(),
                            0,
                            110.0,
                            2200.0,
                            reduced_motion,
                        );

                        let turn = transcript.own_turn.as_ref().unwrap();
                        assert!(
                            !turn.held,
                            "an outgrown reservation must not re-engage the pin"
                        );
                        assert!(transcript.own_turn_last_tick.is_none());
                        assert!(!transcript.pinned);
                        assert!(!transcript.spring_kick);
                        assert_eq!(transcript.user_folds["prompt"].open, Some(!open));
                    }
                }
            });
        }

        #[test]
        fn navigation_cancels_fold_compensation_before_queued_frame() {
            with_transcript(|transcript, cx| {
                for navigation in ["wheel", "rail", "bottom", "send"] {
                    transcript.user_collapse_scroll = Some(UserCollapseScroll {
                        started_at: Instant::now(),
                        duration_ms: 850,
                        height_delta: 2000.0,
                        row_ix: 0,
                        initial_top: -1000.0,
                        target_top: 80.0,
                    });
                    transcript.user_collapse_scroll_scheduled = true;
                    let hold_token = transcript.user_hold_token;
                    match navigation {
                        "wheel" => transcript.handle_scroll(
                            &ListScrollEvent {
                                visible_range: 0..0,
                                count: 0,
                                is_scrolled: true,
                                is_following_tail: false,
                            },
                            cx,
                        ),
                        "rail" => transcript.begin_scroll_navigation(),
                        "bottom" => transcript.jump_to_bottom(cx),
                        "send" => transcript.on_own_send("chat".into(), "prompt".into(), cx),
                        _ => unreachable!(),
                    }
                    assert!(transcript.user_collapse_scroll.is_none(), "{navigation}");
                    assert_ne!(
                        transcript.user_hold_token, hold_token,
                        "cancel stale long presses"
                    );
                    assert!(
                        transcript.user_collapse_scroll_scheduled,
                        "keep the queued-frame guard"
                    );

                    // A frame queued before the input must neither move the
                    // viewport nor resurrect the canceled compensation.
                    let offset = transcript.list.logical_scroll_top();
                    transcript.user_collapse_scroll_scheduled = false;
                    transcript.step_user_collapse_scroll(cx);
                    let after = transcript.list.logical_scroll_top();
                    assert_eq!(after.item_ix, offset.item_ix);
                    assert_eq!(after.offset_in_item, offset.offset_in_item);
                    assert!(transcript.user_collapse_scroll.is_none());
                }
            });
        }
    }

    /// Explicit multiline and long soft-wrapped prompts get a fold affordance;
    /// short messages stay untouched.
    #[test]
    fn long_prompts_collapse_and_short_ones_do_not() {
        assert!(!user_message_needs_collapse("short message"));
        assert!(!user_message_needs_collapse("1\n2\n3\n4\n5"));
        assert!(user_message_needs_collapse("1\n2\n3\n4\n5\n6"));
        assert!(
            !user_message_needs_collapse(&"x".repeat(240)),
            "ordinary two- or three-line prose must not grow a toggle"
        );
        assert!(!user_message_needs_collapse(
            &"x".repeat(USER_COLLAPSE_CHARS)
        ));
        assert!(user_message_needs_collapse(
            &"x".repeat(USER_COLLAPSE_CHARS + 1)
        ));
    }

    #[test]
    fn user_resize_duration_scales_with_distance_and_stays_bounded() {
        let short = user_resize_duration_ms(100.0);
        let medium = user_resize_duration_ms(600.0);
        let long = user_resize_duration_ms(2_000.0);
        assert!((220..=260).contains(&short));
        assert!(medium > short);
        assert_eq!(long, 850);
        assert_eq!(
            user_resize_spec(100.0).curve,
            motion::EASE_OUT,
            "short folds keep the decisive sidebar-like ease-out"
        );
        assert_eq!(
            user_resize_spec(2_000.0).curve,
            motion::EASE_IN_OUT,
            "large folds avoid front-loading the whole travel"
        );
    }

    /// The toggle is render-local: expanding a prompt must not change the
    /// row's identity or version, or the list would splice (and the
    /// virtualizer would drop the scroll anchor) on every click.
    #[test]
    fn expanding_a_prompt_is_not_a_row_change() {
        let mut entry = assistant("u3", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", &"a line\n".repeat(40))];
        let before = rows_for_entry(&entry, false, false, &mut parse);
        let after = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].id, after[0].id);
        assert_eq!(before[0].version, after[0].version);
    }

    #[test]
    fn user_rows_split_attachment_refs_from_text() {
        let content = crate::attachments::with_attachments(
            "what color is this?",
            &["/data/uploads/ab12-red.png".to_string()],
        );
        let mut entry = assistant("u2", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", &content)];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        assert_eq!(rows.len(), 1);
        let RowKind::User {
            text, attachments, ..
        } = &rows[0].kind
        else {
            panic!("expected a user row");
        };
        assert_eq!(text.as_ref(), "what color is this?");
        assert_eq!(rows[0].copy_text.as_deref(), Some("what color is this?"));
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].path, "/data/uploads/ab12-red.png");
        assert_eq!(attachments[0].name, "ab12-red.png");

        // Image-only send: no bubble text, refs parsed.
        let only = crate::attachments::with_attachments("", &["/a/p.png".to_string()]);
        entry.parts = vec![text_part("t0", &only)];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::User {
            text, attachments, ..
        } = &rows[0].kind
        else {
            panic!("expected a user row");
        };
        assert_eq!(text.as_ref(), "");
        assert!(rows[0].copy_text.is_none());
        assert_eq!(attachments.len(), 1);
    }

    /// A sent bubble shapes each chip's padding in Geist and its label, like
    /// the rest of the body, in the interface font.
    #[test]
    fn bubble_chip_padding_is_shaped_in_geist() {
        let raw = format!(
            "open {} and {}",
            openagents_proto::attachment_mentions::attachment_mention_link(1, None),
            openagents_proto::attachment_mentions::attachment_mention_link(2, Some("notes.md")),
        );
        let (text, spans) = crate::composer::sent_mention_display(&raw).expect("chips project");
        let mut theme = Theme::dark();
        theme.font_sans = "Geist Mono".into();
        let runs = user_bubble_runs(&text, &spans, &theme);
        assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), text.len());
        let mut at = 0;
        let mut pads = Vec::new();
        for run in &runs {
            let range = at..at + run.len;
            at = range.end;
            if run.font.family.as_ref() == crate::composer::CHIP_PAD_FAMILY {
                pads.push(range);
            } else {
                assert_eq!(run.font.family.as_ref(), "Geist Mono");
            }
        }
        let expected: Vec<_> = spans
            .iter()
            .flat_map(|span| crate::composer::chip_pad_ranges(&span.range))
            .collect();
        assert_eq!(pads, expected);
    }

    /// A sent prompt's file mentions render as chips in the transcript: the
    /// row carries the projected display text plus spans, while ordinary
    /// prompts keep the empty-spans fast path. The row version derives from
    /// the RAW text either way, so projection never perturbs the diff key.
    /// A chip whose upload is still crossing to the host shows its progress
    /// in place of its icon, as the strip's thumbnail did; once the bytes
    /// land (the ref is rewritten to a local path) the icon comes back.
    #[gpui::test]
    fn sent_chips_show_upload_progress_until_their_bytes_land(cx: &mut gpui::TestAppContext) {
        use openagents_proto::attachment_mentions::attachment_mention_link;
        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
        });
        let state = cx.new(|_| AppState::new());
        state.update(cx, |state, _| {
            state.apply_transfers(vec![openagents_proto::TransferProgress {
                upload_id: "att-1".into(),
                file_name: "Image 1.png".into(),
                done: 430,
                total: 1_000,
            }])
        });
        let transcript =
            cx.new(|cx| Transcript::new(crate::transcript_host::AppHost::shared(&state), cx));
        let body = format!(
            "compare {} with {} and {}",
            attachment_mention_link(1, None),
            attachment_mention_link(2, Some("notes.md")),
            attachment_mention_link(3, Some("done.md")),
        );
        let raw = crate::attachments::with_attachments(
            &body,
            &[
                "pending://att-1/Image 1.png".to_string(),
                "pending://att-2/notes.md".to_string(),
                "/uploads/ef56ab78-done.md".to_string(),
            ],
        );
        let mut entry = assistant("u5", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", &raw)];
        let rows = rows_for_entry(&entry, true, false, &mut parse);
        let RowKind::User {
            mentions,
            attachments,
            ..
        } = &rows[0].kind
        else {
            panic!("expected a user row");
        };
        assert!(attachments.is_empty());
        let mentions = mentions.clone();
        transcript.update(cx, |transcript, cx| {
            let overlays = DesktopMedia::chip_overlays(
                transcript,
                &"u5".into(),
                &mentions,
                &Theme::dark(),
                cx,
            );
            // The image (43% across) and the file still crossing show a
            // status; the landed file shows its icon.
            assert_eq!(
                overlays
                    .iter()
                    .map(|overlay| (overlay.range.clone(), overlay.status.is_some()))
                    .collect::<Vec<_>>(),
                [
                    (mentions[0].range.clone(), true),
                    (mentions[1].range.clone(), true)
                ]
            );
            assert_eq!(
                DesktopMedia::upload_status(transcript, "pending://att-1/Image 1.png", cx),
                (true, Some(43))
            );
            assert_eq!(
                DesktopMedia::upload_status(transcript, "/uploads/ef56ab78-done.md", cx),
                (false, None)
            );
        });
    }

    /// A sent message with image, file-attachment and workspace-file chips
    /// reads as chips in the bubble; only attachments without a chip stay in
    /// the strip above.
    #[test]
    fn user_bubbles_project_attachment_chips_in_place_of_the_strip() {
        use crate::composer::ChipKind;
        use openagents_proto::attachment_mentions::attachment_mention_link;
        let body = format!(
            "compare {} with {} and {}",
            attachment_mention_link(1, None),
            attachment_mention_link(2, Some("notes.md")),
            "[composer.rs](openagents-file:crates/ui/src/composer.rs)",
        );
        let raw = crate::attachments::with_attachments(
            &body,
            &[
                "/uploads/ab12cd34-Image_1.png".to_string(),
                "/uploads/ef56ab78-notes.md".to_string(),
                "/uploads/0a1b2c3d-Image_3.png".to_string(),
            ],
        );
        let mut entry = assistant("u4", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", &raw)];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::User {
            text,
            mentions,
            attachments,
            ..
        } = &rows[0].kind
        else {
            panic!("expected a user row");
        };
        assert!(!text.contains("openagents-") && !text.contains("Attached images"));
        assert_eq!(
            mentions.iter().map(|m| m.kind).collect::<Vec<_>>(),
            [ChipKind::Image, ChipKind::File, ChipKind::File]
        );
        let labels: Vec<String> = mentions
            .iter()
            .map(|m| {
                text[m.range.clone()]
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .replace('\u{a0}', " ")
            })
            .collect();
        assert_eq!(labels, ["Image 1", "notes.md", "composer.rs"]);
        // Chips stand in for their attachments: only the unchipped image is
        // left in the strip, and each attachment chip carries its upload.
        assert_eq!(
            attachments
                .iter()
                .map(|a| a.path.as_str())
                .collect::<Vec<_>>(),
            ["/uploads/0a1b2c3d-Image_3.png"]
        );
        assert_eq!(
            mentions
                .iter()
                .map(|m| m.upload.as_deref())
                .collect::<Vec<_>>(),
            [
                Some("/uploads/ab12cd34-Image_1.png"),
                Some("/uploads/ef56ab78-notes.md"),
                None
            ]
        );

        // A files-only send hides the placeholder body.
        let raw = crate::attachments::with_attachments("", &["/uploads/ef56ab78-notes.md".into()]);
        entry.parts = vec![text_part("t0", &raw)];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::User {
            text, attachments, ..
        } = &rows[0].kind
        else {
            panic!("expected a user row");
        };
        assert!(text.is_empty());
        assert_eq!(attachments.len(), 1);
    }

    #[test]
    fn user_rows_project_file_mentions_into_chips() {
        let raw = "look at [composer.rs](openagents-file:crates/ui/src/composer.rs) please";
        let mut entry = assistant("u3", MessageStatus::Complete, vec![]);
        entry.role = MessageRole::User;
        entry.status = None;
        entry.parts = vec![text_part("t0", raw)];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::User { text, mentions, .. } = &rows[0].kind else {
            panic!("expected a user row");
        };
        assert!(
            !text.contains("openagents-file:"),
            "raw link left visible: {text}"
        );
        assert!(text.contains("composer.rs"));
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].kind, crate::composer::ChipKind::File);
        assert_eq!(mentions[0].path.as_ref(), "crates/ui/src/composer.rs");
        let chip = &text[mentions[0].range.clone()];
        assert!(chip.starts_with('\u{00A0}') && chip.contains("composer.rs"));
        assert_eq!(rows[0].version, (raw.len() as u64) << 1);

        entry.parts = vec![text_part("t0", "no mentions here")];
        let rows = rows_for_entry(&entry, false, false, &mut parse);
        let RowKind::User { text, mentions, .. } = &rows[0].kind else {
            panic!("expected a user row");
        };
        assert_eq!(text.as_ref(), "no mentions here");
        assert!(mentions.is_empty());
    }

    #[test]
    fn diff_rows_appends_and_middle_edits() {
        let entry1 = assistant("m1", MessageStatus::Complete, vec![text_part("t0", "one")]);
        let entry2 = assistant("m2", MessageStatus::Complete, vec![text_part("t0", "two")]);
        let r1 = rows_for_entry(&entry1, false, false, &mut parse);
        let mut both = r1.clone();
        both.extend(rows_for_entry(&entry2, false, false, &mut parse));

        // Identical → None.
        assert!(diff_rows(&r1, &r1.clone()).is_none());
        // Append → splice at the tail.
        assert_eq!(diff_rows(&r1, &both), Some((1..1, 1)));
        // Removal from the end.
        assert_eq!(diff_rows(&both, &r1), Some((1..2, 0)));

        // Middle content change: only the changed row splices.
        let entry1b = assistant(
            "m1",
            MessageStatus::Complete,
            vec![text_part("t0", "one more")],
        );
        let mut both_b = rows_for_entry(&entry1b, false, false, &mut parse);
        both_b.extend(rows_for_entry(&entry2, false, false, &mut parse));
        assert_eq!(diff_rows(&both, &both_b), Some((0..1, 1)));

        // Full reset when everything shifts.
        let r2 = rows_for_entry(&entry2, false, false, &mut parse);
        assert_eq!(diff_rows(&r1, &r2), Some((0..1, 1)));
    }

    #[test]
    fn diff_handles_live_to_split_growth() {
        let live = assistant("m1", MessageStatus::Streaming, vec![text_part("t0", MD)]);
        let done = assistant("m1", MessageStatus::Complete, vec![text_part("t0", MD)]);
        let live_rows = rows_for_entry(&live, false, false, &mut parse);
        let done_rows = rows_for_entry(&done, false, false, &mut parse);
        // Same ids; every version flips its streaming bit → one 3-row splice.
        assert_eq!(diff_rows(&live_rows, &done_rows), Some((0..3, 3)));
    }

    #[test]
    fn tool_diff_builds_real_hunks_with_context_and_numbers() {
        use crate::changes::LineKind;
        let old = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>();
        let mut new = old.clone();
        new[9] = "LINE 10".into();
        let diff = openagents_proto::ToolDiff {
            path: "/w/a.rs".into(),
            old_text: Some(old.join("\n") + "\n"),
            new_text: new.join("\n") + "\n",
        };
        let Some(ToolDetail::Diff {
            file,
            old_text,
            new_text,
        }) = tool_detail(None, Some(&diff), None)
        else {
            panic!("expected diff detail");
        };
        // One hunk: the change plus 3 context lines each side, real numbers.
        assert_eq!(file.hunks.len(), 1);
        let hunk = &file.hunks[0];
        assert_eq!(hunk.header, "@@ -7,7 +7,7 @@");
        assert_eq!(hunk.lines.len(), 8); // 6 context + 1 del + 1 add
        let del = hunk
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Del)
            .expect("del line");
        assert_eq!(del.old_no, Some(10));
        assert_eq!(del.new_no, None);
        assert_eq!(del.text, "line 10");
        let add = hunk
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Add)
            .expect("add line");
        assert_eq!(add.new_no, Some(10));
        assert_eq!(add.text, "LINE 10");
        assert_eq!((file.additions, file.deletions), (1, 1));
        assert_eq!(old_text.as_deref(), diff.old_text.as_deref());
        assert_eq!(new_text.as_deref(), Some(diff.new_text.as_str()));
        // New files carry Added status (and no old numbers).
        let created = openagents_proto::ToolDiff {
            path: "/w/new.txt".into(),
            old_text: None,
            new_text: "only\n".into(),
        };
        let Some(ToolDetail::Diff {
            file,
            old_text,
            new_text,
        }) = tool_detail(None, Some(&created), None)
        else {
            panic!("expected diff detail");
        };
        assert_eq!(file.status, crate::changes::FileStatus::Added);
        assert!(old_text.is_none());
        assert_eq!(new_text.as_deref(), Some("only\n"));

        // Output: verbatim lines (indentation intact), counted-tail cap.
        let output = (0..40)
            .map(|i| format!("    indented {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let Some(ToolDetail::Output {
            lines,
            truncated_by,
        }) = tool_detail(Some(&output), None, None)
        else {
            panic!("expected output detail");
        };
        assert_eq!(lines.len(), OUTPUT_DETAIL_MAX_LINES);
        assert_eq!(truncated_by, 40 - OUTPUT_DETAIL_MAX_LINES);
        assert_eq!(lines[0].as_ref(), "    indented 0");

        // A minified megabyte line is cut on a char boundary before shaping,
        // in the summary and in a fetched full output alike.
        let long = format!("{}\nshort", "é".repeat(1024 * 1024));
        for detail in [
            tool_detail(Some(&long), None, None),
            blob_detail(&long, false),
        ] {
            let Some(ToolDetail::Output { lines, .. }) = detail else {
                panic!("expected output detail");
            };
            assert_eq!(lines[0].chars().count(), OUTPUT_LINE_MAX_CHARS + 1);
            assert!(lines[0].ends_with("é…"));
            assert_eq!(lines[1].as_ref(), "short");
        }

        // Nothing → no affordance.
        assert!(tool_detail(None, None, None).is_none());
        assert!(tool_detail(Some("\n\n"), None, None).is_none());
    }

    #[test]
    fn tool_group_summaries() {
        let exec = |c: &str| ToolItem {
            part_id: "fixture".into(),
            call: ToolCall::Exec { command: c.into() },
            is_error: false,
            resolved: true,
            detail: None,
            invocation: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            subagent_ref: None,
            subagent_status: None,
            subagent_tail: None,
            kind: ToolItemKind::Call,
            images: Arc::new([]),
        };
        let edit = |p: &str| ToolItem {
            part_id: "fixture".into(),
            call: ToolCall::EditFile {
                path: p.into(),
                old_string: None,
                new_string: None,
            },
            is_error: false,
            resolved: true,
            detail: None,
            invocation: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            subagent_ref: None,
            subagent_status: None,
            subagent_tail: None,
            kind: ToolItemKind::Call,
            images: Arc::new([]),
        };
        let tools = vec![
            exec("ls"),
            exec("pwd"),
            exec("make"),
            edit("a.rs"),
            edit("b.rs"),
        ];
        assert_eq!(
            tool_group_summary(&tools),
            "Ran 3 commands · edited 2 files"
        );
        // Distinct-path dedupe: editing one file twice counts once.
        let tools = vec![edit("a.rs"), edit("a.rs")];
        assert_eq!(tool_group_summary(&tools), "Edited 1 file");
        // Failures append.
        let mut failing = exec("boom");
        failing.is_error = true;
        assert_eq!(tool_group_summary(&[failing]), "Ran 1 command · 1 failed");
        // Reads / searches / misc.
        let tools = vec![
            ToolItem {
                part_id: "fixture".into(),
                call: ToolCall::ReadFile { path: "x".into() },
                is_error: false,
                resolved: true,
                detail: None,
                invocation: None,
                output_ref: None,
                output_bytes: None,
                diff_ref: None,
                subagent_ref: None,
                subagent_status: None,
                subagent_tail: None,
                kind: ToolItemKind::Call,
                images: Arc::new([]),
            },
            ToolItem {
                part_id: "fixture".into(),
                call: ToolCall::Glob {
                    pattern: "*.rs".into(),
                },
                is_error: false,
                resolved: true,
                detail: None,
                invocation: None,
                output_ref: None,
                output_bytes: None,
                diff_ref: None,
                subagent_ref: None,
                subagent_status: None,
                subagent_tail: None,
                kind: ToolItemKind::Call,
                images: Arc::new([]),
            },
            ToolItem {
                part_id: "fixture".into(),
                call: ToolCall::WebSearch { query: "q".into() },
                is_error: false,
                resolved: true,
                detail: None,
                invocation: None,
                output_ref: None,
                output_bytes: None,
                diff_ref: None,
                subagent_ref: None,
                subagent_status: None,
                subagent_tail: None,
                kind: ToolItemKind::Call,
                images: Arc::new([]),
            },
        ];
        assert_eq!(tool_group_summary(&tools), "Read 1 file · searched 2 times");
    }

    #[test]
    fn subagent_tab_titles() {
        // The tab is the BARE task — the "Agent:" genus is stripped.
        let named = ToolCall::Unknown {
            name: "Agent: scan repo".into(),
            input: None,
        };
        assert_eq!(subagent_tab_title(&named).as_ref(), "scan repo");
        // A bare "Task"/"Agent" digs the description out of the call input
        // (which sheds any genus of its own).
        let bare = ToolCall::Unknown {
            name: "Task".into(),
            input: Some(serde_json::json!({
                "description": "Agent: audit the auth flow",
                "prompt": "very long instructions…",
            })),
        };
        assert_eq!(subagent_tab_title(&bare).as_ref(), "audit the auth flow");
        // Word boundaries only — a name that merely STARTS with the genus
        // keeps itself.
        let compound = ToolCall::Unknown {
            name: "Taskmaster".into(),
            input: None,
        };
        assert_eq!(subagent_tab_title(&compound).as_ref(), "Taskmaster");
        // Nothing to derive → the generic label.
        let blank = ToolCall::Unknown {
            name: "agent".into(),
            input: None,
        };
        assert_eq!(subagent_tab_title(&blank).as_ref(), "Subagent");
        // Absurd lengths cap with an ellipsis; multiline prompts keep only
        // their first line.
        let long = ToolCall::Unknown {
            name: "x".repeat(120),
            input: None,
        };
        let title = subagent_tab_title(&long);
        assert_eq!(title.chars().count(), SUBAGENT_TITLE_MAX + 1);
        assert!(title.ends_with('…'));
        // Non-spawn-shaped calls stay generic.
        assert_eq!(
            subagent_tab_title(&ToolCall::Exec {
                command: "ls".into()
            })
            .as_ref(),
            "Subagent"
        );
    }

    #[test]
    fn tool_chip_labels_per_kind() {
        assert_eq!(
            tool_chip_content(&ToolCall::Exec {
                command: "cargo test".into()
            }),
            ("Run", "cargo test".to_string())
        );
        assert_eq!(
            tool_chip_content(&ToolCall::Search {
                pattern: "foo".into(),
                path: Some("src".into())
            }),
            ("Search", "foo in src".to_string())
        );
        assert_eq!(
            tool_chip_content(&ToolCall::ApplyPatch { path: None }),
            ("Patch", "workspace".to_string())
        );
        assert_eq!(
            tool_chip_content(&ToolCall::Mcp {
                server: "gh".into(),
                tool: "issues".into(),
                input: None
            }),
            ("MCP", "gh · issues".to_string())
        );
        let todo = ToolCall::Todo {
            items: vec![
                openagents_proto::TodoItem::new("a", openagents_proto::TodoStatus::Completed),
                openagents_proto::TodoItem::new("b", openagents_proto::TodoStatus::Pending),
            ],
        };
        assert_eq!(tool_chip_content(&todo), ("Todo", "1/2 done".to_string()));
    }

    #[test]
    fn file_action_badges_show_only_the_file_name() {
        assert_eq!(file_badge_name("/Users/me/project/src/main.rs"), "main.rs");
        assert_eq!(
            file_badge_name("crates/ui/src/transcript.rs"),
            "transcript.rs"
        );
        assert_eq!(file_badge_name(r"C:\project\src\main.rs"), "main.rs");
        assert_eq!(file_badge_name("src/components/"), "components");
        assert_eq!(file_badge_name("main.rs"), "main.rs");
        assert_eq!(file_badge_name(""), "");
    }

    #[gpui::test]
    fn tool_file_badge_opens_linkable_files_without_toggling_the_chip(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::Modifiers;

        let dir = tempfile::tempdir().unwrap();
        cx.update(|cx| {
            gpui_base::init(cx);
            cx.set_global(Theme::dark());
            crate::settings::init(crate::settings::UiSettings::default(), dir.path(), cx);
        });
        let opened: Rc<RefCell<Vec<render::LinkActivation>>> = Rc::default();
        let recorded = opened.clone();
        let opener = ToolFileOpener {
            link: render::LinkUi {
                source_session: Some("chat".into()),
                source_local: true,
                file_roots: Some(Rc::new(vec![crate::workspace_links::FileLinkRoot {
                    chat: Some("chat".into()),
                    root: "/work/app".into(),
                    local: true,
                }])),
                handler: Rc::new(move |activation, _, _| {
                    recorded.borrow_mut().push(activation.clone());
                    render::LinkOutcome::Internal
                }),
            },
        };
        assert!(opener.opens("/work/app/src/guard.rs"));
        assert!(opener.opens("src/guard.rs"));
        // An absolute file outside every root still opens, read-only.
        assert!(opener.opens("/tmp/report.md"));
        assert!(!opener.opens("/work/app/../secret.rs"));
        assert!(!opener.opens("../secret.rs"));

        let read = |path: &str| ToolItem {
            part_id: "fixture".into(),
            images: Default::default(),
            call: ToolCall::ReadFile { path: path.into() },
            is_error: false,
            resolved: true,
            detail: None,
            invocation: None,
            output_ref: None,
            output_bytes: None,
            diff_ref: None,
            subagent_ref: None,
            subagent_status: None,
            subagent_tail: None,
            kind: ToolItemKind::Call,
        };
        // The chip header inside a clickable parent, standing in for the
        // accordion toggle that wraps it in `render_tool_group`.
        struct ChipFixture {
            tool: ToolItem,
            opener: ToolFileOpener,
            toggles: Rc<Cell<usize>>,
        }
        impl Render for ChipFixture {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let theme = Theme::of(cx).clone();
                let toggles = self.toggles.clone();
                div().w(px(600.0)).child(
                    div()
                        .id("chip-toggle")
                        .debug_selector(|| "chip-toggle".into())
                        .on_click(move |_, _, _| toggles.set(toggles.get() + 1))
                        .child(chip_header(
                            &self.tool,
                            false,
                            Some(&self.opener),
                            &theme,
                            cx.entity_id(),
                            cx,
                        )),
                )
            }
        }

        let toggles = Rc::new(Cell::new(0));
        let (_inside, window) = cx.add_window_view(|_, _| ChipFixture {
            tool: read("/work/app/src/guard.rs"),
            opener: opener.clone(),
            toggles: toggles.clone(),
        });
        window.update(|window, cx| window.draw(cx).clear());
        let badge = window
            .debug_bounds("tool-file-badge")
            .expect("a workspace file badge is clickable");
        window.simulate_click(badge.center(), Modifiers::default());
        {
            let opened = opened.borrow();
            assert_eq!(opened.len(), 1);
            assert_eq!(opened[0].target.original, "/work/app/src/guard.rs");
            assert_eq!(opened[0].action, render::LinkAction::Primary);
            assert_eq!(opened[0].source_session.as_deref(), Some("chat"));
        }
        assert_eq!(
            toggles.get(),
            0,
            "opening the file must not toggle the chip"
        );
        // The rest of the header still toggles the accordion.
        let row = window.debug_bounds("chip-toggle").unwrap();
        window.simulate_click(
            gpui::point(row.left() + px(2.0), row.center().y),
            Modifiers::default(),
        );
        assert_eq!(toggles.get(), 1);
        assert_eq!(opened.borrow().len(), 1);

        let unlinkable_toggles = Rc::new(Cell::new(0));
        let (_unlinkable, window) = cx.add_window_view(|_, _| ChipFixture {
            tool: read("../secret.rs"),
            opener: opener.clone(),
            toggles: unlinkable_toggles.clone(),
        });
        window.update(|window, cx| window.draw(cx).clear());
        assert!(
            window.debug_bounds("tool-file-badge").is_none(),
            "paths the link handler cannot open keep an inert badge"
        );
    }

    #[test]
    fn multiline_command_flattens_to_one_chip_line() {
        // The user's breaker: a multi-line script in a Run chip. The detail
        // must come out as ONE sanitized line — the chip's fixed 30px card
        // then truncates it with an ellipsis like the original's CSS.
        let (label, detail) = tool_chip_content(&ToolCall::Exec {
            command: "set -e\nfixture_in_original=0\n\tgrep -c  \"x\"".into(),
        });
        assert_eq!(label, "Run");
        assert_eq!(detail, "set -e fixture_in_original=0 grep -c \"x\"");
        assert!(!detail.contains('\n'));
        // The chip row height is a constant, independent of content shape.
        assert_eq!(chips_height(1), CHIPS_TOP_PAD + CHIP_HEIGHT);
        // Every detail kind is sanitized (MCP inputs / queries are model text).
        let (_, q) = tool_chip_content(&ToolCall::WebSearch {
            query: "line one\nline two".into(),
        });
        assert_eq!(q, "line one line two");
    }

    #[test]
    fn call_block_carries_the_full_invocation() {
        // Multi-line command: verbatim lines, not the flattened chip line.
        let Some(ToolDetail::Output {
            lines,
            truncated_by,
        }) = call_block(&ToolCall::Exec {
            command: "set -e\ncargo test".into(),
        })
        else {
            panic!("expected an output block")
        };
        assert_eq!(truncated_by, 0);
        assert_eq!(
            lines.iter().map(|l| l.as_ref()).collect::<Vec<_>>(),
            vec!["set -e", "cargo test"]
        );

        // A long single-line command soft-wraps instead of ellipsizing.
        let Some(ToolDetail::Output { lines, .. }) = call_block(&ToolCall::Exec {
            command: "x".repeat(CALL_WRAP_COLS * 2 + 10),
        }) else {
            panic!("expected an output block")
        };
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.chars().count() <= CALL_WRAP_COLS));

        // MCP input pretty-prints under the `server · tool` line.
        let Some(ToolDetail::Output { lines, .. }) = call_block(&ToolCall::Mcp {
            server: "gh".into(),
            tool: "issues".into(),
            input: Some(serde_json::json!({"repo": "openagents"})),
        }) else {
            panic!("expected an output block")
        };
        assert_eq!(lines[0].as_ref(), "gh · issues");
        assert!(lines.iter().any(|l| l.contains("\"repo\": \"openagents\"")));

        // Todos list one item per line with checkbox state.
        let Some(ToolDetail::Output { lines, .. }) = call_block(&ToolCall::Todo {
            items: vec![
                openagents_proto::TodoItem::new("a", openagents_proto::TodoStatus::Completed),
                openagents_proto::TodoItem::new("b", openagents_proto::TodoStatus::InProgress),
                openagents_proto::TodoItem::new("c", openagents_proto::TodoStatus::Pending),
            ],
        }) else {
            panic!("expected an output block")
        };
        assert_eq!(
            lines.iter().map(|l| l.as_ref()).collect::<Vec<_>>(),
            vec!["[x] a", "[~] b", "[ ] c"]
        );

        // Blank invocation → no block; the chip stays a plain card.
        assert!(
            call_block(&ToolCall::Exec {
                command: "  \n ".into()
            })
            .is_none()
        );
    }

    #[test]
    fn timestamp_strip_lands_on_the_last_settled_row() {
        use chrono::FixedOffset;
        // Fixed zone (UTC−4): "Jul 1, 3:45 PM" — the exact formatTimestamp
        // shape (short month, numeric day, no leading zero, 2-digit minutes).
        let tz = FixedOffset::west_opt(4 * 3600).unwrap();
        let ms = chrono::DateTime::parse_from_rfc3339("2026-07-01T19:45:00Z")
            .unwrap()
            .timestamp_millis();
        assert_eq!(format_timestamp(ms, &tz), "Jul 1, 3:45 PM");

        // User entries carry the strip on their single row (pending too).
        let user = SessionMessageEntry {
            id: "u1".into(),
            role: MessageRole::User,
            parts: vec![text_part("p1", "hi")],
            created_at: ms,
            device_id: "dev".into(),
            status: None,
            continuation_of: None,
            duration_ms: None,
        };
        let rows = rows_for_entry(&user, true, false, &mut parse);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].timestamp, Some(ms));

        // Assistant entries: strip on the LAST row once settled…
        let done = assistant(
            "a1",
            MessageStatus::Complete,
            vec![text_part("p1", "one\n\ntwo")],
        );
        let rows = rows_for_entry(&done, false, false, &mut parse);
        assert!(rows.len() >= 2);
        assert_eq!(rows.last().unwrap().timestamp, Some(done.created_at));
        assert_eq!(
            rows.last().unwrap().copy_text.as_deref(),
            Some("one\n\ntwo")
        );
        assert!(rows[..rows.len() - 1].iter().all(|r| r.timestamp.is_none()));
        assert!(rows[..rows.len() - 1].iter().all(|r| r.copy_text.is_none()));

        // …but never mid-stream (chat-view.tsx: no hover under a moving reply).
        let live = assistant(
            "a2",
            MessageStatus::Streaming,
            vec![text_part("p1", "streaming…")],
        );
        let rows = rows_for_entry(&live, false, false, &mut parse);
        assert!(rows.iter().all(|r| r.timestamp.is_none()));
        assert!(rows.iter().all(|r| r.copy_text.is_none()));
        // Every row knows its entry (the hover group).
        assert!(rows.iter().all(|r| r.entry_id.as_ref() == live.id));
    }

    #[test]
    fn message_copy_keeps_authored_text_and_excludes_tool_traces() {
        let entry = assistant(
            "a-copy",
            MessageStatus::Complete,
            vec![
                text_part("p1", "First **paragraph**."),
                tool_part("tool", "printf hidden"),
                text_part("p2", "    indented code\n    stays indented"),
            ],
        );
        assert_eq!(
            assistant_copy_text(&entry).as_deref(),
            Some("First **paragraph**.\n\n    indented code\n    stays indented")
        );
    }

    #[test]
    fn single_line_collapses_all_whitespace_runs() {
        assert_eq!(single_line("a\nb"), "a b");
        assert_eq!(single_line("  a\t\t b \r\n c  "), "a b c");
        assert_eq!(single_line("plain"), "plain");
        assert_eq!(single_line(""), "");
        assert_eq!(single_line("\n\n"), "");
    }

    #[test]
    fn chips_height_is_analytic() {
        assert_eq!(chips_height(0), 0.0);
        assert_eq!(chips_height(1), CHIPS_TOP_PAD + CHIP_HEIGHT);
        assert_eq!(
            chips_height(3),
            CHIPS_TOP_PAD + 3.0 * CHIP_HEIGHT + 2.0 * CHIP_GAP
        );
    }

    #[test]
    fn tool_row_reveal_honors_delay_easing_and_reduced_motion() {
        let epoch = Instant::now();
        let start = epoch + Duration::from_millis(TOOL_ROW_STAGGER_MS);
        assert_eq!(tool_row_reveal_progress(Some(start), epoch, false), 0.0);
        let halfway = tool_row_reveal_progress(
            Some(start),
            start + Duration::from_millis(TOOL_ROW_REVEAL.duration_ms / 2),
            false,
        );
        assert!(halfway > 0.5 && halfway < 1.0);
        assert_eq!(
            tool_row_reveal_progress(Some(start), start + TOOL_ROW_REVEAL.total(), false,),
            1.0
        );
        assert_eq!(tool_row_reveal_progress(Some(start), epoch, true), 1.0);
        assert_eq!(tool_row_reveal_progress(None, epoch, false), 1.0);
    }

    #[test]
    fn tool_connector_draws_continuously_before_revealing_the_branch() {
        let epoch = Instant::now();
        let start = epoch + Duration::from_millis(TOOL_ROW_STAGGER_MS);
        assert_eq!(
            tool_connector_reveal_progress(Some(start), epoch, false),
            0.0
        );
        assert_eq!(
            tool_connector_reveal_progress(Some(start), epoch, true),
            1.0
        );
        assert_eq!(tool_connector_reveal_progress(None, epoch, false), 1.0);
        let halfway = tool_connector_reveal_progress(
            Some(start),
            start + Duration::from_millis(TOOL_CONNECTOR_REVEAL.duration_ms / 2),
            false,
        );
        assert!(halfway > 0.9 && halfway < 1.0);

        assert_eq!(tool_connector_parts(0.0, false), (0.0, 0.0));
        assert_eq!(tool_connector_parts(0.44, true), (0.0, 0.0));
        assert_eq!(tool_connector_continuation(Some(0.0)), 0.0);
        assert!(tool_connector_continuation(Some(0.3)) > 0.0);
        assert_eq!(tool_connector_continuation(Some(0.45)), 1.0);

        let (incoming, branch) = tool_connector_parts(0.60, true);
        assert!(incoming > 0.0 && incoming < 1.0);
        assert_eq!(branch, 0.0);
        assert_eq!(tool_connector_parts(1.0, true), (1.0, 1.0));
        assert_eq!(tool_connector_continuation(None), 0.0);
    }

    #[test]
    fn tool_branch_reveal_tracks_distance_through_the_bend() {
        let length = |points: &[Point<f32>]| -> f32 {
            points
                .windows(2)
                .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
                .sum()
        };
        let full = activity_branch_points(1.0);
        assert_eq!(activity_branch_points(0.0), vec![point(0.0, 0.0)]);
        let end = full.last().unwrap();
        assert!((end.x - (ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X)).abs() < 0.0001);
        assert!((end.y - ACTIVITY_BEND_RADIUS).abs() < 0.0001);
        for progress in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let partial = activity_branch_points(progress);
            assert!((length(&partial) / length(&full) - progress).abs() < 0.0001);
            assert!(
                partial
                    .windows(2)
                    .all(|p| p[1].x >= p[0].x && p[1].y >= p[0].y)
            );
        }
    }

    #[test]
    fn tool_title_shimmer_crosses_the_title_without_a_loop_seam() {
        assert_eq!(tool_title_shimmer_amount(0.5, 0.5), 1.0);
        assert_eq!(tool_title_shimmer_amount(0.0, 0.5), 0.0);
        assert_eq!(tool_title_shimmer_amount(1.0, 0.5), 0.0);
        assert!(tool_title_shimmer_amount(0.3, 0.5) > 0.4);
        assert!(tool_title_shimmer_amount(0.7, 0.5) > 0.4);
        for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(
                tool_title_shimmer_amount(x, 0.0),
                tool_title_shimmer_amount(x, 1.0),
                "the repeating background must meet itself at x={x}"
            );
        }

        let start = Instant::now();
        assert_eq!(tool_title_shimmer_phase(start, start), 0.0);
        let halfway = start + TOOL_GROUP_SHIMMER_DURATION / 2;
        assert!((tool_title_shimmer_phase(start, halfway) - 0.5).abs() < f32::EPSILON);
        assert_eq!(
            tool_title_shimmer_phase(start, start + TOOL_GROUP_SHIMMER_DURATION),
            0.0
        );
    }

    #[test]
    fn elapsed_format_scales_from_seconds_to_days() {
        for (secs, expected) in [
            (-5, "0s"),
            (0, "0s"),
            (59, "59s"),
            (60, "1m 0s"),
            (92, "1m 32s"),
            (310, "5m 10s"),
            (3_599, "59m 59s"),
            (3_600, "1h 0m"),
            (4_800, "1h 20m"),
            (6_000, "1h 40m"),
            (86_399, "23h 59m"),
            (86_400, "1d 0h"),
            (183_845, "2d 3h"),
        ] {
            assert_eq!(format_elapsed(secs), expected, "elapsed seconds: {secs}");
        }
    }

    /// #12204: an account chat answered on another device showed
    /// "Working… 20745d 20h": its messages carry no time, read as 0, and the
    /// timer counted from 1970. An unknown start shows no timer, and neither
    /// does a start from before the chat existed.
    #[test]
    fn turn_timer_never_counts_from_before_the_chat() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-19T20:00:00Z")
            .unwrap()
            .timestamp_millis();
        let created = now - 3_600_000;
        assert_eq!(turn_elapsed_secs(Some(0), None, now), None);
        assert_eq!(turn_elapsed_secs(Some(-5), None, now), None);
        assert_eq!(turn_elapsed_secs(None, Some(created), now), None);
        assert_eq!(
            turn_elapsed_secs(Some(created - 1), Some(created), now),
            None
        );
        assert_eq!(turn_elapsed_secs(Some(now - 95_000), None, now), Some(95));
        assert_eq!(
            turn_elapsed_secs(Some(created), Some(created), now),
            Some(3_600)
        );
        // A start a moment ahead of this clock reads as 0s, not negative.
        assert_eq!(
            turn_elapsed_secs(Some(now + 2_000), Some(created), now),
            Some(0)
        );
    }

    #[test]
    fn sending_bridge_holds_until_the_turn_outdates_the_send() {
        let send = chrono::DateTime::parse_from_rfc3339("2026-08-13T10:00:00Z")
            .unwrap()
            .to_utc();
        let before = send - chrono::Duration::seconds(90);
        let after = send + chrono::Duration::seconds(2);
        // In flight, row still on the previous turn (or no row yet).
        assert!(sending_bridge(Some(send), Some(before)));
        assert!(sending_bridge(Some(send), None));
        // The turn started after the send fired — timer takes over.
        assert!(!sending_bridge(Some(send), Some(after)));
        // No send in flight: never a bridge, whatever the row says.
        assert!(!sending_bridge(None, Some(before)));
        assert!(!sending_bridge(None, None));
    }

    #[test]
    fn empty_text_parts_produce_no_rows() {
        let entry = assistant(
            "m9",
            MessageStatus::Streaming,
            vec![text_part("t0", ""), text_part("t1", "   ")],
        );
        assert!(rows_for_entry(&entry, false, false, &mut parse).is_empty());
    }
}

#[cfg(feature = "appshots-fixture")]
impl Transcript {
    pub fn fixture_appshots_start(&mut self, cx: &mut Context<Self>) {
        self.list.scroll_to(gpui::ListOffset::default());
        cx.notify();
    }

    /// Open or close every tool group and image chip, as clicks would.
    pub fn fixture_tool_images(&mut self, open: bool, cx: &mut Context<Self>) {
        let now = Instant::now();
        for row in &self.rows {
            let RowKind::ToolGroup { tools, .. } = &row.kind else {
                continue;
            };
            let fold = self.folds.entry(row.id.clone()).or_default();
            fold.open = Some(open);
            for (ix, tool) in tools.iter().enumerate() {
                if chip_has_images(tool) {
                    let chip = self
                        .tool_details
                        .entry(SharedString::from(format!("{}#d{ix}", row.id)))
                        .or_default();
                    chip.open = Some(open);
                    chip.epoch += 1;
                    chip.toggled_at = Some(now);
                }
            }
        }
        self.list.remeasure();
        cx.notify();
    }

    /// `(previews resident, retained bytes)`.
    pub fn fixture_tool_image_stats(&self) -> (usize, usize) {
        self.tool_images.stats()
    }

    pub fn fixture_image_menu(
        &mut self,
        path: String,
        action: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let devices = self.image_host(cx).0.into_iter().collect();
        self.open_image_menu(path, devices, gpui::point(px(420.), px(260.)), window, cx);
        if let Some(action) = action {
            self.dispatch_image_menu(action, cx);
        }
    }
}
