//! Your public profile and the agents you own (#12298, #12301).
//!
//! The profile is a kind 0 event this device's key signs (NIP-01, NIP-24),
//! written over the key's current profile so fields another Nostr app set
//! stay, and published to the OpenAgents relay. The `name@openagents.com`
//! name is the account's, claimed under the app session and pointed at
//! this key, so `/.well-known/nostr.json` answers for it.
//!
//! A published agent has a key of its own, which the website holds. Its
//! owner proves ownership with a NIP-OA `auth` tag over the agent's key.
//! A browser extension can only sign whole events, so the apps, which hold
//! the key, sign the tag: [`own`] reads what to sign, mints the tag, and
//! hands it to the website, which publishes the agent's profile and its
//! definition. The tag stands for a year; [`renew_due`] signs a new one a
//! month before it lapses.
//!
//! [`ProfileForm`] is the Your ID profile form as a Rust Native view, the
//! same on the desktop and the phone.
use crate::{Controller, identity::Identity};
use nostr::{
    domain::{Event, Tag},
    metadata::{self, Metadata},
};
use nostr_transport::Connection;
use openagents::{Client, Error, Session};
use rust_native::{Activation, Axis, Element, FieldChange, Node, TextRole, View};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

/// The relay profiles are published to and read from.
pub const RELAY: &str = "wss://relay.openagents.com";
/// The domain names live under.
pub const DOMAIN: &str = "openagents.com";
/// How long one relay read or publish may take.
const WAIT: Duration = Duration::from_secs(8);
/// The most frames one relay exchange reads.
const FRAMES: usize = 64;
/// Renew an owner tag this long before it lapses.
pub const RENEW_BEFORE: u64 = 30 * 24 * 60 * 60;

/// Where this device's key comes from when the form or Agent Lab needs it.
/// `None` until the identity is open.
pub type KeySource = Arc<dyn Fn() -> Option<Arc<Identity>> + Send + Sync>;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

const UNREACHABLE: &str = "We couldn't reach your profile right now. Try again in a minute.";
const NOT_SAVED: &str = "We couldn't publish your profile right now. Try again in a minute.";

/// The events `relay` holds for `filter`, with valid ids and signatures.
///
/// # Errors
///
/// A sentence for the person when the relay can't be reached.
pub async fn read(relay: &str, filter: Value) -> Result<Vec<Event>, String> {
    let mut connection = Connection::connect_open(relay, WAIT)
        .await
        .map_err(|_| UNREACHABLE.to_owned())?;
    let subscription = "profile";
    connection
        .send(json!(["REQ", subscription, filter]))
        .await
        .map_err(|_| UNREACHABLE.to_owned())?;
    let mut events = Vec::new();
    for _ in 0..FRAMES {
        let frame = connection
            .next()
            .await
            .map_err(|_| UNREACHABLE.to_owned())?;
        match frame.get(0).and_then(Value::as_str) {
            Some("EVENT") if frame.get(1) == Some(&json!(subscription)) => {
                if let Some(event) = frame
                    .get(2)
                    .and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
                    && event.validate_crypto().is_ok()
                {
                    events.push(event);
                }
            }
            Some("EOSE") | Some("CLOSED") => break,
            _ => {}
        }
    }
    let _ = connection.send(json!(["CLOSE", subscription])).await;
    let _ = connection.close().await;
    Ok(events)
}

/// The newest profile `pubkey` signed on `relay`, if any.
///
/// # Errors
///
/// A sentence for the person when the relay can't be reached.
pub async fn newest_profile(relay: &str, pubkey: &str) -> Result<Option<Event>, String> {
    let events = read(
        relay,
        json!({"authors": [pubkey], "kinds": [0], "limit": 5}),
    )
    .await?;
    Ok(metadata::newest(pubkey, metadata::METADATA_KIND, &events).cloned())
}

/// Publish `event` to `relay`, answering a NIP-42 challenge with
/// `identity` when the relay asks for one.
///
/// # Errors
///
/// A sentence for the person when the relay refused or can't be reached.
pub async fn publish(relay: &str, identity: &Identity, event: &Event) -> Result<(), String> {
    let mut connection = Connection::connect_open(relay, WAIT)
        .await
        .map_err(|_| NOT_SAVED.to_owned())?;
    let send = json!(["EVENT", event]);
    connection
        .send(send.clone())
        .await
        .map_err(|_| NOT_SAVED.to_owned())?;
    // The id of our AUTH once sent, and whether the event waits for it.
    let mut auth_id: Option<String> = None;
    let mut waiting = false;
    let mut resent = false;
    for _ in 0..FRAMES {
        let frame = connection.next().await.map_err(|_| NOT_SAVED.to_owned())?;
        match frame.get(0).and_then(Value::as_str) {
            Some("AUTH") if auth_id.is_none() => {
                let Some(challenge) = frame.get(1).and_then(Value::as_str) else {
                    continue;
                };
                let tags = vec![
                    Tag::new(vec!["relay".into(), relay.into()]),
                    Tag::new(vec!["challenge".into(), challenge.into()]),
                ];
                let auth = identity
                    .relay_signer()
                    .sign(now(), 22_242, tags, String::new());
                auth_id = Some(auth.id.clone());
                connection
                    .send(json!(["AUTH", auth]))
                    .await
                    .map_err(|_| NOT_SAVED.to_owned())?;
            }
            Some("OK") if frame.get(1) == Some(&json!(event.id)) => {
                if frame.get(2) == Some(&json!(true)) {
                    let _ = connection.close().await;
                    return Ok(());
                }
                let reason = frame.get(3).and_then(Value::as_str).unwrap_or_default();
                if !reason.starts_with("auth-required") || resent {
                    return Err(NOT_SAVED.to_owned());
                }
                // Sent again once the relay has taken our AUTH.
                waiting = true;
            }
            Some("OK") if auth_id.is_some() && frame.get(1) == Some(&json!(auth_id)) => {
                if frame.get(2) != Some(&json!(true)) {
                    return Err(NOT_SAVED.to_owned());
                }
            }
            _ => {}
        }
        // Resend once both the refusal and our accepted AUTH are in.
        if waiting && !resent && auth_id.is_some() {
            resent = true;
            waiting = false;
            connection
                .send(send.clone())
                .await
                .map_err(|_| NOT_SAVED.to_owned())?;
        }
    }
    Err(NOT_SAVED.to_owned())
}

/// Your public profile as the form shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct YourProfile {
    /// The `name@openagents.com` name, or the profile's name when the
    /// account has none yet.
    pub name: String,
    pub picture: String,
    pub about: String,
    #[serde(default)]
    pub lightning: String,
    /// `name@openagents.com`, once the account holds the name.
    pub address: Option<String>,
    /// Where the profile shows on the website.
    pub page: String,
}

/// The fields a person edits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileEdit {
    pub name: String,
    pub picture: String,
    pub about: String,
    pub lightning: String,
}

/// What to say when the account service refused a name.
#[must_use]
pub fn name_problem(error: &Error) -> String {
    match error {
        Error::Refused { code, .. } => match *code {
            Some("name_invalid") => {
                "Use 2 to 30 lowercase letters, numbers, dashes, or underscores, starting and ending with a letter or number."
            }
            Some("name_reserved") => "That name is reserved. Try another.",
            Some("name_taken") => "Someone already has that name. Try another.",
            Some("name_too_often") => {
                "You've changed your name a few times today. Try again tomorrow."
            }
            Some("key_not_yours") => "This device's key isn't on your account yet.",
            _ => "That name can't be used. Try another.",
        }
        .to_owned(),
        Error::Network { .. } | Error::Unavailable { .. } => UNREACHABLE.to_owned(),
        other => other.to_string(),
    }
}

fn page(npub: &str, name: Option<&str>) -> String {
    match name {
        Some(name) => format!("https://{DOMAIN}/@{name}"),
        None => format!("https://{DOMAIN}/@{npub}"),
    }
}

fn shown(name: Option<String>, metadata: &Metadata, npub: &str) -> YourProfile {
    YourProfile {
        address: name.as_ref().map(|n| format!("{n}@{DOMAIN}")),
        page: page(npub, name.as_deref()),
        name: name.or_else(|| metadata.name.clone()).unwrap_or_default(),
        picture: metadata.picture.clone().unwrap_or_default(),
        about: metadata.about.clone().unwrap_or_default(),
        lightning: metadata.lud16.clone().unwrap_or_default(),
    }
}

/// Read your profile: the account's name and this key's newest kind 0.
///
/// # Errors
///
/// A sentence for the person.
pub async fn load(
    client: &Client,
    session: &Session,
    identity: &Identity,
    relay: &str,
) -> Result<YourProfile, String> {
    let name = client
        .account_name(session)
        .await
        .map_err(|e| name_problem(&e))?
        .name;
    let earlier = newest_profile(relay, &identity.pubkey_hex()).await?;
    let metadata = earlier
        .as_ref()
        .map(|e| Metadata::read(&e.content))
        .unwrap_or_default();
    Ok(shown(name, &metadata, &identity.npub()))
}

/// The signed kind 0 for `edit` over `earlier`, with `name` as its claimed
/// `name@openagents.com` name when there is one.
///
/// # Errors
///
/// A sentence naming the field to fix.
pub fn profile_event(
    identity: &Identity,
    edit: &ProfileEdit,
    name: Option<&str>,
    earlier: Option<&Event>,
    at: u64,
) -> Result<Event, String> {
    let (picture, about) = (edit.picture.trim(), edit.about.trim());
    let check = Metadata {
        name: Some(edit.name.trim().to_owned()).filter(|n| !n.is_empty()),
        picture: Some(picture.to_owned()).filter(|p| !p.is_empty()),
        about: Some(about.to_owned()).filter(|a| !a.is_empty()),
        lud16: Some(edit.lightning.trim().to_owned()).filter(|a| !a.is_empty()),
        ..Metadata::default()
    };
    check.check().map_err(str::to_owned)?;
    let fields = Metadata {
        // An empty field removes it from the profile.
        name: Some(name.unwrap_or(edit.name.trim()).to_owned()),
        picture: Some(picture.to_owned()),
        about: Some(about.to_owned()),
        lud16: Some(edit.lightning.trim().to_owned()),
        nip05: name.map(|n| format!("{n}@{DOMAIN}")),
        ..Metadata::default()
    };
    let created_at = earlier.map_or(at, |e| at.max(e.created_at + 1));
    Ok(fields.sign(
        &identity.relay_signer(),
        created_at,
        earlier.map(|e| e.content.as_str()),
    ))
}

/// Claim the name (when it changed) and publish the profile.
///
/// # Errors
///
/// A sentence for the person; nothing is published when the name was
/// refused or the current profile can't be read.
pub async fn save(
    client: &Client,
    session: &Session,
    identity: &Identity,
    relay: &str,
    edit: &ProfileEdit,
) -> Result<YourProfile, String> {
    let pubkey = identity.pubkey_hex();
    let wanted = edit.name.trim().to_ascii_lowercase();
    let held = client
        .account_name(session)
        .await
        .map_err(|e| name_problem(&e))?;
    let name = if wanted.is_empty() {
        held.name.clone()
    } else if held.name.as_deref() == Some(wanted.as_str())
        && held.pubkey.as_deref() == Some(pubkey.as_str())
    {
        Some(wanted)
    } else {
        client
            .claim_account_name(session, &wanted, Some(&pubkey))
            .await
            .map_err(|e| name_problem(&e))?
            .name
    };
    let earlier = newest_profile(relay, &pubkey).await?;
    let event = profile_event(identity, edit, name.as_deref(), earlier.as_ref(), now())?;
    publish(relay, identity, &event).await?;
    Ok(shown(
        name,
        &Metadata::read(&event.content),
        &identity.npub(),
    ))
}

/// A published agent this device signed the owner tag for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owned {
    /// The published agent's ID.
    pub id: String,
    /// The agent's own key as `npub1…`.
    pub npub: String,
    /// When the tag lapses, in Unix seconds.
    pub until: u64,
}

fn until_of(conditions: &str) -> Option<u64> {
    conditions.strip_prefix("created_at<")?.parse().ok()
}

/// Sign the owner tag for published agent `id` with `identity` and hand it
/// to the website, which publishes the agent's profile and definition.
///
/// # Errors
///
/// A sentence for the person.
pub async fn own(
    client: &Client,
    session: &Session,
    identity: &Identity,
    id: &str,
) -> Result<Owned, String> {
    let owner = client
        .published_agent_owner(id)
        .await
        .map_err(|_| "We couldn't reach this agent. Try again.".to_owned())?;
    if owner.revoked {
        return Err("This agent was retired.".into());
    }
    let until = until_of(&owner.conditions).ok_or("We couldn't sign for this agent.")?;
    let tag = nostr::domain::mint_owner_attestation(
        &identity.secret_key(),
        &owner.agent,
        &owner.conditions,
    )
    .map_err(|_| "We couldn't sign for this agent.".to_owned())?
    .tag();
    let set = client
        .set_published_agent_owner(session, id, tag.as_slice())
        .await
        .map_err(|_| "We couldn't make you this agent's owner. Try again.".to_owned())?;
    Ok(Owned {
        id: id.to_owned(),
        npub: set.npub,
        until,
    })
}

/// The owner tags this device signed, by published agent ID.
fn book(dir: &Path) -> PathBuf {
    dir.join("owned-agents.json")
}

fn read_book(dir: &Path) -> BTreeMap<String, Owned> {
    std::fs::read(book(dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Keep `owned` so its tag is renewed before it lapses.
pub fn remember(dir: &Path, owned: &Owned) {
    let mut all = read_book(dir);
    all.insert(owned.id.clone(), owned.clone());
    if let Ok(bytes) = serde_json::to_vec(&all) {
        let _ = std::fs::create_dir_all(dir);
        let temporary = book(dir).with_extension("new");
        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(temporary, book(dir));
        }
    }
}

/// The published agents whose tags lapse within [`RENEW_BEFORE`] of `at`.
#[must_use]
pub fn due(dir: &Path, at: u64) -> Vec<String> {
    read_book(dir)
        .into_values()
        .filter(|o| o.until <= at.saturating_add(RENEW_BEFORE))
        .map(|o| o.id)
        .collect()
}

/// Sign new tags for every agent that is [`due`]. Answers how many were
/// renewed; one that fails is tried again next time.
pub async fn renew_due(
    dir: &Path,
    client: &Client,
    session: &Session,
    identity: &Identity,
) -> usize {
    let mut renewed = 0;
    for id in due(dir, now()) {
        if let Ok(owned) = own(client, session, identity, &id).await {
            remember(dir, &owned);
            renewed += 1;
        }
    }
    renewed
}

/// The profile form's intent: the node it came from.
pub type Intent = String;
mod earnings;

#[derive(Default)]
struct FormState {
    revision: u64,
    earnings: Vec<openagents::profile_earnings::Earning>,
    earnings_generation: Option<u64>,
    edit: ProfileEdit,
    shown: Option<YourProfile>,
    notice: Option<String>,
    busy: bool,
    /// A browser's Verse key waiting for this key's owner tag (#12313).
    verse: Option<openagents::profiles::VerseKey>,
}

/// Settings > Your ID's public profile form: name, picture, and about, and
/// Save, which claims the name and publishes the profile.
pub struct ProfileForm {
    instance: String,
    controller: Arc<Controller>,
    key: KeySource,
    relay: String,
    state: Mutex<FormState>,
    updates: tokio::sync::watch::Sender<u64>,
    io: tokio::sync::Mutex<()>,
}

/// The form's words.
pub mod copy {
    pub const TITLE: &str = "Public profile";
    pub const NAME: &str = "Name";
    pub const PICTURE: &str = "Picture link";
    pub const ABOUT: &str = "About";
    pub const LIGHTNING: &str = "Lightning address";
    pub const SAVE: &str = "Save";
    pub const SAVING: &str = "Saving…";
    pub const LOADING: &str = "Loading your profile…";
    pub const SAVED: &str = "Saved. Your profile is on your page.";
    pub const NOT_READY: &str = "Your account isn't ready yet. Try again in a moment.";
    pub const VERSE_ASKS: &str =
        "A browser you're logged in on wants to show your name over your avatar in the Verse.";
    pub const VERSE_LINK: &str = "Show my name there";
    pub const VERSE_LINKED: &str = "Done. Your avatar in the Verse shows your name.";
}

impl ProfileForm {
    /// The form over the account `controller` signs in with and the key
    /// `key` answers, publishing to [`RELAY`].
    pub fn new(controller: Arc<Controller>, key: KeySource) -> Arc<Self> {
        Self::with_relay(controller, key, RELAY.into())
    }

    /// The same, publishing to `relay`.
    pub fn with_relay(controller: Arc<Controller>, key: KeySource, relay: String) -> Arc<Self> {
        let (updates, _) = tokio::sync::watch::channel(1);
        Arc::new(Self {
            instance: "public-profile".into(),
            controller,
            key,
            relay,
            state: Mutex::new(FormState {
                revision: 1,
                ..FormState::default()
            }),
            updates,
            io: tokio::sync::Mutex::new(()),
        })
    }

    /// Changes whenever the view changes.
    pub fn watch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.updates.subscribe()
    }

    fn changed(&self, state: &mut FormState) {
        state.revision = state.revision.wrapping_add(1).max(1);
        let revision = state.revision;
        self.updates.send_modify(|r| *r = revision);
    }

    /// The profile last loaded or saved.
    pub fn shown(&self) -> Option<YourProfile> {
        self.state.lock().unwrap().shown.clone()
    }

    /// The form as a Rust Native view.
    pub fn view(&self) -> View<Intent> {
        let state = self.state.lock().unwrap();
        let ready = state.shown.is_some() && !state.busy;
        let text = |key: &str, value: String, role| Node {
            key: key.into(),
            style: Default::default(),
            element: Element::Text { value, role },
        };
        let field = |key: &str, label: &str, value: &str, max: usize, multiline| Node {
            key: key.into(),
            style: Default::default(),
            element: Element::Field {
                label: label.into(),
                value: value.into(),
                placeholder: String::new(),
                secret: false,
                multiline,
                enabled: ready,
                max_bytes: max,
                on_change: key.to_owned(),
            },
        };
        let mut children = vec![text("title", copy::TITLE.into(), TextRole::Heading)];
        if let Some(address) = state.shown.as_ref().and_then(|s| s.address.clone()) {
            children.push(text("address", address, TextRole::Body));
        }
        children.extend([
            field("name", copy::NAME, &state.edit.name, 30, false),
            field(
                "picture",
                copy::PICTURE,
                &state.edit.picture,
                metadata::URL_MAX,
                false,
            ),
            field(
                "about",
                copy::ABOUT,
                &state.edit.about,
                metadata::ABOUT_MAX * 4,
                true,
            ),
            field(
                "lightning",
                copy::LIGHTNING,
                &state.edit.lightning,
                320,
                false,
            ),
            Node {
                key: "save".into(),
                style: Default::default(),
                element: Element::Button {
                    label: if state.busy { copy::SAVING } else { copy::SAVE }.into(),
                    enabled: ready,
                    icon: None,
                    shortcut: None,
                    intent: "save".into(),
                },
            },
        ]);
        let notice = match (&state.notice, &state.shown) {
            (Some(notice), _) => Some(notice.clone()),
            (None, None) => Some(copy::LOADING.into()),
            _ => None,
        };
        if let Some(notice) = notice {
            children.push(text("notice", notice, TextRole::Status));
        }
        if let Some(shown) = &state.shown {
            children.push(text("page", shown.page.clone(), TextRole::Body));
        }
        if state.verse.is_some() {
            children.push(text("verse", copy::VERSE_ASKS.into(), TextRole::Body));
            children.push(Node {
                key: "verse-link".into(),
                style: Default::default(),
                element: Element::Button {
                    label: copy::VERSE_LINK.into(),
                    enabled: !state.busy,
                    icon: None,
                    shortcut: None,
                    intent: "verse-link".into(),
                },
            });
        }
        earnings::append(
            &mut children,
            &state,
            self.controller.lease().map(|l| l.generation),
        );
        View::new_v3(
            &self.instance,
            state.revision,
            Node {
                key: "public-profile".into(),
                style: Default::default(),
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        )
    }

    /// Keep a typed change to one of the fields.
    ///
    /// # Errors
    ///
    /// When the change is for a view that has since changed.
    pub fn change_field(&self, event: &FieldChange) -> Result<(), String> {
        let key = self
            .view()
            .validate()
            .map_err(|_| "Couldn't open your profile.".to_owned())?
            .change_field(event)
            .map_err(|_| "This field changed. Try it again.".to_owned())?
            .clone();
        let mut state = self.state.lock().unwrap();
        match key.as_str() {
            "name" => state.edit.name = event.value.clone(),
            "picture" => state.edit.picture = event.value.clone(),
            "about" => state.edit.about = event.value.clone(),
            "lightning" => state.edit.lightning = event.value.clone(),
            _ => return Err("Choose a field.".into()),
        }
        state.notice = None;
        self.changed(&mut state);
        Ok(())
    }

    fn signed_in(&self) -> Result<(crate::Lease, Arc<Identity>), String> {
        let lease = self.controller.lease().ok_or(copy::NOT_READY)?;
        let identity = (self.key)().ok_or(copy::NOT_READY)?;
        Ok((lease, identity))
    }

    /// Read the profile into the form.
    ///
    /// # Errors
    ///
    /// A sentence for the person, also shown in the form.
    pub async fn load(&self) -> Result<(), String> {
        let _gate = self.io.lock().await;
        let result = async {
            let (lease, identity) = self.signed_in()?;
            let shown = load(lease.client(), lease.session(), &identity, &self.relay).await?;
            // A browser waiting for its Verse key's owner tag, if any.
            let verse = lease
                .client()
                .verse_key(lease.session())
                .await
                .ok()
                .flatten()
                .filter(|key| key.auth.is_none());
            let earnings = lease
                .client()
                .profile_earnings(lease.session(), &identity.pubkey_hex())
                .await
                .ok()
                .map(|r| r.rows)
                .unwrap_or_default();
            if !lease.is_current() {
                return Err(copy::NOT_READY.to_owned());
            }
            Ok::<_, String>((shown, verse, earnings, lease.generation))
        }
        .await;
        let mut state = self.state.lock().unwrap();
        let result = result.map(|(shown, verse, earnings, generation)| {
            state.earnings = earnings;
            state.earnings_generation = Some(generation);
            state.verse = verse;
            shown
        });
        match &result {
            Ok(shown) => {
                state.edit = ProfileEdit {
                    name: shown.name.clone(),
                    picture: shown.picture.clone(),
                    about: shown.about.clone(),
                    lightning: shown.lightning.clone(),
                };
                state.shown = Some(shown.clone());
                state.notice = None;
            }
            Err(message) => state.notice = Some(message.clone()),
        }
        self.changed(&mut state);
        result.map(|_| ())
    }

    /// Run the button the person pressed: Save.
    ///
    /// # Errors
    ///
    /// A sentence for the person, also shown in the form.
    pub async fn activate(&self, event: &Activation) -> Result<(), String> {
        let intent = self
            .view()
            .validate()
            .map_err(|_| "Couldn't open your profile.".to_owned())?
            .activate(event)
            .map_err(|_| "This view changed. Try that again.".to_owned())?
            .clone();
        if let Some(source) = intent.strip_prefix("earning:") {
            return self.set_earning(source).await;
        }
        if intent == "verse-link" {
            return self.link_verse().await;
        }
        if intent != "save" {
            return Err("Choose an action.".into());
        }
        let _gate = self.io.lock().await;
        let edit = {
            let mut state = self.state.lock().unwrap();
            state.busy = true;
            state.notice = None;
            self.changed(&mut state);
            state.edit.clone()
        };
        let result = async {
            let (lease, identity) = self.signed_in()?;
            save(
                lease.client(),
                lease.session(),
                &identity,
                &self.relay,
                &edit,
            )
            .await
        }
        .await;
        let mut state = self.state.lock().unwrap();
        state.busy = false;
        match &result {
            Ok(shown) => {
                state.edit.name = shown.name.clone();
                state.shown = Some(shown.clone());
                state.notice = Some(copy::SAVED.into());
            }
            Err(message) => state.notice = Some(message.clone()),
        }
        self.changed(&mut state);
        result.map(|_| ())
    }
}

impl ProfileForm {
    /// Sign the waiting browser Verse key's owner tag with this key.
    async fn link_verse(&self) -> Result<(), String> {
        let _gate = self.io.lock().await;
        let Some(key) = self.state.lock().unwrap().verse.clone() else {
            return Err("No browser is waiting.".into());
        };
        let result = async {
            let (lease, identity) = self.signed_in()?;
            let tag = verse_tag(&identity, &key)?;
            lease
                .client()
                .own_verse_key(lease.session(), tag.as_slice())
                .await
                .map_err(|_| "We couldn't show your name there. Try again.".to_owned())
        }
        .await;
        let mut state = self.state.lock().unwrap();
        match &result {
            Ok(_) => {
                state.verse = None;
                state.notice = Some(copy::VERSE_LINKED.into());
            }
            Err(message) => state.notice = Some(message.clone()),
        }
        self.changed(&mut state);
        result.map(|_| ())
    }
}

/// The NIP-OA tag by which `identity` owns a browser's Verse key, over the
/// conditions the site named, which must stand for at most a year.
///
/// # Errors
///
/// A sentence for the person.
pub fn verse_tag(
    identity: &Identity,
    key: &openagents::profiles::VerseKey,
) -> Result<nostr::domain::Tag, String> {
    until_of(&key.conditions).ok_or("We couldn't sign for that browser.")?;
    Ok(
        nostr::domain::mint_owner_attestation(&identity.secret_key(), &key.pubkey, &key.conditions)
            .map_err(|_| "We couldn't sign for that browser.".to_owned())?
            .tag(),
    )
}

#[cfg(test)]
mod tests;
