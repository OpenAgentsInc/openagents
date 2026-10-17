//! Move plans against the fake Computers server from #11932's tests, whose
//! commands really run in `sh` under a temporary home.
use super::*;
use crate::sessions::move_cloud::tests::{CHAT, GIT, SESSION, World, chat, logins, sh, world};
use chat_home::{Role, Seed};
use openagents_proto::HarnessId;
use openagents_sdk::computers::RetryPolicy;
use std::collections::BTreeMap;

const CHATS: [&str; 3] = [
    CHAT,
    "0b5c6c3e-4a0e-4c39-9a51-5a2d8f6e1c02",
    "0b5c6c3e-4a0e-4c39-9a51-5a2d8f6e1c03",
];

#[derive(Default)]
struct Hooks {
    paused: Mutex<Vec<String>>,
    /// Continuations by key; a key always names the same one.
    continued: Mutex<BTreeMap<String, String>>,
    /// Continuations applied whose reply is then lost.
    lose_continue: Mutex<usize>,
    released: Mutex<Vec<String>>,
    /// Continue-here messages by key: the chat and the branch.
    here: Mutex<BTreeMap<String, (String, Option<String>)>>,
}

impl MoveHooks for Hooks {
    async fn pause(&self, chat: &str, _wait: Duration) -> Result<bool, String> {
        self.paused.lock().unwrap().push(chat.into());
        Ok(true)
    }
    async fn continue_on(
        &self,
        key: &str,
        chat: &str,
        _computer: &str,
        _command: &str,
    ) -> Result<String, String> {
        let id = self
            .continued
            .lock()
            .unwrap()
            .entry(key.into())
            .or_insert_with(|| format!("continued-{chat}"))
            .clone();
        let mut lose = self.lose_continue.lock().unwrap();
        if *lose > 0 {
            *lose -= 1;
            return Err("We couldn't reach OpenAgents.".into());
        }
        Ok(id)
    }
    async fn release(&self, chat: &str) -> Result<(), String> {
        self.released.lock().unwrap().push(chat.into());
        Ok(())
    }
    async fn continue_here(
        &self,
        key: &str,
        chat: &str,
        branch: Option<&str>,
    ) -> Result<(), String> {
        self.here
            .lock()
            .unwrap()
            .entry(key.into())
            .or_insert_with(|| (chat.into(), branch.map(str::to_owned)));
        Ok(())
    }
}

/// Three chats in three projects, each with its own Claude Code session.
fn three_chats(w: &World) -> Vec<PlanInput> {
    let work = w.project.parent().unwrap().to_path_buf();
    let base = work.parent().unwrap().to_path_buf();
    let mut out = vec![PlanInput {
        chat: chat(w, HarnessId::ClaudeCode),
        title: "Fix the bug".into(),
    }];
    for (i, id) in CHATS.iter().enumerate().skip(1) {
        let name = format!("p{i}");
        let project = work.join(&name);
        std::fs::create_dir_all(&project).unwrap();
        sh(
            &project,
            &format!(
                "git init -q -b main && printf 'p{i}\\n' > a.txt && {GIT} add -A && {GIT} commit -q -m first && printf 'wip\\n' > b.txt"
            ),
        );
        let session = format!("5f1d2c3b-0000-4000-8000-00000000000{i}");
        let dir = base.join(format!("claude/projects/-work-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{session}.jsonl")), b"{}\n").unwrap();
        out.push(PlanInput {
            chat: ChatToMove {
                chat: (*id).into(),
                harness: HarnessId::ClaudeCode,
                project,
                session,
                seed: Seed::new(&format!("Task {i}"), [(Role::User, "Keep going")]),
            },
            title: format!("Task {i}"),
        });
    }
    out
}

struct Run<'a> {
    plans: PlanStore,
    computers: Computers<'a>,
    logins: CloudLogins,
    hooks: Hooks,
}

impl<'a> Run<'a> {
    fn new(w: &'a World) -> Self {
        Self {
            plans: PlanStore::open(w._tmp.path().join("plans")).unwrap(),
            computers: w
                .client
                .computers(&w.session)
                .unwrap()
                .with_retries(RetryPolicy::none())
                .unwrap(),
            logins: logins(),
            hooks: Hooks::default(),
        }
    }
    /// A fresh planner each time, as after a restart.
    fn planner(&'a self, w: &'a World) -> Planner<'a, Hooks> {
        Planner {
            plans: &self.plans,
            moves: &w.store,
            computers: &self.computers,
            logins: &self.logins,
            roots: &w.roots,
            hooks: &self.hooks,
            cancel: CancellationToken::new(),
            pause_wait: Duration::from_millis(1),
        }
    }
}

fn states(plan: &MovePlan) -> Vec<MoveState> {
    plan.chats.iter().map(|c| c.state).collect()
}

#[test]
fn reasons_are_plain_words() {
    for reason in all_reasons() {
        assert!(oa_copy::violations(&reason, &[]).is_empty(), "{reason}");
    }
    assert!(oa_copy::violations(STEER_PROMPT, &[]).is_empty());
    for state in [
        MoveState::Planned,
        MoveState::Pausing,
        MoveState::Captured,
        MoveState::Provisioned,
        MoveState::Restored,
        MoveState::Continued,
        MoveState::Released,
        MoveState::StaysHere,
        MoveState::BroughtBack,
    ] {
        assert!(oa_copy::violations(state.label(), &[]).is_empty());
    }
}

#[tokio::test]
async fn three_chats_move_at_once_to_one_new_computer() {
    let w = world(0).await;
    let run = Run::new(&w);
    let planner = run.planner(&w);
    let plan = planner
        .plan(three_chats(&w), None, Requester::Agent, 1_760_000_000)
        .await
        .unwrap();
    assert_eq!(states(&plan), vec![MoveState::Planned; 3]);
    assert_eq!(plan.estimate_minutes, estimate_minutes(3));
    assert!(plan.view().needs_approval());
    // Every chat has its own record before anything is sent.
    for id in CHATS {
        assert!(w.store.load(id).unwrap().is_some());
    }
    // The same chats can't be in two plans.
    assert!(matches!(
        planner
            .plan(three_chats(&w), None, Requester::Person, 1_760_000_001)
            .await,
        Err(PlanError::AlreadyMoving)
    ));
    // Nothing moves before the person approves.
    let seen = w.fake.book.lock().unwrap().seen.len();
    assert!(matches!(
        planner.start(&plan.id).await,
        Err(PlanError::NotApproved)
    ));
    assert_eq!(w.fake.book.lock().unwrap().seen.len(), seen);
    assert!(run.hooks.paused.lock().unwrap().is_empty());

    planner.approve(&plan.id, 1_760_000_002).unwrap();
    let done = planner.start(&plan.id).await.unwrap();
    assert_eq!(
        states(&done),
        vec![MoveState::Released; 3],
        "{:?}",
        done.chats.iter().map(|c| &c.reason).collect::<Vec<_>>()
    );
    assert!(!done.view().needs_approval());
    let book = w.fake.book.lock().unwrap();
    assert_eq!(book.created.len(), 1, "one computer for all three");
    assert_eq!(book.runs, 9, "probe, restore, and resume for each chat");
    drop(book);
    assert_eq!(run.hooks.paused.lock().unwrap().len(), 3);
    assert_eq!(run.hooks.continued.lock().unwrap().len(), 3);
    assert_eq!(run.hooks.released.lock().unwrap().len(), 3);
    for c in &done.chats {
        assert_eq!(
            c.continuation.as_deref(),
            Some(&*format!("continued-{}", c.chat))
        );
    }
    // Starting a finished plan again sends nothing.
    let seen = w.fake.book.lock().unwrap().seen.len();
    planner.start(&plan.id).await.unwrap();
    assert_eq!(w.fake.book.lock().unwrap().seen.len(), seen);
}

#[tokio::test]
async fn an_interrupted_move_retried_makes_one_computer_and_one_continuation_each() {
    let w = world(0).await;
    let run = Run::new(&w);
    let id = {
        let planner = run.planner(&w);
        let plan = planner
            .plan(three_chats(&w), None, Requester::Person, 1_760_000_000)
            .await
            .unwrap();
        planner.approve(&plan.id, 1_760_000_001).unwrap();
        plan.id
    };
    // Each of these is applied, then its reply is lost.
    w.fake.book.lock().unwrap().lose = vec!["create", "command", "credential", "put", "command"];
    *run.hooks.lose_continue.lock().unwrap() = 2;
    let mut tries = 0;
    let done = loop {
        tries += 1;
        assert!(tries < 12, "never finished");
        let plan = run.planner(&w).start(&id).await.unwrap();
        if plan.chats.iter().all(|c| c.state == MoveState::Released) {
            break plan;
        }
        // A chat that hasn't moved yet stays here with a reason.
        assert!(
            plan.chats
                .iter()
                .any(|c| c.state != MoveState::Released && c.reason.is_some())
        );
    };
    assert!(tries > 1);
    let book = w.fake.book.lock().unwrap();
    let keys: std::collections::BTreeSet<_> = book.created.iter().collect();
    assert_eq!(keys.len(), 1, "one computer");
    assert_eq!(book.runs, 9, "no command ran twice");
    assert_eq!(book.credentials.len(), 3, "one login reference a chat");
    let mut puts = book.puts.clone();
    let total = puts.len();
    puts.sort();
    puts.dedup();
    assert_eq!(total, puts.len(), "every piece written once");
    drop(book);
    let continued = run.hooks.continued.lock().unwrap();
    assert_eq!(continued.len(), 3, "one continuation a chat");
    for c in &done.chats {
        assert_eq!(
            c.continuation.as_ref(),
            continued.values().find(|v| v.ends_with(&c.chat))
        );
    }
}

#[tokio::test]
async fn bring_back_fetches_the_work_continues_here_and_stops_the_computer() {
    let w = world(0).await;
    let run = Run::new(&w);
    let planner = run.planner(&w);
    let inputs = vec![PlanInput {
        chat: chat(&w, HarnessId::ClaudeCode),
        title: "Fix the bug".into(),
    }];
    let plan = planner
        .plan(inputs, None, Requester::Person, 1_760_000_000)
        .await
        .unwrap();
    planner.approve(&plan.id, 1_760_000_001).unwrap();
    let moved = planner.start(&plan.id).await.unwrap();
    assert_eq!(states(&moved), vec![MoveState::Released]);

    // The agent worked on the cloud computer: one commit, one new file.
    let remote = w.fake.home.join(format!("openagents/moved/{CHAT}/proj"));
    sh(
        &remote,
        &format!(
            "printf 'fixed\\n' > tracked.txt && {GIT} add -A && {GIT} commit -q -m fix && printf 'notes\\n' > cloud.txt"
        ),
    );

    // An agent asks; nothing happens until the person approves.
    let asked = planner
        .request_bring_back(CHAT, AfterBringBack::Stop, Requester::Agent)
        .unwrap();
    assert!(asked.view().needs_approval());
    let seen = w.fake.book.lock().unwrap().seen.len();
    assert!(matches!(
        planner.bring_back(&plan.id, CHAT).await,
        Err(PlanError::NotApproved)
    ));
    assert_eq!(w.fake.book.lock().unwrap().seen.len(), seen);

    planner.approve_bring_back(&plan.id, CHAT).unwrap();
    let back = planner.bring_back(&plan.id, CHAT).await.unwrap();
    let entry = &back.chats[0];
    assert_eq!(entry.state, MoveState::BroughtBack, "{:?}", entry.reason);
    let branch = format!("openagents/cloud-{}", &CHAT[..8]);
    assert_eq!(
        entry.back.as_ref().unwrap().branch.as_deref(),
        Some(&*branch)
    );
    assert_eq!(
        sh(&w.project, &format!("git show {branch}:tracked.txt")),
        "fixed\n"
    );
    assert_eq!(
        sh(&w.project, &format!("git show {branch}:cloud.txt")),
        "notes\n"
    );
    // The person's checkout is as it was.
    assert_eq!(sh(&w.project, "git symbolic-ref --short HEAD"), "main\n");
    let here = run.hooks.here.lock().unwrap();
    let back_keys: Vec<_> = here.keys().filter(|k| k.starts_with("back-")).collect();
    assert_eq!(back_keys.len(), 1);
    assert_eq!(here[back_keys[0]], (CHAT.to_owned(), Some(branch.clone())));
    drop(here);
    assert_eq!(w.fake.book.lock().unwrap().stopped, 1);
    assert_eq!(w.fake.book.lock().unwrap().deleted, 0);

    // Done: running it again sends nothing, and the chat can move again.
    let seen = w.fake.book.lock().unwrap().seen.len();
    planner.bring_back(&plan.id, CHAT).await.unwrap();
    assert_eq!(w.fake.book.lock().unwrap().seen.len(), seen);
    assert!(w.store.load(CHAT).unwrap().is_none());
    assert!(run.plans.active_for(CHAT).unwrap().is_none());
    let _ = SESSION;
}
