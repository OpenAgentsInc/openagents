//! Conversation handoff and project selection shared by phone and desktop.
use crate::basic_coder::Turn;

pub const MAX_PROMPT_BYTES: usize = 16 * 1024;

pub fn project(listed: &[String], used: impl Fn(&str) -> Option<u64>) -> Option<String> {
    listed
        .iter()
        .filter_map(|label| Some((used(label)?, label)))
        .max()
        .map(|(_, label)| label)
        .or_else(|| listed.iter().find(|label| *label == "openagents"))
        .or_else(|| listed.first())
        .cloned()
}

/// The prompt a Coder run starts with: the message that asked for the
/// work, titled by it, then how the run was started ([`routing`], for the
/// engine the reply's offer names), then bounded context
/// ([`crate::basic_chats::handoff_routed`]). `chat_title` titles it only
/// when the conversation has no user turn. Every surface that starts Coder
/// from a conversation (the desktop's local run, the host's handoff and
/// `thread.run`, the phone through it, and the CLI) builds it here.
pub fn prompt(chat_title: &str, turns: &[Turn]) -> String {
    let routing = routing(requested(turns));
    crate::basic_chats::handoff_routed(
        &title(chat_title, turns),
        turns,
        MAX_PROMPT_BYTES,
        Some(&routing),
    )
}

/// What a handoff tells the engine about how its run was started (#10084).
/// The person's message asked OpenAgents to delegate, and maybe to one
/// engine (`requested`, the offer's typed engine, never read from text);
/// by the time an engine reads this, that request is done. So the engine
/// is told the routing is settled, to do the work the message names (a
/// "test delegation" that also says "clone grok-build to ~" clones it,
/// owner 2026-10-02), never to start another coding engine's command line,
/// and that only a message with no task beyond the handoff itself gets a
/// small, harmless check of the project. It is told to report the work,
/// not the handoff: Grok Build answered "The delegation ran here…". Which engine a
/// message names is the router's typed judgment; whether it asks for more
/// than the delegation is the engine's own reading of it.
#[must_use]
pub fn routing(requested: Option<nostr::cj_conversation::Engine>) -> String {
    let asked = match requested {
        Some(engine) => {
            let name = engine.name();
            format!(
                "The person asked for this to run on {name}. OpenAgents has already started \
                 this run on the engine it chose, so that request is done: if you are {name}, \
                 you are the engine they asked for; if you are another engine, {name} could not \
                 run on this computer now (it is not signed in here, it is unavailable right now, \
                 or this computer's Coder settings do not allow it), and OpenAgents has already \
                 told the person why and which engine runs instead. "
            )
        }
        None => "The person asked OpenAgents to hand this conversation to Coder. OpenAgents has \
                 already started this run on the engine it chose, so that request is done. "
            .to_owned(),
    };
    format!("{asked}{DO_THE_WORK}")
}

/// What every handoff asks of the engine, after how the run started.
/// Delegating, to one engine or several, is OpenAgents' job and is done:
/// the engine never refuses the person's request, or reports it could
/// not delegate, because it does not start other engines itself (#10183:
/// Codex answered "I couldn't perform the three agent delegations" and
/// gave that as the reason).
const DO_THE_WORK: &str = "Do what the person's message asks, here, with your own commands: \
when it names work, such as cloning a repository, running something, exploring the code, or \
changing files, do that work. Handing work to coding engines, one or several, is OpenAgents' \
job, and it is already done: if the message asks for delegations or for several agents, this \
run is the share OpenAgents gave you, so do the work it names yourself and never say you \
could not delegate. You never start another coding engine's command line (such as `claude`, \
`codex`, `devin`, `opencode`, or `grok`) as a sub-process, and you never ask the person for \
another engine's login. Only when the message asks for nothing but the handoff itself, such \
as a bare test delegation, is the task a small, harmless check of this project: look at what \
it holds, change nothing, and tell the person in a few sentences what you found. Tell the \
person about the work, not about how this run was started.";

/// The prompt one run of a dispatch plan starts with (#10183): the same
/// handoff as [`prompt`], told that OpenAgents started one run on each of
/// `plan.runs` for the message and that this one is `engine`'s, and, for
/// a read-only plan, that the run reads and changes nothing (Coder's
/// boundary enforces it whatever the run does).
#[must_use]
pub fn plan_prompt(
    chat_title: &str,
    turns: &[Turn],
    plan: &nostr::cj_conversation::Plan,
    engine: nostr::cj_conversation::Engine,
) -> String {
    let names: Vec<&str> = plan.runs.iter().map(|engine| engine.name()).collect();
    let mut routing = format!(
        "The person asked for this work to go to several coding engines. OpenAgents has \
         already started {} runs of it, one each on {}, and this run is the one on {}; the \
         others run beside it and report to the person themselves, and OpenAgents puts their \
         results together. They are the other delegations the person asked for, so do this \
         run's share yourself, in one pass: never split it into subagents or delegations of \
         your own. ",
        plan.runs.len(),
        names.join(", "),
        engine.name()
    );
    if plan.read_only {
        routing.push_str(
            "This run is read-only: its worktree and Git are sealed against writes, so read, \
             explore, and run commands that only look; change, create, delete, and commit \
             nothing. ",
        );
    }
    routing.push_str(DO_THE_WORK);
    routing.push_str(" Report what you found in a few short paragraphs.");
    crate::basic_chats::handoff_routed(
        &title(chat_title, turns),
        turns,
        MAX_PROMPT_BYTES,
        Some(&routing),
    )
}

/// The dispatch plan for a Coder start from `turns` (#10183): the typed
/// plan on the latest reply's `run_coder` offer
/// ([`crate::router::Meta::plan`]) when it names several runs, else none.
/// Never read from text.
#[must_use]
pub fn plan(turns: &[Turn]) -> Option<nostr::cj_conversation::Plan> {
    turns
        .iter()
        .rev()
        .find(|turn| turn.role == crate::basic_coder::Role::Assistant)
        .and_then(|turn| turn.meta.as_ref())
        .and_then(|meta| meta.plan.clone())
        .filter(|plan| !plan.is_single())
}

/// The task's title for a Coder run started from a conversation: the
/// message that asked for the work, not the chat's title, which is its
/// first message (#10073).
pub fn title(chat_title: &str, turns: &[Turn]) -> String {
    crate::basic_chats::handoff_title(chat_title, turns)
}

/// Whether a reply offers Coder. The router's offer selects presentation
/// only; the host admits execution.
///
/// Precedence when one reply carries several things (#10073): an explicit
/// [`Offer::RunCoder`](crate::router::Offer::RunCoder) offers Coder; else a
/// typed offer or card for another action (a Gym test, a result, a deck,
/// a screen other than Computers, a command) is what the router chose, and
/// the reply does not
/// also offer Coder, even when the worker judged the thread's lane a
/// computer's; else the computer lane offers it, but only for a reply that
/// defers to Coder ([`defers`]). So one message never yields both a Gym
/// offer and a Coder start, and a reply that answered the question (the
/// working directory from the surface's context, #10079) never starts
/// Coder.
pub fn offered(meta: Option<&crate::router::Meta>, computer_lane: bool) -> bool {
    use crate::router::{Offer, Screen};
    if meta.is_some_and(|meta| meta.offers.contains(&Offer::RunCoder)) {
        return true;
    }
    // A step of making a plugin (#10177) starts Coder only with its own Run
    // Coder offer; every other step runs here, whatever the route read.
    if meta.is_some_and(|meta| meta.plugin.is_some()) {
        return false;
    }
    // Opening Computers is the dispatch's own "Connect a computer" offer,
    // part of offering Coder, not another action.
    let other = meta.is_some_and(|meta| {
        !meta.cards.is_empty()
            || meta.offers.iter().any(|offer| {
                !matches!(
                    offer,
                    Offer::OpenScreen {
                        screen: Screen::Computers
                    }
                )
            })
    });
    computer_lane && !other && defers(meta)
}

/// The router's typed route for work a computer does (`work.dispatch`).
pub const DISPATCH_ROUTE: &str = "work.dispatch";

/// Whether a reply on the computer lane hands the message to Coder rather
/// than answering it (#10079), read only from the worker's typed judgment,
/// never from text: its route is [`DISPATCH_ROUTE`]. A reply on another
/// route (a `meta` answer from the surface's context, such as the working
/// directory; a knowledge, CLI, or account answer; smalltalk) answered the
/// question, whatever the thread's lane, and a reply with no judgment has
/// no reading that it defers. The worker's own dispatches off that route
/// carry a `run_coder` offer, which [`offered`] reads first.
#[must_use]
pub fn defers(meta: Option<&crate::router::Meta>) -> bool {
    meta.is_some_and(|meta| meta.route.as_deref() == Some(DISPATCH_ROUTE))
}

/// Put this computer's prediction of who runs Coder on the reply that
/// offers it: the last turn, when it is a reply that offers Coder
/// ([`offered`]). `predict` is asked only
/// then, so a thread without the offer reads no login state. An earlier
/// prediction is replaced; with none, it is cleared.
///
/// `predict` gets the engine the reply's offer says the person asked for
/// ([`requested`]), so the prediction puts it first (#10076).
pub fn attach_runner(
    turns: &mut [Turn],
    computer_lane: bool,
    predict: impl FnOnce(Option<nostr::cj_conversation::Engine>) -> Option<crate::coder_events::Runner>,
) {
    let engine = requested(turns);
    let Some(last) = turns.last_mut() else {
        return;
    };
    if last.role != crate::basic_coder::Role::Assistant || last.stopped {
        return;
    }
    if !offered(last.meta.as_ref(), computer_lane) {
        return;
    }
    let runner = predict(engine);
    if runner.is_none() && last.meta.is_none() {
        return;
    }
    last.meta.get_or_insert_with(Default::default).runner = runner;
}

/// The coding engine the person asked for, for a Coder run started from
/// `turns` (#10076): the typed engine on the latest reply's `run_coder`
/// offer ([`crate::router::Meta::engine`]), else none. Never read from
/// text.
#[must_use]
pub fn requested(turns: &[Turn]) -> Option<nostr::cj_conversation::Engine> {
    turns
        .iter()
        .rev()
        .find(|turn| turn.role == crate::basic_coder::Role::Assistant)
        .and_then(|turn| turn.meta.as_ref())
        .and_then(|meta| meta.engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_reply_offering_coder_carries_the_prediction() {
        use crate::coder_events::Runner;
        use crate::router::{Meta, Offer};
        let offering = Some(Meta {
            offers: vec![Offer::RunCoder],
            ..Meta::default()
        });
        let mut turns = vec![
            Turn::user("fix the parser"),
            Turn::assistant("Coder can do that.", offering),
        ];
        attach_runner(&mut turns, false, |_| {
            Some(Runner::NotSignedIn { providers: vec![] })
        });
        assert_eq!(
            turns[1].meta.as_ref().unwrap().runner,
            Some(Runner::NotSignedIn { providers: vec![] })
        );
        let mut plain = vec![Turn::user("hi"), Turn::assistant("Hello.", None)];
        attach_runner(&mut plain, false, |_| panic!("no offer, no prediction"));
        assert!(plain[1].meta.is_none());
        // A reply with no judgment does not defer to Coder, even on the
        // computer lane (#10079).
        attach_runner(&mut plain, true, |_| panic!("an answer, no prediction"));
        assert!(plain[1].meta.is_none());
        // The computer lane makes a dispatch-routed reply an offer.
        let mut routed = vec![
            Turn::user("run the tests"),
            Turn::assistant("On it.", Some(dispatch_route())),
        ];
        attach_runner(&mut routed, true, |_| {
            Some(Runner::NotSignedIn { providers: vec![] })
        });
        assert!(routed[1].meta.as_ref().unwrap().runner.is_some());
    }

    /// The prediction is asked for the engine the reply's offer names,
    /// and a reply without one asks for none (#10076).
    #[test]
    fn the_prediction_puts_the_requested_engine_first() {
        use crate::router::{Meta, Offer};
        use nostr::cj_conversation::Engine;
        let mut turns = vec![
            Turn::user("Do a test delegation to claude"),
            Turn::assistant(
                "Starting Claude Code on this.",
                Some(Meta {
                    offers: vec![Offer::RunCoder],
                    engine: Some(Engine::ClaudeCode),
                    ..Meta::default()
                }),
            ),
        ];
        assert_eq!(requested(&turns), Some(Engine::ClaudeCode));
        let mut asked = None;
        attach_runner(&mut turns, false, |engine| {
            asked = Some(engine);
            None
        });
        assert_eq!(asked, Some(Some(Engine::ClaudeCode)));
        let plain = vec![
            Turn::user("delegate this"),
            Turn::assistant(
                "Working on this.",
                Some(Meta {
                    offers: vec![Offer::RunCoder],
                    ..Meta::default()
                }),
            ),
        ];
        assert_eq!(requested(&plain), None);
        assert_eq!(requested(&[Turn::user("hi")]), None);
    }
    /// One message never yields both a Gym offer and a Coder start
    /// (#10073): a reply carrying the router's Gym card and `start_eval`
    /// offers no Coder, even on a computer lane; an explicit Run Coder
    /// offer still does, and a plain reply on a computer lane does.
    #[test]
    fn another_typed_action_outranks_the_computer_lane() {
        use crate::router::{Meta, Offer};
        let gym = Meta {
            offers: vec![Offer::StartEval {
                body: serde_json::json!({"offer": "start_eval"}),
            }],
            cards: vec![serde_json::json!({"card": "tool"})],
            ..Meta::default()
        };
        assert!(!offered(Some(&gym), true));
        let card_only = Meta {
            cards: vec![serde_json::json!({"card": "tool"})],
            ..Meta::default()
        };
        assert!(!offered(Some(&card_only), true));
        let deck = Meta {
            offers: vec![Offer::OpenPresentation { deck: "d".into() }],
            ..Meta::default()
        };
        assert!(!offered(Some(&deck), true));
        let both = Meta {
            offers: vec![
                Offer::StartEval {
                    body: serde_json::json!({"offer": "start_eval"}),
                },
                Offer::RunCoder,
            ],
            ..Meta::default()
        };
        assert!(offered(Some(&both), false));
        // With no computer, the dispatch offers Connect a computer: still
        // the Coder offer on a computer lane.
        let connect = Meta {
            offers: vec![Offer::OpenScreen {
                screen: crate::router::Screen::Computers,
            }],
            ..dispatch_route()
        };
        assert!(offered(Some(&connect), true));
        assert!(offered(Some(&dispatch_route()), true));
        assert!(!offered(Some(&dispatch_route()), false));
        assert!(!offered(None, false));
    }

    fn dispatch_route() -> crate::router::Meta {
        crate::router::Meta {
            route: Some(DISPATCH_ROUTE.into()),
            ..crate::router::Meta::default()
        }
    }

    /// A reply that answered the question never starts Coder, even when
    /// the worker judged the thread's lane a computer's (#10079): the
    /// working directory answered from the desktop's context (route
    /// `meta`, a canned answer), an account answer with its Connect a
    /// computer screen, a model answer on a knowledge route, and a reply
    /// with no judgment. A dispatch-routed reply on the computer lane, or
    /// any `run_coder` offer, still does.
    #[test]
    fn a_reply_that_answered_does_not_start_coder() {
        use crate::router::{Meta, Offer, Screen};
        let working_directory = Meta {
            tier: Some("canned".into()),
            answer: Some("meta.limits_chat.here@1".into()),
            route: Some("meta".into()),
            ..Meta::default()
        };
        assert!(!defers(Some(&working_directory)));
        assert!(!offered(Some(&working_directory), true));
        let account = Meta {
            route: Some("account".into()),
            offers: vec![Offer::OpenScreen {
                screen: Screen::Computers,
            }],
            ..Meta::default()
        };
        assert!(!offered(Some(&account), true));
        let knowledge = Meta {
            tier: Some("model".into()),
            route: Some("codebase.kb".into()),
            ..Meta::default()
        };
        assert!(!offered(Some(&knowledge), true));
        assert!(!offered(Some(&Meta::default()), true));
        assert!(!offered(None, true));
        // Work: the dispatch route on the computer lane, or the offer.
        let work = Meta {
            tier: Some("model".into()),
            ..dispatch_route()
        };
        assert!(defers(Some(&work)));
        assert!(offered(Some(&work), true));
        let offer = Meta {
            route: Some("general".into()),
            offers: vec![Offer::RunCoder],
            ..Meta::default()
        };
        assert!(offered(Some(&offer), false));
        // A plugin step on the work route runs here, not in Coder; only
        // its own Run Coder offer starts Coder (#10177).
        let running_tests = Meta {
            plugin: Some(crate::plugin_flow::Flow::at(
                crate::plugin_flow::Step::Run,
                Some("hello".into()),
            )),
            ..dispatch_route()
        };
        assert!(!offered(Some(&running_tests), true));
        let drafting = Meta {
            offers: vec![Offer::RunCoder],
            ..running_tests
        };
        assert!(offered(Some(&drafting), true));
    }

    /// The owner's gate run on 2026-10-01 (#10084): "do a test delegation
    /// to claude" started Coder on Claude Code with the message as its only
    /// task, so the engine ran the `claude` command line in its sandbox,
    /// hit its login, and waited on a credentials question. The prompt now
    /// says the routing is done and what the task is, for the engine asked
    /// for and for none, and still reads as the person's message.
    #[test]
    fn the_prompt_says_the_routing_is_done_and_what_the_task_is() {
        use crate::router::{Meta, Offer};
        use nostr::cj_conversation::Engine;
        let offer = |engine| {
            Some(Meta {
                offers: vec![Offer::RunCoder],
                engine,
                ..Meta::default()
            })
        };
        let claude = vec![
            Turn::user("do a test delegation to claude"),
            Turn::assistant(
                "Starting Claude Code on this.",
                offer(Some(Engine::ClaudeCode)),
            ),
        ];
        let text = prompt("Chat", &claude);
        assert!(
            text.starts_with("do a test delegation to claude\n\n"),
            "{text}"
        );
        let request = text
            .find("The request:\n\ndo a test delegation to claude")
            .unwrap();
        let routing_at = text.find("How this run started:").unwrap();
        assert!(request < routing_at, "{text}");
        for needle in [
            "The person asked for this to run on Claude Code.",
            "that request is done",
            "if you are Claude Code, you are the engine they asked for",
            "Claude Code could not run on this computer now",
            "which engine runs instead",
            "Do what the person's message asks",
            "such as cloning a repository",
            "do that work",
            "never start another coding engine's command line",
            "`claude`",
            "never ask the person for another engine's login",
            "Only when the message asks for nothing but the handoff",
            "a small, harmless check of this project",
            "change nothing",
            "not about how this run was started",
        ] {
            assert!(text.contains(needle), "{needle:?} missing from {text}");
        }
        for leaked in ["This run is the delegation", "Your job is"] {
            assert!(!text.contains(leaked), "{leaked:?} in {text}");
        }
        assert_eq!(
            crate::basic_chats::handoff_request(&text).as_deref(),
            Some("do a test delegation to claude")
        );
        // Codex, asked for by name, is named the same way.
        let codex = vec![
            Turn::user("have codex fix the parser"),
            Turn::assistant("Working on this.", offer(Some(Engine::Codex))),
        ];
        assert!(prompt("Chat", &codex).contains("if you are Codex, you are the engine"));
        // No engine named: the routing is still done, and no engine is
        // named as asked for.
        let now = vec![
            Turn::user("who are you"),
            Turn::assistant("We are OpenAgents.", None),
            Turn::user("do a test delegation now"),
            Turn::assistant("Working on this.", offer(None)),
        ];
        let text = prompt("who are you", &now);
        assert!(text.contains("hand this conversation to Coder"), "{text}");
        assert!(text.contains("that request is done"), "{text}");
        assert!(!text.contains("asked for this to run on"), "{text}");
        assert!(text.contains("a small, harmless check of this project"));
        // The routing comes before the context, which is still carried.
        let routing_at = text.find("How this run started:").unwrap();
        let context = text.find("User: who are you").unwrap();
        assert!(routing_at < context, "{text}");
        assert_eq!(
            crate::basic_chats::handoff_request(&text).as_deref(),
            Some("do a test delegation now")
        );
        // A request too long to fit is cut; the routing is kept whole.
        let long = vec![
            Turn::user("y".repeat(MAX_PROMPT_BYTES * 2)),
            Turn::assistant("Working on this.", offer(Some(Engine::ClaudeCode))),
        ];
        let text = prompt("Chat", &long);
        assert!(text.len() <= MAX_PROMPT_BYTES, "{}", text.len());
        assert!(
            text.ends_with(&routing(Some(Engine::ClaudeCode))),
            "routing kept whole"
        );
    }

    #[test]
    fn the_prompt_and_title_name_the_request() {
        let turns = vec![
            Turn::user("who are you"),
            Turn::assistant("We are OpenAgents.", None),
            Turn::user("do a test delegation now"),
        ];
        assert_eq!(title("who are you", &turns), "do a test delegation now");
        assert!(prompt("who are you", &turns).starts_with("do a test delegation now\n"));
    }

    #[test]
    fn project_prefers_last_used_then_openagents_then_first() {
        let listed = vec!["first".into(), "openagents".into(), "last".into()];
        assert_eq!(project(&listed, |_| None).as_deref(), Some("openagents"));
        assert_eq!(
            project(&listed, |label| (label == "last").then_some(10)).as_deref(),
            Some("last")
        );
        assert_eq!(project(&listed[..1], |_| None).as_deref(), Some("first"));
        assert!(project(&[], |_| None).is_none());
    }
}
