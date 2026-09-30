//! Chat invitations for every paired computer, however it paired.
//!
//! A computer's Coder chats read through a chat pairing (see
//! [`crate::chats`]). The iroh enroll reply and tailnet admission carry a
//! `coder-pair:` invitation; nearby pairing and a connect code redeemed on
//! the relay carry none, and every chat grant ends after 29 days. So the app
//! asks each computer it may observe, and holds no current chat pairing
//! for, over its host link with NIP-HOST `chats.invite`: the same answer on
//! every pairing path, and again before the chat grant ends.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use coder_computers::Enrollment;
use coder_computers::live::Terminals;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::{Code, Right};

/// How long after an answer before the same computer is asked again, while
/// the phone redeems the invitation it got.
const SETTLING: Duration = Duration::from_secs(10 * 60);
/// How long after a failed ask, such as a computer that is not connected.
const RETRY: Duration = Duration::from_secs(60);
/// How long after a computer said it serves no chats: an older host, or
/// one with no Coder task directory.
const NOT_SERVED: Duration = Duration::from_secs(6 * 60 * 60);

/// Why a computer gave no invitation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The computer answered that it serves no chats to this phone.
    NotServed,
    /// The computer could not be reached; ask again soon.
    Failed,
}

/// How the phone asks a computer for a chat invitation.
pub trait Asker: Send + Sync {
    /// `chats.invite` on `host`'s current link: a `coder-pair:` invitation.
    fn invite(&self, host: &str) -> Result<String, Refusal>;
}

/// The live asker: the Computers service's current link to each host.
pub struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }
}

impl Asker for Live {
    fn invite(&self, host: &str) -> Result<String, Refusal> {
        let link = (self.terminals.links(host))().map_err(|_| Refusal::Failed)?;
        match self.handle.block_on(link.call(Operation::InviteChats {})) {
            Ok(Outcome::Chats { invitation, .. }) => Ok(invitation),
            Ok(_) => Err(Refusal::Failed),
            Err(coder_host::Error::Access(error))
                if matches!(
                    error.code,
                    Code::Unavailable
                        | Code::Unsupported
                        | Code::Malformed
                        | Code::MissingRight
                        | Code::Forbidden
                ) =>
            {
                Err(Refusal::NotServed)
            }
            Err(_) => Err(Refusal::Failed),
        }
    }
}

/// The computers to ask, as (host key, label): enrolled with `observe`,
/// and with no current chat pairing linked to them (`linked`). `records` is
/// each Computers host's key, label, and enrollment; how it paired plays
/// no part.
pub fn wanted<'a>(
    records: impl IntoIterator<Item = (&'a str, &'a str, &'a Enrollment)>,
    linked: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    records
        .into_iter()
        .filter(|(_, _, enrollment)| {
            matches!(
                enrollment,
                Enrollment::Enrolled { rights, .. } if rights.contains(Right::Observe)
            )
        })
        .filter(|(key, _, _)| !linked(key))
        .map(|(key, label, _)| (key.to_owned(), label.to_owned()))
        .collect()
}

#[derive(Default)]
struct Inner {
    /// Asks running now, by host key.
    asking: BTreeSet<String>,
    /// The earliest time each host may be asked again.
    next: BTreeMap<String, Instant>,
    /// Invitations to pair on the app thread: host key, label, invitation.
    answers: Vec<(String, String, String)>,
}

/// Asks each computer that needs one for a chat invitation, in the
/// background, at most one ask per computer at a time.
#[derive(Clone, Default)]
pub struct Invites {
    inner: Arc<Mutex<Inner>>,
}

impl Invites {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Note that `host` just got an invitation some other way (the iroh
    /// enroll reply or tailnet admission), so it is not asked while the
    /// phone redeems it.
    pub fn got(&self, host: &str) {
        self.lock()
            .next
            .insert(host.to_owned(), Instant::now() + SETTLING);
    }

    /// Ask each of `hosts` (host key, label) that is not being asked and
    /// is due, on its own thread.
    pub fn ask(&self, hosts: Vec<(String, String)>, asker: Arc<dyn Asker>) {
        let now = Instant::now();
        let due: Vec<(String, String)> = {
            let mut inner = self.lock();
            let due: Vec<(String, String)> = hosts
                .into_iter()
                .filter(|(host, _)| {
                    !inner.asking.contains(host) && inner.next.get(host).is_none_or(|at| now >= *at)
                })
                .collect();
            for (host, _) in &due {
                inner.asking.insert(host.clone());
            }
            due
        };
        for (host, label) in due {
            let invites = self.clone();
            let asker = asker.clone();
            std::thread::spawn(move || {
                let answer = asker.invite(&host);
                let mut inner = invites.lock();
                inner.asking.remove(&host);
                let wait = match answer {
                    Ok(invitation) => {
                        inner.answers.push((host.clone(), label, invitation));
                        SETTLING
                    }
                    Err(Refusal::NotServed) => NOT_SERVED,
                    Err(Refusal::Failed) => RETRY,
                };
                inner.next.insert(host, Instant::now() + wait);
            });
        }
    }

    /// The invitations that arrived since the last call.
    pub fn take(&self) -> Vec<(String, String, String)> {
        std::mem::take(&mut self.lock().answers)
    }

    /// Whether an ask is running.
    #[cfg(test)]
    fn busy(&self) -> bool {
        !self.lock().asking.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        asked: AtomicUsize,
        answer: Result<String, Refusal>,
    }

    impl Asker for Fake {
        fn invite(&self, _host: &str) -> Result<String, Refusal> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            self.answer.clone()
        }
    }

    fn settle(invites: &Invites) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while invites.busy() {
            assert!(Instant::now() < deadline, "the ask never finished");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn enrolled(rights: &str) -> Enrollment {
        Enrollment::Enrolled {
            grant: "g".repeat(64),
            rights: coder_host::access::Rights::parse_list(rights).unwrap(),
            epoch: 0,
            expires_at: u64::MAX,
        }
    }

    #[test]
    fn every_observed_computer_without_current_chats_is_wanted() {
        let records = [
            ("a", "Studio Mac", enrolled("observe,operate")),
            ("b", "Laptop", enrolled("observe,operate,terminal")),
            ("c", "Old Mac", Enrollment::Revoked),
            ("d", "Runner", enrolled("operate")),
        ];
        // A computer with a current chat pairing is left alone, as is one
        // this phone may not observe or no longer holds.
        let wanted = wanted(
            records.iter().map(|(key, label, e)| (*key, *label, e)),
            |host| host == "b",
        );
        assert_eq!(wanted, vec![("a".to_owned(), "Studio Mac".to_owned())]);
    }

    #[test]
    fn an_answer_is_paired_once_and_the_computer_is_not_asked_again_while_it_redeems() {
        let invites = Invites::default();
        let fake = Arc::new(Fake {
            asked: AtomicUsize::new(0),
            answer: Ok("coder-pair:AAAA".into()),
        });
        let hosts = vec![("a".to_owned(), "Studio Mac".to_owned())];
        invites.ask(hosts.clone(), fake.clone());
        settle(&invites);
        assert_eq!(
            invites.take(),
            vec![(
                "a".to_owned(),
                "Studio Mac".to_owned(),
                "coder-pair:AAAA".to_owned()
            )]
        );
        assert!(invites.take().is_empty());
        invites.ask(hosts, fake.clone());
        settle(&invites);
        assert_eq!(fake.asked.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_computer_that_got_its_invitation_another_way_is_not_asked() {
        let invites = Invites::default();
        let fake = Arc::new(Fake {
            asked: AtomicUsize::new(0),
            answer: Ok("coder-pair:AAAA".into()),
        });
        invites.got("a");
        invites.ask(vec![("a".into(), "Studio Mac".into())], fake.clone());
        settle(&invites);
        assert_eq!(fake.asked.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_refusal_waits_before_the_computer_is_asked_again() {
        for answer in [Err(Refusal::NotServed), Err(Refusal::Failed)] {
            let invites = Invites::default();
            let fake = Arc::new(Fake {
                asked: AtomicUsize::new(0),
                answer,
            });
            let hosts = vec![("a".to_owned(), "Studio Mac".to_owned())];
            invites.ask(hosts.clone(), fake.clone());
            settle(&invites);
            invites.ask(hosts, fake.clone());
            settle(&invites);
            assert_eq!(fake.asked.load(Ordering::SeqCst), 1);
            assert!(invites.take().is_empty());
        }
    }
}
