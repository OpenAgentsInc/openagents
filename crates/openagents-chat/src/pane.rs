//! Threads as workbench product panes (`workbench::pane`).
//!
//! [`ThreadPanes`] reads a thread through the chat service, the same way
//! [`crate::thread::collect`] does for export, and describes it: its title,
//! how many turns it holds, whether a reply is arriving, and the one action
//! a thread offers, replying, unless it is archived. It never sends,
//! archives, or creates a thread: a reference to a thread this device does
//! not keep is `missing`, and one whose turn count moved on is `stale`.

use std::sync::Mutex;

use workbench::Revision;
use workbench::pane::{Description, PaneAdapter, PaneKind, PaneState, Subject, TITLE_MAX};

use crate::service::{Command, Snapshot};
use crate::thread::{Thread, collect};

/// The owner intent a thread pane offers: send a message to the thread.
pub const REPLY: &str = "reply";

/// A thread pane adapter over the chat service's operations.
pub struct ThreadPanes<A> {
    apply: Mutex<A>,
}

impl<A> ThreadPanes<A>
where
    A: FnMut(Command) -> Result<Snapshot, String> + Send,
{
    /// An adapter that reads through `apply`: the in-process service or a
    /// host's control socket.
    pub fn new(apply: A) -> Self {
        ThreadPanes {
            apply: Mutex::new(apply),
        }
    }
}

impl<A> PaneAdapter for ThreadPanes<A>
where
    A: FnMut(Command) -> Result<Snapshot, String> + Send,
{
    fn kind(&self) -> PaneKind {
        PaneKind::Thread
    }

    fn describe(&self, subject: &Subject) -> Description {
        let mut apply = self
            .apply
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        describe(collect(subject.id(), &mut *apply), subject.revision())
    }
}

/// A thread pane for what reading the thread answered, against the
/// revision (its turn count) the reference asked for.
#[must_use]
pub fn describe(thread: Result<Thread, String>, asked: Option<&Revision>) -> Description {
    let thread = match thread {
        Ok(thread) => thread,
        Err(error) if error == "Thread not found." => {
            return Description::only(PaneState::Missing, "This thread isn't on this device");
        }
        Err(_) => return Description::only(PaneState::Unavailable, "The thread can't be read now"),
    };
    let turns = thread.turns.len() as u64;
    let title = title(&thread.summary.title);
    if let Some(asked) = asked
        && *asked != Revision::Counter(turns)
    {
        return Description::only(
            PaneState::Stale {
                current: Some(Revision::Counter(turns)),
            },
            title,
        );
    }
    let mut detail = match turns {
        1 => "1 turn".to_owned(),
        n => format!("{n} turns"),
    };
    if thread.busy {
        detail.push_str("\nA reply is arriving.");
    }
    if let Some(failure) = &thread.failure {
        detail.push('\n');
        detail.extend(
            failure
                .chars()
                .filter(|c| !c.is_control())
                .take(workbench::SUMMARY_MAX / 2),
        );
    }
    let actions = if thread.summary.archived {
        Vec::new()
    } else {
        vec![REPLY.to_owned()]
    };
    Description {
        state: PaneState::Ready,
        title,
        detail,
        actions,
    }
}

/// The thread's title as a pane title: plain text within the bound.
fn title(title: &str) -> String {
    let mut clean: String = title.chars().filter(|c| !c.is_control()).collect();
    if clean.trim().is_empty() {
        return "Untitled thread".into();
    }
    if clean.len() > TITLE_MAX {
        let mut end = TITLE_MAX;
        while !clean.is_char_boundary(end) {
            end -= 1;
        }
        clean.truncate(end);
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_chats::Summary;
    use crate::basic_coder::Turn;
    use workbench::pane::{PaneDescriptor, Panes};
    use workbench::{Host, Kind, ResourceRef};

    fn thread(turns: usize, archived: bool) -> Thread {
        Thread {
            summary: Summary {
                id: "chat-1".into(),
                title: "Plan the\u{7} release".into(),
                started: 1,
                updated: 2,
                coder: None,
                archived,
                pinned: false,
                named: true,
            },
            turns: (0..turns)
                .map(|n| Turn::user(format!("message {n}")))
                .collect(),
            busy: true,
            failure: None,
        }
    }

    fn subject(revision: Option<u64>) -> Subject {
        let mut resource = ResourceRef::new(
            Kind::Thread,
            Host::Paired {
                key: "cd".repeat(32),
            },
            "chat-1",
        );
        resource.revision = revision.map(Revision::Counter);
        Subject::Resource { resource }
    }

    fn resolve<A>(adapter: ThreadPanes<A>, revision: Option<u64>) -> PaneDescriptor
    where
        A: FnMut(Command) -> Result<Snapshot, String> + Send + 'static,
    {
        let pane = Panes::new()
            .adapter(Box::new(adapter))
            .resolve(PaneKind::Thread, &subject(revision))
            .unwrap();
        pane.check().unwrap();
        pane
    }

    #[test]
    fn a_thread_reads_as_its_title_turns_and_reply() {
        let ready = describe(Ok(thread(3, false)), None);
        assert_eq!(ready.state, PaneState::Ready);
        assert_eq!(ready.title, "Plan the release");
        assert_eq!(ready.detail, "3 turns\nA reply is arriving.");
        assert_eq!(ready.actions, vec![REPLY]);
        // An archived thread is read-only.
        assert!(describe(Ok(thread(3, true)), None).actions.is_empty());
        // A reference to an older turn count is stale and names the current.
        let stale = describe(Ok(thread(3, false)), Some(&Revision::Counter(1)));
        assert_eq!(
            stale.state,
            PaneState::Stale {
                current: Some(Revision::Counter(3))
            }
        );
    }

    #[test]
    fn a_thread_this_device_lacks_is_missing_and_nothing_is_created() {
        let missing = resolve(
            ThreadPanes::new(|command: Command| {
                // The adapter only reads.
                assert!(matches!(
                    command,
                    Command::Read { .. } | Command::ListMore { .. }
                ));
                Err("Thread not found.".into())
            }),
            None,
        );
        assert_eq!(missing.state, PaneState::Missing);
        assert!(missing.actions.is_empty());
        let unavailable = resolve(
            ThreadPanes::new(|_: Command| Err("The host's control socket is closed.".into())),
            Some(2),
        );
        assert_eq!(unavailable.state, PaneState::Unavailable);
    }
}
