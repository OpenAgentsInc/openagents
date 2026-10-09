//! One OpenAgents app lifetime: the Coder tab, the Computers surface, the
//! terminal screen it opens, and the Tailnet surface.

use crate::chats::Chats;
use crate::coder_tab::CoderTab;
use crate::computers_home::{
    self, Choice as ComputersChoice, Destination as ComputersDestination, Home,
};
use crate::tailnet::{Client, Outcome as TailnetOutcome};
use crate::tailnet_view::{self, Admit, Intent as TailnetIntent, Screen as TailnetScreen};
use coder_computers::cache::Cache;
use coder_computers::live::{Live, Saved, Settings, Store, Terminals};
use coder_computers::terminal::screen::{Outcome as TerminalOutcome, Terminal, TerminalPacket};
use coder_computers::{Capabilities, Computers, InputRequest, LocalHost, Outcome, Platform};
use coder_host::tailnet::Admission;
use rust_native::{Activation, ValidatedView, View};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

const TAILNET_REFRESH_LIMIT: Duration = Duration::from_secs(20);
const TAILNET_SIGN_IN_LIMIT: Duration = Duration::from_secs(300);

#[derive(Deserialize)]
pub struct Config {
    /// A private directory for the app's encrypted records.
    pub state_dir: PathBuf,
    /// This device's Nostr secret key, from the platform's protected store.
    pub secret_hex: String,
}

/// In a debug build, `OPENAGENTS_COMPUTERS_FIXTURE` set in the environment
/// swaps the live client for Coder's offline Computers fixture, for
/// simulator checks: `SIMCTL_CHILD_OPENAGENTS_COMPUTERS_FIXTURE=1 xcrun simctl
/// launch ...`. The fixture contacts no host or relay. Release builds ignore it.
#[cfg(debug_assertions)]
fn fixture_requested() -> bool {
    std::env::var_os("OPENAGENTS_COMPUTERS_FIXTURE").is_some()
}

/// Launch options beside [`Config`] in the host's configuration.
#[derive(Default, Deserialize)]
pub struct Launch {
    /// Draw Computers from Coder's offline fixture instead of real hosts,
    /// as [`fixture_requested`] does, for tests. It contacts no host or
    /// relay.
    #[serde(default)]
    pub computers_fixture: bool,
    /// The host draws the Computers list natively (`computers_home`) and
    /// its own navigation, so the shared screens drop their tab row. The iOS
    /// host sets it; the Android host draws the shared screens whole.
    #[serde(default)]
    pub native_computers: bool,
    /// The host's transcript layout reads chat rows from Rust
    /// (`rust_native::layout::source`) instead of the view, so a chat's
    /// length is not bounded by the view's size.
    #[serde(default)]
    pub pulled_transcripts: bool,
    /// The host draws the phone's shell (#11126): the top bar with the
    /// Chat / Code switch, the drawer, and the feature cards, from the
    /// packet's `shell`. Both phone hosts set it.
    #[serde(default)]
    pub shell: bool,
    /// Run the Wallet tab on an offline fixture wallet, for simulator
    /// screenshots. Honored only in debug builds; it holds no money and
    /// reaches no network.
    #[serde(default)]
    pub wallet_fixture: bool,
    /// The basic Coder's relay and worker key, in place of the OpenAgents
    /// chat worker, for tests against a local worker. Honored only in debug
    /// builds.
    #[serde(default)]
    pub chat_relay: Option<String>,
    #[serde(default)]
    pub chat_worker: Option<String>,
    /// Answer chats from an offline worker that sends the chat router's
    /// fields (`chat_fixture`), for simulator screenshots. Honored only in
    /// debug builds; it reaches no network.
    #[serde(default)]
    pub chat_fixture: bool,
    /// Answer chats and run tests offline with the chat router's recorded
    /// Gym cards and a recorded report (`gym_fixture`), for simulator
    /// screenshots of states a live chat can't reach yet. Debug builds only.
    #[serde(default)]
    pub gym_fixture: bool,
    /// Messages the chat sends at launch, one reply at a time, with `!run`
    /// and `!wrong` steps, for simulator screenshots. Debug builds only.
    #[serde(default)]
    pub chat_script: Vec<String>,
    /// The app's version and build, as `1.0.0 (19)`: the chat router's
    /// context names it, so the worker can answer for this build.
    #[serde(default)]
    pub app_build: Option<String>,
    /// Where the Gym intro starts, for simulator screenshots: `choose`,
    /// `end_card`, `chat`, or `done`; each opts into the Gym. Debug builds
    /// only.
    #[serde(default)]
    pub gym_first_run: Option<String>,
    /// Push wakes through a relay's NIP-PL executor and a push gateway, so
    /// a computer's spend request reaches a phone that is not looking.
    /// Absent, the default, leaves push off.
    #[serde(default)]
    pub push: Option<coder_mobile::PushConfig>,
    /// This device's iroh secret key as 64 hex characters, from the same
    /// this-device-only store as the device key. Connecting a computer
    /// dials it over iroh with this key; without it the phone connects over
    /// the Nostr relay only.
    #[serde(default)]
    pub iroh_secret_hex: Option<String>,
}

/// The basic Coder's door: the OpenAgents chat worker on its relay, or, in
/// a debug build, the relay and worker the launch names.
fn basic_door(launch: &Launch, secret: SecretKey) -> Option<Arc<dyn crate::basic_coder::Door>> {
    let debug = cfg!(debug_assertions);
    #[cfg(debug_assertions)]
    {
        if launch.chat_fixture {
            return Some(Arc::new(crate::chat_fixture::ChatFixture));
        }
        if launch.gym_fixture {
            return Some(Arc::new(crate::gym_fixture::GymFixture));
        }
    }
    let relay = launch
        .chat_relay
        .as_deref()
        .filter(|_| debug)
        .unwrap_or(crate::basic_coder::RELAY);
    let worker = launch
        .chat_worker
        .as_deref()
        .filter(|_| debug)
        .unwrap_or(crate::basic_coder::WORKER);
    crate::basic_coder::Relay::new(relay, worker, secret)
        .ok()
        .map(|door| {
            Arc::new(door.with_wake(Arc::new(crate::wake::ring)))
                as Arc<dyn crate::basic_coder::Door>
        })
}

/// The hosted eval runner this build sends test runs to
/// (`crate::gym::Hosted`): the deployed runner, or, in a debug build with
/// `gym_fixture`, the offline recorded one.
fn hosted_runner(launch: &Launch, _secret: SecretKey) -> Option<Arc<dyn crate::gym::Hosted>> {
    #[cfg(debug_assertions)]
    if launch.gym_fixture {
        return Some(Arc::new(crate::gym_fixture::FixtureRunner));
    }
    #[cfg(not(debug_assertions))]
    let _ = launch;
    Some(Arc::new(crate::hosted::HostedRelay::new(None, None)))
}

/// What this phone's Computers screens can do.
const CAPABILITIES: Capabilities = Capabilities {
    platform: Platform::Phone,
    camera: true,
};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// The current packet, without other work.
    Snapshot,
    /// The current packet after Rust said it changed (`wake`): reloads the
    /// Computers state first when that changed.
    Changed,
    /// The app moved to the foreground or the background.
    Lifecycle {
        active: bool,
    },
    ComputersActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    ComputersInput {
        token: String,
        value: String,
    },
    ComputersCancel {
        token: String,
    },
    ComputersRefresh,
    /// Show **Connect a computer** (`SCR-22`), from Account > Computers.
    ConnectOpen,
    /// The text of a scanned or pasted code on `SCR-22`.
    ConnectCode {
        value: String,
    },
    /// A connect link the phone opened the app with
    /// (`https://openagents.com/connect#...`, from the system camera or a
    /// tap): show `SCR-22` and pair with it, as a scan would.
    ConnectLink {
        value: String,
    },
    /// Close `SCR-22` or `SCR-23` (**Done**).
    ConnectClose,
    /// A tap on a computer in **Nearby** on `SCR-22`.
    ConnectNearby {
        id: String,
    },
    /// Open a computer from the native Computers list.
    ComputersOpen {
        host: String,
    },
    /// A choice from a row's menu on the native Computers list. The host
    /// confirms a destructive choice before it sends it.
    ComputersChoose {
        host: String,
        choice: ComputersChoice,
    },
    /// The list's own controls, and back to the list from a computer's
    /// screens.
    ComputersGo {
        to: ComputersDestination,
    },
    CoderActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// A composer's send on the Coder surface.
    CoderInput {
        token: String,
        value: String,
    },
    TailnetActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Read the tailnet again in the background.
    TailnetRefresh,
    /// The browser opened the sign-in page: wait for approval in the
    /// background.
    TailnetWaitForSignIn,
    TerminalActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Poll the terminal. `known` is the view revision the host has.
    TerminalPoll {
        known: Option<u64>,
    },
    TerminalResize {
        rows: u16,
        cols: u16,
    },
    TerminalText {
        text: String,
    },
    TerminalKey {
        key: String,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
        #[serde(default)]
        shift: bool,
    },
    TerminalPaste {
        text: String,
    },
    /// This device's identity keys and the changelog. `reveal` returns the
    /// secret key too; the Identity Keys screen sends it only after the
    /// person asks to see the nsec and confirms a warning.
    Account {
        #[serde(default)]
        reveal: bool,
    },
    /// Sets the display name shown over the player's head in the Verse
    /// (Account > Display name), then answers with the account packet.
    /// An empty name clears it.
    SetDisplayName {
        name: String,
    },
    /// The Account tab's trainer card for the Verse world key, as 64 hex
    /// characters from the platform's protected store. `reveal` returns
    /// its secret too; the Trainer Key screen sends it only after the
    /// person asks to see the nsec and confirms a warning. `preview` shows
    /// the labeled tutorial fixture instead of reading the relay.
    Trainer {
        world_secret_hex: String,
        #[serde(default)]
        reveal: bool,
        #[serde(default)]
        preview: bool,
    },
    /// Publish the world key's trainer profile (NIP-XP `13193`), which the
    /// Trainer screen sends only after the person taps **Show my level** or
    /// **Hide my level** and confirms. The direct reply is the trainer
    /// packet.
    TrainerProfile {
        world_secret_hex: String,
        shown: bool,
    },
    /// Sign the trainer card and publish it at its address; the direct
    /// reply is the card as JSON and its link. The Trainer screen sends it
    /// only after the person taps **Export card** and confirms.
    TrainerExport {
        world_secret_hex: String,
    },
    /// Add a key to, or remove one from, the world key's trainer profile
    /// (NIP-XP key links) and publish it. The added key counts only after
    /// it signs a link back on its own device. The direct reply is the
    /// trainer packet.
    TrainerLink {
        world_secret_hex: String,
        #[serde(default)]
        add: Option<String>,
        #[serde(default)]
        remove: Option<String>,
    },
    /// Report a problem: the form for the tab and screen the tester is on.
    /// The direct reply is a draft packet, never the app packet.
    ReportDraft {
        tab: playtest::session::Tab,
        route: playtest::session::Route,
    },
    /// File the report on the form, signed by the Verse world key (64 hex
    /// characters from the platform's protected store). The direct reply
    /// is the reports packet.
    ReportSend {
        world_secret_hex: String,
        form: Box<crate::playtest::Form>,
    },
    /// **Give feedback** on selected text (#10127), signed by the Verse
    /// world key. The direct reply is the reports packet, whose `feedback`
    /// is what the dialog says.
    FeedbackSend {
        world_secret_hex: String,
        form: Box<crate::playtest::FeedbackForm>,
    },
    /// My reports and playtest logging's state. With the world key, reports
    /// that wait or failed are sent again.
    Reports {
        #[serde(default)]
        world_secret_hex: String,
    },
    /// Delete the playtest log.
    PlaytestClear,
    /// The tester moved to a tab and screen: recorded only in a build with
    /// playtest logging on.
    PlaytestScreen {
        tab: playtest::session::Tab,
        route: playtest::session::Route,
    },
    /// Start the Wallet tab's wallet with its seed from the platform's
    /// protected store: BIP39 entropy, 16 or 32 bytes as hex. Never logged
    /// or stored. `replace` follows a restore: it stops the running wallet
    /// and opens this one.
    WalletOpen {
        entropy_hex: String,
        #[serde(default)]
        replace: bool,
    },
    /// Sync the wallet with Spark, or retry a failed start.
    WalletRefresh,
    /// Make a Lightning invoice; an empty `amount` takes any amount.
    WalletInvoice {
        #[serde(default)]
        amount: String,
    },
    /// Quote a payment to a pasted or scanned request. `comment` goes to an
    /// LNURL recipient that takes one.
    WalletQuote {
        input: String,
        #[serde(default)]
        amount: String,
        #[serde(default)]
        comment: String,
    },
    /// Pay the quote on screen, which the person confirmed.
    WalletPay {
        quote: u64,
    },
    /// Close the send review or its result.
    WalletSendReset,
    /// Buy bitcoin with dollars: `provider` is `moonpay` or `cashapp`, and
    /// `amount` is typed in the app's amount format. The page to open
    /// arrives as `wallet_open_url`.
    WalletBuy {
        provider: String,
        amount: String,
    },
    /// Quote claiming a waiting on-chain deposit.
    WalletClaimQuote {
        txid: String,
        vout: u32,
    },
    /// Claim the quoted deposit at its quoted fee.
    WalletClaim {
        txid: String,
        vout: u32,
    },
    /// Close the deposit claim step.
    WalletClaimReset,
    /// Choose how fast a quoted on-chain withdrawal confirms: `slow`,
    /// `medium`, or `fast`.
    WalletSpeed {
        quote: u64,
        speed: String,
    },
    /// Publish this wallet's Spark address in this device's Nostr payment
    /// targets, or take it out. Only the person's setting sends it.
    WalletPublish {
        on: bool,
    },
    /// Save a Lightning address as a contact.
    WalletSaveContact {
        name: String,
        address: String,
    },
    /// Start refunding a waiting deposit: read the fee rates.
    WalletRefundStart {
        txid: String,
        vout: u32,
    },
    /// Review a refund to `address` at `speed`; nothing is sent.
    WalletRefundReview {
        txid: String,
        vout: u32,
        address: String,
        speed: String,
    },
    /// Send the reviewed refund, which the person confirmed.
    WalletRefund {
        txid: String,
        vout: u32,
    },
    /// Close the refund step.
    WalletRefundReset,
    /// The saved unilateral-exit state as a file, in the direct reply only.
    /// The host sends it when the person taps Export.
    WalletExitExport,
    /// The person read the wallet's trust note.
    WalletAcknowledge,
    /// The person wrote down the recovery words the words sheet showed; the
    /// Back up card goes away for this wallet.
    WalletWordsSaved,
    /// Open or close the Wallet's Advanced section, remembered on this phone.
    WalletAdvanced {
        open: bool,
    },
    /// The recovery words, in the direct reply only. The host sends it after
    /// the person asks to see them and confirms a warning.
    WalletWords,
    /// Check recovery words for a restore. The direct reply carries their
    /// entropy for the host's key store, or why they were refused.
    WalletRestoreCheck {
        words: String,
    },
    /// A tap on a Gym button: a chat card's, a sheet's, the Gym menu's, or
    /// the Gym intro's, by the ID the last packet gave it. An ID the last
    /// packet didn't carry does nothing.
    Gym {
        id: String,
    },
    /// **Train Coder**, from the Verse's Gym board or Account: opt into
    /// the Gym. The Chat tab shows the Gym intro, and `coder_go` is `chat`
    /// so the host switches to it.
    GymTrain,
    /// **Profile**, from Account: the Chat tab shows the Profile sheet, and
    /// `coder_go` is `chat` so the host switches to it.
    Profile,
    /// The phone's shell (#11126): the switch, the drawer, and the cards.
    Shell {
        shell: crate::coder_tab::ShellAction,
    },
    /// The trainer's Verse world key (64 hex characters from the platform's
    /// protected store): it names the trainer on the menu, reads their XP,
    /// and signs their hosted test requests. Kept in memory only.
    GymWorld {
        world_secret_hex: String,
    },
    /// Pay the agent's spend request on the approval sheet. The host sends
    /// it only after the owner's tap on Approve (and Face ID or the passcode
    /// when the sheet asks for it).
    SpendApprove {
        request: String,
    },
    /// Refuse the request on the sheet: `declined_by_owner`.
    SpendDeny {
        request: String,
    },
    /// Stop a computer's payment requests; its epoch advances.
    SpendBlock {
        host: String,
    },
    /// Let a blocked computer ask for payments again.
    SpendAllow {
        host: String,
    },
    /// Clear the last agent payment's notice.
    SpendDismiss,
    /// Send the wallet to the computer on the link sheet
    /// (`openagents wallet link`), sealed to its one-time key. The host
    /// sends it only after the owner's tap on Approve and Face ID or the
    /// passcode.
    WalletLinkApprove {
        host: String,
        id: String,
    },
    /// Decline the computer's ask for the wallet.
    WalletLinkDeny {
        host: String,
        id: String,
    },
    /// Clear the last wallet link notice.
    WalletLinkDismiss,
    /// Pay the request on the sheet and trust its payee: later payments to
    /// it from that computer, within the automatic ceilings, need no tap.
    /// The host sends it only after the owner's tap and Face ID or the
    /// passcode.
    SpendApproveTrust {
        request: String,
    },
    /// Stop paying a trusted payee without a tap.
    SpendUntrust {
        host: String,
        payee: String,
    },
    /// Stop every automatic payment for a computer.
    SpendManual {
        host: String,
    },
    /// The platform issued or reissued its push token: an APNs device token
    /// as lowercase hex, or an FCM registration token. Rust registers it
    /// with the push gateway and publishes the device's push lease.
    PushToken {
        token: String,
    },
    /// Revoke the push lease and forget the token at the gateway.
    PushDisable,
    /// Account > **Your keys** (BYOK, #10176): the keys the host read from
    /// its protected store at start, and the saved "Use my keys for
    /// everything" switch. Kept in memory only.
    ProviderKeys {
        #[serde(default)]
        keys: Vec<crate::provider_keys::Stored>,
        #[serde(default)]
        mine: bool,
    },
    /// A key the person entered in the secure field: tested in the
    /// background, and kept by the host only when the packet's
    /// `provider_keys.done` says so.
    ProviderKeyAdd {
        provider: String,
        key: String,
    },
    /// Test the stored key again.
    ProviderKeyTest {
        provider: String,
    },
    /// Remove the key; the host deletes it from its store first.
    ProviderKeyRemove {
        provider: String,
    },
    /// "Use my keys for everything", on or off.
    ProviderKeysMine {
        on: bool,
    },
    /// Show and read amounts app-wide as `bip177` (₿12,345) or `btc`
    /// (0.00012345 BTC). The choice is saved.
    AmountFormat {
        format: String,
    },
    /// The phone's appearance (UITraitCollection's user interface style,
    /// or the night bits of Android's uiMode), at launch and whenever it
    /// changes. System follows it.
    SystemAppearance {
        dark: bool,
    },
    /// Account > Appearance: `system`, `light`, or `dark`. The choice is
    /// saved.
    Theme {
        theme: String,
    },
}

/// The direct reply to [`Request::WalletWords`] and
/// [`Request::WalletRestoreCheck`]. It never reaches the app packet.
#[derive(Serialize)]
pub struct WalletSecretPacket {
    pub schema: &'static str,
    pub words: Option<Vec<String>>,
    pub entropy_hex: Option<String>,
    pub error: Option<String>,
}

/// The direct reply to [`Request::WalletExitExport`]. It never reaches the
/// app packet.
#[derive(Serialize)]
pub struct WalletFilePacket {
    pub schema: &'static str,
    pub file_name: Option<String>,
    pub text: Option<String>,
    pub error: Option<String>,
}

/// The unilateral-exit backup in the app's encrypted store. The store takes
/// items up to 192 KiB and an exit state grows with the wallet's leaves, so
/// it is kept in parts under an index written last.
struct ExitVault(Cache);

#[derive(Serialize, Deserialize)]
struct ExitIndex {
    saved_at: u64,
    parts: usize,
}

/// Characters per part, well inside the store's item bound.
const EXIT_PART: usize = 120 * 1024;
/// The most parts kept: about 7.5 MB.
const EXIT_PARTS: usize = 64;

impl crate::wallet::Vault for ExitVault {
    fn save(&self, saved: &crate::wallet::SavedExit) -> Result<(), String> {
        let chars: Vec<char> = saved.state.chars().collect();
        let parts: Vec<String> = chars
            .chunks(EXIT_PART)
            .map(|part| part.iter().collect())
            .collect();
        if parts.is_empty() || parts.len() > EXIT_PARTS {
            return Err("exit state size out of bounds".into());
        }
        for (index, part) in parts.iter().enumerate() {
            self.0.write(&format!("spark-exit-{index}"), part)?;
        }
        self.0.write(
            "spark-exit",
            &ExitIndex {
                saved_at: saved.saved_at,
                parts: parts.len(),
            },
        )
    }
    fn load(&self) -> Option<crate::wallet::SavedExit> {
        let index: ExitIndex = self.0.read("spark-exit").ok().flatten()?;
        if index.parts == 0 || index.parts > EXIT_PARTS {
            return None;
        }
        let mut state = String::new();
        for part in 0..index.parts {
            state.push_str(
                &self
                    .0
                    .read::<String>(&format!("spark-exit-{part}"))
                    .ok()??,
            );
        }
        Some(crate::wallet::SavedExit {
            state,
            saved_at: index.saved_at,
        })
    }
    fn clear(&self) {
        let _ = self.0.write("spark-exit", &Option::<ExitIndex>::None);
    }
}

impl Request {
    fn terminal(&self) -> bool {
        matches!(
            self,
            Self::TerminalPoll { .. }
                | Self::TerminalResize { .. }
                | Self::TerminalText { .. }
                | Self::TerminalKey { .. }
                | Self::TerminalPaste { .. }
        )
    }
}

/// The invitation QR code the Computers surface shows: one string of `1`
/// (dark) and `0` (light) per module row, quiet zone included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QrModules {
    pub size: usize,
    pub rows: Vec<String>,
}

#[derive(Serialize)]
pub struct Packet {
    pub schema: &'static str,
    /// This build shows the preview features (`crate::preview`).
    pub preview: bool,
    /// This device's public key in hex, for display.
    pub device: String,
    /// The same key in NIP-19 form (`npub1…`).
    pub device_npub: String,
    /// Coder's shared Computers screens, while they are past the list.
    pub computers: Option<serde_json::Value>,
    /// The native Computers list, while the shared screens are on it.
    pub computers_home: Option<Home>,
    /// A value the Computers surface asks the host to collect.
    pub computers_input: Option<InputRequest>,
    pub computers_qr: Option<QrModules>,
    pub coder: Option<serde_json::Value>,
    /// The phone's shell, when the host draws it (`Launch::shell`).
    pub shell: Option<crate::coder_tab::ShellView>,
    /// The link cards the chat shows, by surface resource (`link:…`): the
    /// link a tap opens, the title and site, and whether the card has a
    /// picture, read as that surface's image.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub links: std::collections::BTreeMap<String, openagents_chat_app::links::Card>,
    /// The open Coder chat changes on its own, as while its task runs: ask
    /// for a packet again soon.
    pub coder_live: bool,
    /// A basic Coder reply is streaming: ask for a packet every few hundred
    /// milliseconds.
    pub chat_streaming: bool,
    /// Show this screen of another tab, once: `computers` is Account >
    /// Computers.
    pub coder_go: Option<crate::coder_tab::Go>,
    /// The chat takes images (`coder_tab::ATTACHMENTS_ENABLED`, off as of
    /// #10093): the host mounts its photo picker and sends picked images
    /// only while this is set.
    pub attachments: bool,
    /// **Connect a computer** (`SCR-22`) or **Connected** (`SCR-23`), while
    /// it shows. The host draws the camera and the paste field.
    pub connect: Option<crate::connect::View>,
    /// The phone listens for computers on its Wi-Fi: the Android host
    /// holds its multicast lock exactly while this is set.
    pub nearby_listening: bool,
    pub tailnet: Option<serde_json::Value>,
    /// The Tailnet surface is reading in the background.
    pub tailnet_loading: bool,
    /// Open this URL in the browser, then send `tailnet_wait_for_sign_in`.
    pub open_url: Option<String>,
    /// A terminal screen is open; poll it with `terminal_poll`.
    pub terminal: bool,
    /// App-level problems, such as an unavailable host client.
    pub notices: Vec<String>,
    /// The Wallet tab's state.
    pub wallet: crate::wallet::Screen,
    /// The wallet is starting or syncing in the background.
    pub wallet_loading: bool,
    /// A bitcoin purchase page to open in the browser, once. Always `https`.
    pub wallet_open_url: Option<String>,
    /// Agents' payment requests: the approval sheet, the computers that may
    /// ask, and the payments they asked for.
    pub spend: crate::spend::View,
    /// A computer's ask for the wallet (`openagents wallet link`): the sheet
    /// that names it and its code.
    pub wallet_link: crate::wallet_link::View,
    /// How amounts show and are typed, app-wide.
    pub amounts: crate::amounts::AmountsView,
    /// The theme: the choice, the resolved scheme (the host's color
    /// scheme and status bar), and the palette for the host's own chrome.
    pub appearance: crate::appearance::View,
    /// Push wake status (`Wakes on`, `Wakes off`, or why not), once the
    /// build is configured for push or a push request arrived.
    pub push: Option<String>,
    /// The Gym in chat: which of the chat tab's screens shows (the main
    /// menu, the chat, or the first run), the cards the chat's surfaces
    /// name, the open sheet, and a share sheet to open.
    pub gym: crate::gym::View,
    /// Account > **Your keys** (BYOK): each provider's last four
    /// characters and state, the switch, and the status line. Never a key.
    pub provider_keys: crate::provider_keys::View,
}

/// The encrypted store for the Computers record, keyed by the device key.
struct ComputersStore(Cache);

impl Store for ComputersStore {
    fn load(&mut self) -> Result<Option<Saved>, String> {
        self.0.read("computers")
    }
    fn save(&mut self, saved: &Saved) -> Result<(), String> {
        self.0.write("computers", saved)
    }
}

struct TailnetState {
    screen: TailnetScreen,
    loading: bool,
    /// A new device list: look for OpenAgents hosts on it.
    probe: bool,
    /// Admission status by tailnet address.
    admits: BTreeMap<String, Admit>,
    /// Admissions to apply on the app thread: address, label, answer.
    answers: Vec<(String, String, Admission)>,
}

/// A device tailnet admission already added, by tailnet address.
#[derive(Clone, Serialize, Deserialize)]
struct Admitted {
    host: String,
    /// When its chat pairing ends; 0 when it serves none.
    chats_until: u64,
}

const ADMISSION_LIMIT: Duration = Duration::from_secs(5);

pub struct App {
    runtime: tokio::runtime::Runtime,
    /// The link cards' page reads in flight (`link_fetch`).
    link_reads: Arc<tokio::sync::Semaphore>,
    native_computers: bool,
    /// The device key. It leaves the app only through an explicit
    /// [`Request::Account`] reveal.
    secret: SecretKey,
    /// Where the app keeps its state, including the display name.
    state_dir: PathBuf,
    device: String,
    device_npub: String,
    computers: Option<Computers>,
    /// The computers' Coder chats, read through their history observers.
    chats: Chats,
    coder: CoderTab,
    terminals: Option<Terminals>,
    terminal: Option<Terminal>,
    tailnet_client: Result<Arc<Client>, String>,
    tailnet: Arc<Mutex<TailnetState>>,
    tailnet_revision: u64,
    tailnet_view: Option<ValidatedView<TailnetIntent>>,
    /// Records of what tailnet admission added, keyed by address.
    admissions: Result<Cache, String>,
    wallet: crate::wallet::Wallet,
    spend: crate::spend::Spending,
    /// How agent spend requests reach the computers; `None` without the
    /// live client.
    spend_transport: Option<Arc<dyn crate::spend::Transport>>,
    /// Computers' asks for the wallet, and how they are answered.
    wallet_link: crate::wallet_link::Linking,
    link_transport: Option<Arc<dyn crate::wallet_link::Transport>>,
    /// The owner's private Verse placements, kept in step with the paired
    /// computers (`crate::verse_private`).
    verse_private: crate::verse_private::Syncing,
    verse_private_transport: Option<Arc<dyn crate::verse_private::Transport>>,
    /// Chat invitations asked of computers paired without a current one.
    chat_invites: crate::chat_invites::Invites,
    /// How those asks reach the computers; `None` without the live client.
    chat_asker: Option<Arc<dyn crate::chat_invites::Asker>>,
    trainer: crate::trainer::Trainer,
    /// Report a problem, My reports, and the playtest log.
    playtest: crate::playtest::Playtest,
    /// The amount format, applied to every surface that shows bitcoin.
    amounts: crate::amounts::Amounts,
    /// The theme: the person's choice and the phone's appearance.
    appearance: crate::appearance::Appearance,
    /// Push wakes, when this build is configured for them.
    push: Option<coder_mobile::Push>,
    push_status: Option<String>,
    notices: Vec<String>,
    /// The trainer's world key, once the host handed it over.
    world: Option<SecretKey>,
    /// Connect a computer (`SCR-22`, `SCR-23`).
    connect: crate::connect::Connect,
    /// The person's own model provider keys (BYOK), in memory only.
    provider_keys: crate::provider_keys::ProviderKeys,
    /// How a provider key is tested.
    key_check: Arc<dyn crate::provider_keys::Check>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, String> {
        Self::open(config, Launch::default())
    }

    pub fn open(config: Config, launch: Launch) -> Result<Self, String> {
        let secret = SecretKey::from_str(&config.secret_hex).map_err(|_| "invalid device key")?;
        let (device, device_npub) = crate::account::public(&secret);
        // Both ring and aws-lc-rs are linked; TLS clients that use the
        // process default need one chosen. An earlier install is kept.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let mut notices = vec![];
        let mut terminals = None;
        let mut pairing = None;
        let handle = runtime.handle().clone();
        let iroh_secret = launch.iroh_secret_hex.as_deref().and_then(secret_bytes);
        // A phone never runs a host; it reaches hosts through the live
        // client, which keeps its grants in their own encrypted store.
        // Debug builds only: a release build has no Computers fixture.
        #[cfg(debug_assertions)]
        let fixture: Option<Box<dyn coder_computers::ComputersService + Send>> =
            (launch.computers_fixture || fixture_requested()).then(|| {
                Box::new(coder_computers::synthetic::Synthetic::fixture(
                    Platform::Phone,
                    now,
                )) as Box<dyn coder_computers::ComputersService + Send>
            });
        #[cfg(not(debug_assertions))]
        let fixture: Option<Box<dyn coder_computers::ComputersService + Send>> = None;
        let service: Box<dyn coder_computers::ComputersService + Send> =
            if let Some(fixture) = fixture {
                fixture
            } else {
                match Cache::open(&config.state_dir.join("computers"), &secret).and_then(|cache| {
                    let mut settings = Settings::new(Platform::Phone);
                    settings.now = now;
                    settings.iroh_secret = iroh_secret;
                    Live::open(
                        settings,
                        secret,
                        Box::new(ComputersStore(cache)),
                        runtime.handle().clone(),
                    )
                    .map_err(|error| coder_computers::describe(&error))
                }) {
                    Ok(live) => {
                        terminals = Some(live.terminals());
                        pairing = Some(live.pairing());
                        // A summary, a catch-up, or a connection change
                        // shows at once instead of on the host's timer.
                        live.on_change(Arc::new(crate::wake::computers));
                        Box::new(live)
                    }
                    Err(reason) => {
                        notices.push(format!("Computers unavailable: {reason}"));
                        Box::new(coder_computers::Unavailable::new(
                            device.clone(),
                            LocalHost::NotSupported,
                            now,
                        ))
                    }
                }
            };
        let computers = match Computers::new(service, CAPABILITIES, format!("computers:{}", id())) {
            Ok(mut computers) => {
                // Hosts arrive through tailnet admission or an invitation;
                // the phone skips Coder's first-run page.
                let _ = computers.finish_first_run();
                Some(computers)
            }
            Err(error) => {
                notices.push(format!("Computers unavailable: {}", error.message));
                None
            }
        };
        let chats = Chats::new(
            runtime.handle().clone(),
            secret,
            Cache::open(&config.state_dir.join("chats"), &secret),
        );
        // The phone shows only Coder's chats now; the transcripts the
        // all-harness Chats list kept are not read again.
        let _ = std::fs::remove_dir_all(config.state_dir.join("chats-transcripts"));
        let admissions = Cache::open(&config.state_dir.join("admissions"), &secret);
        // Chats read directly at each admitted computer's tailnet listener,
        // including pairings made before the phone remembered its address.
        let mut chats = chats;
        let admitted: BTreeMap<String, Admitted> = admissions
            .as_ref()
            .ok()
            .and_then(|cache| cache.read("admitted").ok().flatten())
            .unwrap_or_default();
        for (address, admitted) in &admitted {
            if let Ok(ip) = address.parse::<std::net::Ipv4Addr>() {
                chats.set_direct(
                    &admitted.host,
                    std::net::SocketAddr::from((ip, coder_host::tailnet::PORT)),
                );
            }
        }
        let tailnet_client = Client::open(&config.state_dir.join("tailscale")).map(Arc::new);
        let spend_transport = terminals.clone().map(|terminals| {
            Arc::new(crate::spend::Live::new(terminals, runtime.handle().clone()))
                as Arc<dyn crate::spend::Transport>
        });
        let link_transport = terminals.clone().map(|terminals| {
            Arc::new(crate::wallet_link::Live::new(
                terminals,
                runtime.handle().clone(),
            )) as Arc<dyn crate::wallet_link::Transport>
        });
        let verse_private_transport = terminals.clone().map(|terminals| {
            Arc::new(crate::verse_private::Live::new(
                terminals,
                runtime.handle().clone(),
            )) as Arc<dyn crate::verse_private::Transport>
        });
        // The Verse tab's Everglade reads the owner's private placements
        // from here.
        crate::verse_private::set_home(&config.state_dir);
        let chat_asker = terminals.clone().map(|terminals| {
            Arc::new(crate::chat_invites::Live::new(
                terminals,
                runtime.handle().clone(),
            )) as Arc<dyn crate::chat_invites::Asker>
        });
        // Each paired computer's own threads, read on its current link.
        let thread_link = terminals.clone().map(|terminals| {
            Arc::new(openagents_chat_app::host_threads::Live::new(
                terminals,
                runtime.handle().clone(),
            )) as Arc<dyn openagents_chat_app::host_threads::Link>
        });
        let remote_cli = terminals.clone().map(|terminals| {
            Arc::new(crate::cli_run::Live::new(
                terminals,
                runtime.handle().clone(),
            )) as Arc<dyn crate::cli_run::RemoteCli>
        });
        let spend = crate::spend::Spending::new(
            device.clone(),
            Cache::open(&config.state_dir.join("spend"), &secret).ok(),
        );
        // The Spark wallet replaced the Mutinynet test wallet, whose store
        // held only signet test coins; remove it. Its Keychain item goes too.
        let _ = std::fs::remove_dir_all(config.state_dir.join("wallet"));
        let amounts = crate::amounts::Amounts::open(&config.state_dir);
        // The theme seam: sets the shared views' scheme before any view
        // is built.
        let appearance = crate::appearance::Appearance::open(&config.state_dir);
        spend.set_format(amounts.format());
        // The recorded Gym keeps its chats and runs apart, so its fixture
        // numbers never show in the real app's results.
        let recorded = if launch.gym_fixture && cfg!(debug_assertions) {
            "-fixture"
        } else {
            ""
        };
        let basic = crate::basic_chats::BasicChats::new(
            Some(runtime.handle().clone()),
            basic_door(&launch, secret),
            Cache::open(
                &config.state_dir.join(format!("basic-chats{recorded}")),
                &secret,
            )
            .ok(),
        )
        .with_wake(Arc::new(crate::wake::ring));
        let mut gym = crate::gym::Gym::new(
            Cache::open(&config.state_dir.join(format!("gym{recorded}")), &secret).ok(),
            hosted_runner(&launch, secret),
            Some(runtime.handle().clone()),
        );
        // The release gate (`crate::preview`): outside a preview build the
        // Gym never shows, whatever an earlier build saved or a launch
        // option asks.
        if !crate::preview::ON {
            gym.hide();
        }
        if cfg!(debug_assertions)
            && let Some(step) = launch.gym_first_run.as_deref()
        {
            gym.set_start(step);
        }
        // Loopback relay and gateway URLs are for simulator tests only.
        let (push, push_status) = match launch.push {
            None => (None, None),
            Some(settings) => match coder_mobile::Push::open(
                settings,
                &config.state_dir,
                &secret,
                cfg!(debug_assertions),
            ) {
                Ok(push) => {
                    let status = push.status.clone();
                    (Some(push), Some(status))
                }
                Err(reason) => (None, Some(format!("Wakes unavailable: {reason}"))),
            },
        };
        Ok(Self {
            runtime,
            link_reads: Arc::new(tokio::sync::Semaphore::new(crate::link_fetch::AT_ONCE)),
            native_computers: launch.native_computers,
            secret,
            state_dir: config.state_dir.clone(),
            device,
            device_npub: device_npub.clone(),
            computers,
            chats,
            coder: CoderTab::new(format!("coder:{}", id()))
                .with_gym(gym)
                .with_app_build(launch.app_build.clone())
                .with_remote_cli(remote_cli)
                .with_script(launch.chat_script.clone())
                .with_pulled_transcripts(launch.pulled_transcripts)
                .with_shell(launch.shell)
                .with_basic(basic)
                .with_threads(
                    openagents_chat_app::host_threads::HostThreads::new(Arc::new(
                        crate::wake::ring,
                    ))
                    .with_cache(Cache::open(&config.state_dir.join("host-threads"), &secret).ok()),
                    thread_link,
                )
                .with_list(crate::coder_list::Store::open(
                    Cache::open(&config.state_dir.join("coder-list"), &secret).ok(),
                ))
                .with_outbox(crate::outbox::Outbox::open(
                    Cache::open(&config.state_dir.join("coder-outbox"), &secret).ok(),
                ))
                .with_transcripts(crate::transcripts::Transcripts::open(
                    Cache::open(&config.state_dir.join("coder-transcripts"), &secret).ok(),
                )),
            terminals,
            terminal: None,
            tailnet_client,
            tailnet: Arc::new(Mutex::new(TailnetState {
                screen: TailnetScreen::Loading,
                loading: false,
                probe: false,
                admits: BTreeMap::new(),
                answers: vec![],
            })),
            admissions,
            tailnet_revision: 0,
            tailnet_view: None,
            wallet: {
                // The fixture keeps its own folders, so it never touches the
                // real wallet's caches or exit backup.
                // Debug builds only: a release build has no wallet fixture.
                #[cfg(debug_assertions)]
                let (home, exit, opener, directory): (
                    _,
                    _,
                    _,
                    Arc<dyn crate::payees::Directory>,
                ) = if launch.wallet_fixture {
                    (
                        "spark-fixture",
                        "spark-fixture-exit",
                        crate::wallet_fixture::opener(),
                        Arc::new(crate::wallet_fixture::Directory),
                    )
                } else {
                    (
                        "spark",
                        "spark-exit",
                        crate::wallet::spark_opener(),
                        Arc::new(crate::payees::NostrDirectory::new(secret)),
                    )
                };
                #[cfg(not(debug_assertions))]
                let (home, exit, opener, directory): (
                    _,
                    _,
                    _,
                    Arc<dyn crate::payees::Directory>,
                ) = (
                    "spark",
                    "spark-exit",
                    crate::wallet::spark_opener(),
                    Arc::new(crate::payees::NostrDirectory::new(secret)),
                );
                let mut wallet = crate::wallet::Wallet::new(config.state_dir.join(home), opener)
                    .with_directory(directory, device_npub.clone());
                wallet.set_format(amounts.format());
                match Cache::open(&config.state_dir.join(exit), &secret) {
                    Ok(cache) => wallet.with_vault(Arc::new(ExitVault(cache))),
                    Err(_) => wallet,
                }
            },
            spend,
            spend_transport,
            wallet_link: crate::wallet_link::Linking::default(),
            link_transport,
            verse_private: crate::verse_private::Syncing::default(),
            verse_private_transport,
            chat_invites: crate::chat_invites::Invites::default(),
            chat_asker,
            trainer: crate::trainer::Trainer::default(),
            playtest: crate::playtest::Playtest::live(
                Cache::open(&config.state_dir.join("playtest"), &secret).ok(),
            ),
            amounts,
            appearance,
            push,
            push_status,
            notices,
            world: None,
            // Tests share the process: only the app installs who pays.
            provider_keys: crate::provider_keys::ProviderKeys::new(!cfg!(test), crate::wake::ring),
            key_check: Arc::new(crate::provider_keys::Https),
            connect: crate::connect::Connect::new(
                pairing
                    .clone()
                    .map(|pairing| Arc::new(LivePair(pairing)) as Arc<dyn crate::connect::Pair>),
                Some(handle),
            )
            .with_nearby(
                pairing.map(|pairing| Arc::new(pairing) as Arc<dyn crate::nearby::Finder>),
            ),
        })
    }

    /// Attach an image the host's photo picker read to the open draft, and
    /// answer with the app packet. The shared attachments code decodes and
    /// bounds it (PNG or JPEG, 8 MiB, 4096 pixels a side, four per draft);
    /// a refusal shows as the chat's notice. While phone attachments are
    /// off (#10093) the image is dropped quietly and the packet is
    /// unchanged.
    pub fn attach_image(&mut self, name: &str, bytes: Vec<u8>) -> Vec<u8> {
        self.coder.attach_image(name, bytes);
        serde_json::to_vec(&self.call(Request::Snapshot)).unwrap_or_default()
    }

    /// The encoded bytes an `image:{id}` surface in the chat shows, which
    /// the attachments code already decoded and bounded; empty when the
    /// open draft has no such image.
    pub fn image(&self, resource: &str) -> Vec<u8> {
        // A link card's picture, already re-encoded (`links::card_image`).
        if resource.starts_with("link:") {
            return self
                .coder
                .link_previews()
                .image(resource)
                .map(|image| image.as_ref().clone())
                .unwrap_or_default();
        }
        self.coder
            .image(resource)
            .map(|image| image.bytes.as_ref().clone())
            .unwrap_or_default()
    }

    /// The supervised link to the paired computer `host` and the runtime
    /// that runs its calls, for Everglade's studio (`crate::studio`).
    /// `None` without the live Computers client.
    pub(crate) fn studio_links(
        &self,
        host: &str,
    ) -> Option<(
        coder_computers::terminal::session::Links,
        tokio::runtime::Handle,
    )> {
        let links = self.terminals.as_ref()?.links(host);
        Some((links, self.runtime.handle().clone()))
    }

    /// Answer one request as JSON: a terminal request with the terminal
    /// packet, anything else with the app packet.
    pub fn respond(&mut self, request: Request) -> Vec<u8> {
        if request.terminal() {
            let packet = self.terminal_request(request);
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        if let Request::Account { reveal } = request {
            let name = crate::account::load_display_name(&self.state_dir);
            let packet = crate::account::packet(&self.secret, reveal, name);
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        if let Request::SetDisplayName { name } = request {
            let saved = crate::account::save_display_name(&self.state_dir, &name).unwrap_or(None);
            let packet = crate::account::packet(&self.secret, false, saved);
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        if let Request::Trainer {
            world_secret_hex,
            reveal,
            preview,
        } = request
        {
            return match self.trainer.packet(
                &world_secret_hex,
                reveal,
                preview && cfg!(debug_assertions),
            ) {
                Ok(packet) => serde_json::to_vec(&packet).unwrap_or_default(),
                Err(error) => serde_json::to_vec(&serde_json::json!({
                    "schema": "openagents.trainer.v1",
                    "error": error,
                }))
                .unwrap_or_default(),
            };
        }
        if let Request::TrainerProfile {
            world_secret_hex,
            shown,
        } = request
        {
            let packet = self
                .trainer
                .set_profile(&world_secret_hex, Some(shown), None)
                .or_else(|error| self.trainer.refuse(error))
                .and_then(|()| self.trainer.packet(&world_secret_hex, false, false));
            return match packet {
                Ok(packet) => serde_json::to_vec(&packet).unwrap_or_default(),
                Err(error) => serde_json::to_vec(&serde_json::json!({
                    "schema": "openagents.trainer.v1",
                    "error": error,
                }))
                .unwrap_or_default(),
            };
        }
        if let Request::TrainerExport { world_secret_hex } = request {
            let reply = self
                .trainer
                .export(&world_secret_hex)
                .unwrap_or_else(|error| crate::trainer::CardExport {
                    schema: "openagents.trainer-card.v1",
                    file_name: None,
                    json: None,
                    link: None,
                    preview: false,
                    error: Some(error),
                });
            return serde_json::to_vec(&reply).unwrap_or_default();
        }
        if let Request::TrainerLink {
            world_secret_hex,
            add,
            remove,
        } = request
        {
            let packet = self
                .trainer
                .set_link(&world_secret_hex, add.as_deref(), remove.as_deref())
                .or_else(|error| self.trainer.refuse(error))
                .and_then(|()| self.trainer.packet(&world_secret_hex, false, false));
            return match packet {
                Ok(packet) => serde_json::to_vec(&packet).unwrap_or_default(),
                Err(error) => serde_json::to_vec(&serde_json::json!({
                    "schema": "openagents.trainer.v1",
                    "error": error,
                }))
                .unwrap_or_default(),
            };
        }
        if let Some(bytes) = self.playtest_request(&request) {
            return bytes;
        }
        if matches!(
            request,
            Request::WalletWords | Request::WalletRestoreCheck { .. }
        ) {
            let packet = self.wallet_secret(request);
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        if matches!(request, Request::WalletExitExport) {
            let mut packet = WalletFilePacket {
                schema: "openagents.wallet-file.v1",
                file_name: None,
                text: None,
                error: None,
            };
            match self.wallet.exit_export() {
                Ok((name, text)) => {
                    packet.file_name = Some(name);
                    packet.text = Some(text);
                }
                Err(error) => packet.error = Some(error),
            }
            return serde_json::to_vec(&packet).unwrap_or_default();
        }
        let packet = self.call(request);
        serde_json::to_vec(&packet).unwrap_or_default()
    }

    /// The screen as Rust knows it: the Coder tab's first screen is an
    /// open chat while one is open.
    fn place(
        &self,
        tab: playtest::session::Tab,
        route: playtest::session::Route,
    ) -> playtest::session::Route {
        use playtest::session::{Route, Tab};
        if tab != Tab::Coder || route != Route::Home {
            return route;
        }
        if let Some(sheet) = &self.coder.gym.sheet {
            return match sheet {
                crate::gym::Sheet::Result { .. } => Route::Result,
                crate::gym::Sheet::Publish { .. } => Route::Publish,
                crate::gym::Sheet::TestSet(_) => Route::TestSet,
                crate::gym::Sheet::LevelUp { .. } => Route::LevelUp,
                crate::gym::Sheet::Profile => Route::Profile,
                crate::gym::Sheet::Stop { .. } => Route::Chat,
            };
        }
        match crate::first_run::screen(&self.coder.gym) {
            "menu" => Route::Menu,
            "first_run" => Route::FirstRun,
            _ if self.coder.in_chat() => Route::Chat,
            _ => route,
        }
    }

    /// Answers a Report a problem or playtest log request with its
    /// direct reply, or hands any other request back.
    fn playtest_request(&mut self, request: &Request) -> Option<Vec<u8>> {
        let packet = match *request {
            Request::ReportDraft { tab, route } => {
                let route = self.place(tab, route);
                let task = self.coder.open_task().map(|(_, task)| task);
                let chat = self.coder.shared_chat();
                return Some(
                    serde_json::to_vec(&self.playtest.draft(tab, route, task, chat.as_ref()))
                        .unwrap_or_default(),
                );
            }
            Request::ReportSend {
                ref world_secret_hex,
                ref form,
            } => match SecretKey::from_str(world_secret_hex) {
                Ok(world) => {
                    let task = self.coder.open_task().map(|(_, task)| task);
                    let chat = self.coder.shared_chat();
                    let platform = crate::playtest::platform(std::env::consts::OS);
                    self.playtest
                        .send((**form).clone(), &world, task, chat, platform)
                }
                Err(_) => self
                    .playtest
                    .refuse("Your Verse world key couldn't be read."),
            },
            Request::FeedbackSend {
                ref world_secret_hex,
                ref form,
            } => match SecretKey::from_str(world_secret_hex) {
                Ok(world) => {
                    let form = (**form).clone();
                    let route = self.place(form.tab, form.route);
                    let selection = self
                        .coder
                        .feedback_selection(&form.text, form.row.as_deref());
                    let platform = crate::playtest::platform(std::env::consts::OS);
                    self.playtest.feedback(
                        crate::playtest::FeedbackForm { route, ..form },
                        selection,
                        &world,
                        platform,
                    )
                }
                Err(_) => self
                    .playtest
                    .refuse("Your Verse world key couldn't be read."),
            },
            Request::Reports {
                ref world_secret_hex,
            } => {
                let world = SecretKey::from_str(world_secret_hex).ok();
                self.playtest.reports(world.as_ref())
            }
            Request::PlaytestClear => {
                self.playtest.clear_log();
                self.playtest.reports(None)
            }
            _ => return None,
        };
        Some(serde_json::to_vec(&packet).unwrap_or_default())
    }

    pub fn call(&mut self, request: Request) -> Packet {
        self.admit();
        let mut open_url = None;
        match request {
            Request::Snapshot => {}
            Request::Changed => {
                if crate::wake::take_computers()
                    && let Some(computers) = self.computers.as_mut()
                {
                    // A value the person is typing stays until it is done.
                    if computers.input().is_none() {
                        let _ = computers.refresh();
                    } else {
                        crate::wake::keep_computers();
                    }
                }
            }
            Request::Lifecycle { active } => {
                crate::wake::set_active(active);
                self.connect.set_active(active);
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.set_active(active);
                }
                self.playtest.lifecycle(active);
                self.coder.lifecycle(active);
                if !active {
                    self.trainer.pause();
                }
                if active {
                    // Opened from a wake or brought back: read the
                    // computers' payment requests now.
                    self.spend.soon();
                    self.wallet_link.soon();
                    self.wallet.refresh_if_open();
                    self.chats.refresh();
                    self.chats.warm();
                    self.load_tailnet(None, TAILNET_REFRESH_LIMIT);
                }
            }
            Request::ComputersActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                if let Some(computers) = self.computers.as_mut()
                    && computers.activate(&event) == Ok(Outcome::Terminal)
                {
                    self.open_terminal();
                }
            }
            Request::ComputersInput { token, value } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.submit(&token, &value);
                }
            }
            Request::ComputersCancel { token } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.cancel_input(&token);
                }
            }
            Request::ConnectOpen => self.connect.open(),
            Request::ConnectCode { value } => self.connect.code(&value),
            Request::ConnectLink { value } => self.connect.link(&value),
            Request::ConnectClose => self.connect.close(),
            Request::ConnectNearby { id } => self.connect.nearby(&id),
            Request::ComputersRefresh => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers.refresh();
                }
            }
            // A refusal shows as the list's or the screen's notice.
            Request::ComputersOpen { host } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers_home::open(computers, &host);
                }
            }
            Request::ComputersChoose { host, choice } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers_home::choose(computers, &host, choice);
                }
            }
            Request::ComputersGo { to } => {
                if let Some(computers) = self.computers.as_mut() {
                    let _ = computers_home::go(computers, to);
                }
            }
            Request::CoderActivate {
                instance,
                revision,
                node,
            } => self.coder.activate(
                &Activation {
                    instance,
                    revision,
                    node,
                },
                self.computers.as_mut(),
                &mut self.chats,
            ),
            Request::CoderInput { token, value } => {
                self.coder
                    .submit(&token, &value, self.computers.as_mut(), &mut self.chats)
            }
            Request::TailnetActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                let intent = self
                    .tailnet_view
                    .as_ref()
                    .and_then(|view| view.activate(&event).ok())
                    .copied();
                match intent {
                    Some(TailnetIntent::SignIn) => {
                        if let TailnetScreen::SignIn(url) = &self.lock_tailnet().screen {
                            open_url = Some(url.to_string());
                        }
                    }
                    Some(TailnetIntent::Refresh) => self.load_tailnet(None, TAILNET_REFRESH_LIMIT),
                    None => {}
                }
            }
            Request::TailnetRefresh => self.load_tailnet(None, TAILNET_REFRESH_LIMIT),
            Request::TailnetWaitForSignIn => {
                let url = match &self.lock_tailnet().screen {
                    TailnetScreen::SignIn(url) => Some(url.clone()),
                    _ => None,
                };
                if let Some(url) = url {
                    self.load_tailnet(Some(url), TAILNET_SIGN_IN_LIMIT);
                }
            }
            Request::TerminalActivate {
                instance,
                revision,
                node,
            } => {
                let event = Activation {
                    instance,
                    revision,
                    node,
                };
                if let Some(terminal) = self.terminal.as_mut()
                    && terminal.activate(&event) == Ok(TerminalOutcome::Closed)
                {
                    self.terminal = None;
                }
            }
            Request::TerminalPoll { .. }
            | Request::TerminalResize { .. }
            | Request::TerminalText { .. }
            | Request::TerminalKey { .. }
            | Request::TerminalPaste { .. } => {
                let _ = self.terminal_request(request);
            }
            // `respond` answers it with the account packet; the app packet
            // never carries the secret key.
            Request::Account { .. }
            | Request::SetDisplayName { .. }
            | Request::Trainer { .. }
            | Request::TrainerProfile { .. }
            | Request::TrainerLink { .. }
            | Request::TrainerExport { .. }
            | Request::ReportDraft { .. }
            | Request::ReportSend { .. }
            | Request::FeedbackSend { .. }
            | Request::Reports { .. }
            | Request::PlaytestClear => {}
            Request::PlaytestScreen { tab, route } => {
                // The Coder tab shows: open its computers' connections now,
                // so its first read or send pays none.
                if tab == playtest::session::Tab::Coder {
                    self.chats.warm();
                }
                self.coder.show(tab == playtest::session::Tab::Coder);
                let route = self.place(tab, route);
                self.playtest.screen(tab, route);
            }
            Request::WalletOpen {
                entropy_hex,
                replace,
            } => self.wallet.open(&entropy_hex, replace),
            Request::WalletRefresh => self.wallet.refresh(),
            Request::WalletInvoice { amount } => self.wallet.invoice(&amount),
            Request::WalletQuote {
                input,
                amount,
                comment,
            } => self.wallet.quote(&input, &amount, &comment),
            Request::WalletPay { quote } => self.wallet.pay(quote),
            Request::WalletSendReset => self.wallet.reset_send(),
            Request::WalletBuy { provider, amount } => self.wallet.buy(&provider, &amount),
            Request::WalletClaimQuote { txid, vout } => self.wallet.claim_quote(&txid, vout),
            Request::WalletClaim { txid, vout } => self.wallet.claim(&txid, vout),
            Request::WalletClaimReset => self.wallet.claim_reset(),
            Request::WalletSpeed { quote, speed } => self.wallet.speed(quote, &speed),
            Request::WalletRefundStart { txid, vout } => self.wallet.refund_start(&txid, vout),
            Request::WalletRefundReview {
                txid,
                vout,
                address,
                speed,
            } => self.wallet.refund_review(&txid, vout, &address, &speed),
            Request::WalletRefund { txid, vout } => self.wallet.refund(&txid, vout),
            Request::WalletRefundReset => self.wallet.refund_reset(),
            Request::WalletPublish { on } => self.wallet.publish(on),
            Request::WalletSaveContact { name, address } => {
                self.wallet.save_contact(&name, &address)
            }
            Request::WalletAcknowledge => self.wallet.acknowledge(),
            Request::WalletWordsSaved => self.wallet.words_saved(),
            Request::WalletAdvanced { open } => self.wallet.set_advanced(open),
            Request::AmountFormat { format } => {
                if let Some(format) = self.amounts.choose(&format) {
                    self.wallet.set_format(format);
                    self.spend.set_format(format);
                }
            }
            // The packet below rebuilds every Rust Native view in the
            // resolved scheme.
            Request::SystemAppearance { dark } => {
                self.appearance.set_system(dark);
            }
            Request::Theme { theme } => {
                self.appearance.choose(&theme);
            }
            // `respond` answers these directly; the app packet never
            // carries recovery words or a seed.
            Request::WalletWords
            | Request::WalletRestoreCheck { .. }
            | Request::WalletExitExport => {}
            Request::SpendApprove { request } => {
                self.spend
                    .approve(&request, self.payer(), self.spend_transport.clone())
            }
            Request::SpendDeny { request } => self.spend.deny(&request),
            Request::SpendBlock { host } => self.spend.block(&host),
            Request::SpendAllow { host } => self.spend.allow(&host),
            Request::SpendDismiss => self.spend.dismiss(),
            Request::WalletLinkApprove { host, id } => {
                let wallet = &self.wallet;
                self.wallet_link.approve(
                    &host,
                    &id,
                    |key| wallet.seal_for(key),
                    self.link_transport.clone(),
                );
            }
            Request::WalletLinkDeny { host, id } => {
                self.wallet_link
                    .deny(&host, &id, self.link_transport.clone());
            }
            Request::WalletLinkDismiss => self.wallet_link.dismiss(),
            Request::SpendApproveTrust { request } => {
                self.spend
                    .approve_and_trust(&request, self.payer(), self.spend_transport.clone())
            }
            Request::SpendUntrust { host, payee } => self.spend.untrust(&host, &payee),
            Request::SpendManual { host } => self.spend.manual(&host),
            Request::Gym { id } => {
                self.coder
                    .gym_tap(&id, self.computers.as_mut(), &mut self.chats);
            }
            Request::GymTrain => self.coder.train_coder(),
            Request::Profile => self.coder.show_profile(),
            Request::Shell { shell } => {
                self.coder
                    .shell(shell, self.computers.as_mut(), &mut self.chats)
            }
            Request::GymWorld { world_secret_hex } => {
                if let Ok(world) = SecretKey::from_str(&world_secret_hex) {
                    self.world = Some(world);
                    self.coder.gym.set_world(world);
                }
            }
            Request::ProviderKeys { keys, mine } => self.provider_keys.load(keys, mine),
            Request::ProviderKeyAdd { provider, key } => {
                self.provider_keys
                    .add(&provider, key, &self.key_check, self.runtime.handle())
            }
            Request::ProviderKeyTest { provider } => {
                self.provider_keys
                    .test(&provider, &self.key_check, self.runtime.handle());
            }
            Request::ProviderKeyRemove { provider } => self.provider_keys.remove(&provider),
            Request::ProviderKeysMine { on } => self.provider_keys.set_mine(on),
            Request::PushToken { token } => self.push_token(Some(&token)),
            Request::PushDisable => self.push_token(None),
        }
        self.packet(open_url)
    }

    /// The direct reply for the recovery words or a restore check.
    pub fn wallet_secret(&self, request: Request) -> WalletSecretPacket {
        let mut packet = WalletSecretPacket {
            schema: "openagents.wallet-secret.v1",
            words: None,
            entropy_hex: None,
            error: None,
        };
        match request {
            Request::WalletWords => match self.wallet.words() {
                Some(words) => packet.words = Some(words),
                None => packet.error = Some("The wallet hasn't opened yet.".into()),
            },
            Request::WalletRestoreCheck { words } => match crate::wallet::restore_entropy(&words) {
                Ok(entropy) => packet.entropy_hex = Some(entropy),
                Err(error) => packet.error = Some(error),
            },
            _ => {}
        }
        packet
    }

    fn open_terminal(&mut self) {
        let Some(computers) = self.computers.as_mut() else {
            return;
        };
        let Some(host) = computers.take_terminal() else {
            return;
        };
        let label = computers
            .snapshot()
            .host(&host)
            .map_or_else(|| "this computer".to_owned(), |record| record.label.clone());
        match Terminal::open(
            host,
            label,
            self.terminals.clone(),
            self.runtime.handle().clone(),
        ) {
            Ok(terminal) => self.terminal = Some(terminal),
            Err(error) => self.notices.push(format!("Terminal unavailable: {error}")),
        }
    }

    fn terminal_request(&mut self, request: Request) -> TerminalPacket {
        let mut packet = self.terminal_packet(request);
        packet.view = packet.view.map(neutral);
        packet
    }

    fn terminal_packet(&mut self, request: Request) -> TerminalPacket {
        let Some(terminal) = self.terminal.as_mut() else {
            return TerminalPacket::closed();
        };
        let mut known = None;
        match request {
            Request::TerminalPoll { known: have } => known = have,
            Request::TerminalResize { rows, cols } => terminal.resize(rows, cols),
            Request::TerminalText { text } => terminal.text(&text),
            Request::TerminalKey {
                key,
                ctrl,
                alt,
                shift,
            } => terminal.key(&key, coder_vt::Modifiers { ctrl, alt, shift }),
            Request::TerminalPaste { text } => terminal.paste(&text),
            _ => {}
        }
        terminal.packet(known)
    }

    fn lock_tailnet(&self) -> std::sync::MutexGuard<'_, TailnetState> {
        self.tailnet
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Read the tailnet on the runtime. A read already in flight wins.
    fn load_tailnet(&mut self, followup: Option<Url>, limit: Duration) {
        let client = match &self.tailnet_client {
            Ok(client) => client.clone(),
            Err(error) => {
                self.lock_tailnet().screen = TailnetScreen::Failed(error.clone());
                return;
            }
        };
        {
            let mut state = self.lock_tailnet();
            if state.loading {
                return;
            }
            state.loading = true;
        }
        let shared = self.tailnet.clone();
        let read = self
            .runtime
            .spawn(async move { client.devices(followup, limit).await });
        self.runtime.spawn(async move {
            // A panicked read still ends loading, with an error to retry.
            let screen = match read.await {
                Ok(Ok(TailnetOutcome::Devices(tailnet))) => TailnetScreen::Devices(tailnet),
                Ok(Ok(TailnetOutcome::SignIn(url))) => TailnetScreen::SignIn(url),
                Ok(Err(error)) => TailnetScreen::Failed(error),
                Err(_) => TailnetScreen::Failed("Couldn't read your tailnet. Try again.".into()),
            };
            let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
            state.probe = matches!(screen, TailnetScreen::Devices(_));
            state.screen = screen;
            state.loading = false;
        });
    }

    /// Tailnet admission (NIP-HOST): ask each device on the tailnet for
    /// invitations, then redeem them here. Tailscale only identifies this
    /// phone to the host; each host still signs the grant it issues.
    fn admit(&mut self) {
        let (candidates, answers) = {
            let mut state = self.lock_tailnet();
            let answers = std::mem::take(&mut state.answers);
            let mut candidates = vec![];
            if std::mem::take(&mut state.probe)
                && let TailnetScreen::Devices(tailnet) = &state.screen
            {
                candidates = tailnet
                    .devices
                    .iter()
                    .filter(|device| device.online != Some(false))
                    .filter_map(|device| {
                        let ip: std::net::Ipv4Addr = device.address.parse().ok()?;
                        Some((device.address.clone(), device.name.clone(), ip))
                    })
                    .collect::<Vec<_>>();
                for (address, _, _) in &candidates {
                    state.admits.insert(address.clone(), Admit::Checking);
                }
            }
            (candidates, answers)
        };
        let known: BTreeMap<String, Admitted> = self
            .admissions
            .as_ref()
            .ok()
            .and_then(|cache| cache.read("admitted").ok().flatten())
            .unwrap_or_default();
        let hosts: Vec<String> = self
            .computers
            .as_ref()
            .map(|computers| {
                computers
                    .snapshot()
                    .hosts
                    .iter()
                    .map(|h| h.key.clone())
                    .collect()
            })
            .unwrap_or_default();
        let soon = now().saturating_add(24 * 60 * 60);
        for (address, name, ip) in candidates {
            // Skip a device that is already added with current chats.
            if let Some(admitted) = known.get(&address)
                && hosts.contains(&admitted.host)
                && admitted.chats_until > soon
                && self.chats.linked(&admitted.host, soon)
            {
                self.lock_tailnet().admits.insert(address, Admit::Connected);
                continue;
            }
            let shared = self.tailnet.clone();
            self.runtime.spawn(async move {
                let target = std::net::SocketAddr::from((ip, coder_host::tailnet::PORT));
                let answer = coder_host::tailnet::request(target, true, ADMISSION_LIMIT).await;
                let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                match answer {
                    Ok(admission) => state.answers.push((address, name, admission)),
                    Err(_) => {
                        state.admits.insert(address, Admit::NotRunning);
                    }
                }
            });
        }
        if answers.is_empty() {
            return;
        }
        let mut known = known;
        for (address, name, admission) in answers {
            let status = match (&admission.refused, &admission.invitation) {
                (Some(code), _) => Admit::Refused(code.clone()),
                (None, Some(invitation)) => {
                    let label = if admission.label.is_empty() {
                        name
                    } else {
                        admission.label.clone()
                    };
                    let added = hosts.contains(&admission.host)
                        || self
                            .computers
                            .as_mut()
                            .is_some_and(|computers| computers.admit(invitation, &label).is_ok());
                    let direct = address
                        .parse::<std::net::Ipv4Addr>()
                        .ok()
                        .map(|ip| std::net::SocketAddr::from((ip, coder_host::tailnet::PORT)));
                    if let Some(chats) = admission.chats.clone() {
                        self.chat_invites.got(&admission.host);
                        self.chats
                            .pair(chats, label, admission.host.clone(), direct);
                    }
                    if added {
                        known.insert(
                            address.clone(),
                            Admitted {
                                host: admission.host.clone(),
                                chats_until: if admission.chats.is_some() {
                                    now().saturating_add(29 * 24 * 60 * 60)
                                } else {
                                    0
                                },
                            },
                        );
                        Admit::Connected
                    } else {
                        Admit::Refused("unavailable".into())
                    }
                }
                (None, None) => Admit::Refused("malformed".into()),
            };
            self.lock_tailnet().admits.insert(address, status);
        }
        if let Ok(cache) = &self.admissions {
            let _ = cache.write("admitted", &known);
        }
    }

    fn render_tailnet(&mut self) -> Option<serde_json::Value> {
        self.tailnet_revision += 1;
        let root = {
            let state = self.lock_tailnet();
            tailnet_view::root(&state.screen, &state.admits)
        };
        let view = View::new("openagents.tailnet", self.tailnet_revision, root)
            .validate()
            .ok()?;
        let value = serde_json::to_value(view.view()).ok();
        self.tailnet_view = Some(view);
        value
    }

    /// Pair the chat invitations that arrived, and ask each computer this
    /// phone may observe, and holds no chat pairing for that lasts another
    /// day, for one: a computer paired nearby or on the relay, whose answer
    /// carried none, or one whose chat grant is near its end.
    fn renew_chats(&mut self) {
        for (host, label, invitation) in self.chat_invites.take() {
            self.chats.pair(invitation, label, host, None);
        }
        let (Some(asker), Some(computers)) = (self.chat_asker.clone(), &self.computers) else {
            return;
        };
        let soon = now().saturating_add(24 * 60 * 60);
        let snapshot = computers.snapshot();
        let chats = &self.chats;
        let wanted = crate::chat_invites::wanted(
            snapshot
                .hosts
                .iter()
                .map(|host| (host.key.as_str(), host.label.as_str(), &host.enrollment)),
            |host| chats.linked(host, soon),
        );
        if !wanted.is_empty() {
            self.chat_invites.ask(wanted, asker);
        }
    }

    /// Read agents' spend requests from the computers this phone may
    /// operate, in the background.
    fn poll_spends(&mut self) {
        let (Some(transport), Some(computers)) = (self.spend_transport.clone(), &self.computers)
        else {
            return;
        };
        let snapshot = computers.snapshot();
        let hosts: Vec<(String, String)> = snapshot
            .hosts
            .iter()
            // The controller's snapshot can be older than the live links,
            // so a computer that is not connected is left to its
            // transport, which fails at once and waits a little.
            .filter(|record| {
                matches!(
                    &record.enrollment,
                    coder_computers::Enrollment::Enrolled { rights, .. }
                        if rights.contains(coder_host::access::Right::Operate)
                )
            })
            .map(|record| (record.key.clone(), record.label.clone()))
            .collect();
        if let Some(links) = self.link_transport.clone() {
            self.wallet_link.poll(hosts.clone(), links);
        }
        self.spend.poll(hosts, transport, self.payer());
    }

    /// Ask the computers this phone may observe for the owner's private
    /// Verse placements, in the background, once the world key is here.
    fn sync_verse_private(&mut self) {
        let (Some(transport), Some(computers), Some(world), Some(home)) = (
            self.verse_private_transport.clone(),
            &self.computers,
            self.world,
            crate::verse_private::home(),
        ) else {
            return;
        };
        let Ok(identity) = verse::identity::Identity::from_secret("phone", world) else {
            return;
        };
        let hosts: Vec<String> = computers
            .snapshot()
            .hosts
            .iter()
            .filter(|record| {
                matches!(
                    &record.enrollment,
                    coder_computers::Enrollment::Enrolled { rights, .. }
                        if rights.contains(coder_host::access::Right::Observe)
                )
            })
            .map(|record| record.key.clone())
            .collect();
        self.verse_private
            .poll(home, identity.signer.pubkey().to_owned(), hosts, transport);
    }

    /// Register `token` for wakes, or with `None` revoke the lease.
    fn push_token(&mut self, token: Option<&str>) {
        let Some(push) = self.push.as_mut() else {
            if self.push_status.is_none() {
                self.push_status = Some("Wakes are off in this build.".into());
            }
            return;
        };
        let result = match token {
            Some(token) => push.token(&self.runtime, &self.secret, &self.device, token, now()),
            None => push.disable(&self.runtime, &self.secret, &self.device, now()),
        };
        self.push_status = Some(match result {
            Ok(()) => push.status.clone(),
            Err(reason) => format!("{}: {reason}", push.status),
        });
    }

    /// The running wallet as agent spending's payer.
    fn payer(&self) -> Option<Arc<dyn crate::spend::Payer>> {
        self.wallet
            .node()
            .map(|node| Arc::new(crate::spend::NodePayer(node)) as Arc<dyn crate::spend::Payer>)
    }

    fn packet(&mut self, open_url: Option<String>) -> Packet {
        self.chats.settle();
        // A computer just connected: list it, and send Run Coder there.
        if let Some(paired) = self.connect.poll() {
            if let Some(computers) = self.computers.as_mut() {
                let _ = computers.refresh();
            }
            // Its Coder chats, so a task started there reads back here.
            if let Some(chats) = paired.chats.clone() {
                self.chat_invites.got(&paired.host);
                self.chats
                    .pair(chats, paired.label.clone(), paired.host.clone(), None);
            }
            self.coder.prefer(paired.host);
            // A new computer may hold the owner's private placements, and
            // notes this phone's world key for the owner to grant.
            self.verse_private.soon();
        }
        self.renew_chats();
        self.poll_spends();
        self.sync_verse_private();
        let tailnet = self.render_tailnet();
        // Chat commands that waited for their computer try again.
        self.coder.flush(self.computers.as_mut());
        // A coding reply to a message sent here starts Coder at once where
        // the computer allows it (#10101).
        self.coder
            .start_offered(self.computers.as_mut(), &mut self.chats);
        if let Some(world) = self.world {
            self.coder.gym.standing = self.trainer.standing(&world);
        }
        let coder = self.coder.render(self.computers.as_ref(), &mut self.chats);
        let shell = self.coder.shell_view(self.computers.as_ref(), &self.chats);
        let previews = self.coder.link_previews();
        crate::link_fetch::fetch_wanted(&previews, self.runtime.handle(), &self.link_reads);
        let links = previews.shown();
        let gym = self.coder.gym_view();
        for code in self.coder.gym.take_logged() {
            self.playtest.event(code);
        }
        let computers = self.computers.as_ref();
        let computers_home = computers
            .filter(|_| self.native_computers)
            .and_then(|c| computers_home::home(c, CAPABILITIES));
        let native = self.native_computers;
        let wallet = self.wallet.screen();
        self.playtest.observe(
            !self.notices.is_empty(),
            match &wallet {
                crate::wallet::Screen::Failed { .. } => true,
                crate::wallet::Screen::Ready(summary) => summary.error.is_some(),
            },
            self.coder.notice_shown(),
        );
        let coder_live = self.coder.live(self.computers.as_ref())
            || previews.pending()
            || self.spend.live()
            || self.wallet_link.live();
        crate::wake::set_live(coder_live);
        Packet {
            schema: "openagents.mobile.v1",
            preview: crate::preview::ON,
            device: self.device.clone(),
            device_npub: self.device_npub.clone(),
            // The host's navigation replaces the shared screens' tab row and
            // a computer's back and refresh row, and a phone never runs a
            // local host.
            computers: computers
                .filter(|_| computers_home.is_none())
                .and_then(Computers::view)
                .and_then(|view| serde_json::to_value(view.view()).ok())
                .map(|view| {
                    if native {
                        framed(neutral(without(
                            view,
                            &["directory", "tabs", "host-end", "local"],
                        )))
                    } else {
                        neutral(without(view, &["directory"]))
                    }
                }),
            computers_home,
            computers_input: computers.and_then(Computers::input).cloned(),
            computers_qr: computers
                .and_then(Computers::invitation_qr)
                .map(|modules| QrModules {
                    size: modules.len(),
                    rows: modules
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|dark| if *dark { '1' } else { '0' })
                                .collect()
                        })
                        .collect(),
                }),
            coder,
            shell,
            links,
            // A payment request on the sheet keeps packets coming too.
            coder_live,
            chat_streaming: self.coder.streaming(),
            attachments: self.coder.attachments_enabled(),
            coder_go: match self.coder.take_go() {
                // The scanner is this app's own screen.
                Some(crate::coder_tab::Go::Connect) => {
                    self.connect.open();
                    None
                }
                go => go,
            },
            // After `coder_go`, which may have opened it.
            connect: self.connect.view(),
            nearby_listening: self.connect.listening(),
            tailnet,
            tailnet_loading: {
                let state = self.lock_tailnet();
                state.loading || state.admits.values().any(|a| *a == Admit::Checking)
            },
            open_url,
            terminal: self.terminal.is_some(),
            notices: self.notices.clone(),
            wallet,
            wallet_loading: self.wallet.loading(),
            wallet_open_url: self.wallet.take_open_url(),
            spend: self.spend.view(),
            wallet_link: self.wallet_link.view(),
            push: self.push_status.clone(),
            amounts: self.amounts.view(),
            appearance: self.appearance.view(),
            gym,
            provider_keys: self.provider_keys.view(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_tailnet(&mut self, screen: TailnetScreen) {
        let mut state = self.lock_tailnet();
        state.probe = matches!(screen, TailnetScreen::Devices(_));
        state.screen = screen;
    }

    #[cfg(test)]
    pub(crate) fn admit_for_test(&mut self, invitation: &str, label: &str) {
        self.computers
            .as_mut()
            .expect("computers")
            .admit(invitation, label)
            .expect("admitted");
    }

    /// The host and task of the open Coder chat.
    #[cfg(test)]
    pub(crate) fn open_coder_task(&self) -> Option<(String, String)> {
        self.coder.open_task()
    }

    /// Archive a live test's task once it has ended, so the test leaves
    /// nothing in the owner's lists. It waits up to `limit` for the task to
    /// end, stopping it first when it still runs.
    #[cfg(test)]
    pub(crate) fn archive_task_for_test(
        &mut self,
        (host, task): (String, String),
        limit: Duration,
    ) -> Result<(), String> {
        let computers = self.computers.as_mut().ok_or("no computers")?;
        let deadline = std::time::Instant::now() + limit;
        let mut stopped = false;
        loop {
            match computers.archive_task(&host, &task) {
                Ok(()) => return Ok(()),
                // `conflict`: the task has not ended yet.
                Err(_) if std::time::Instant::now() < deadline => {
                    if !stopped {
                        let revision = computers
                            .snapshot()
                            .activity
                            .iter()
                            .filter(|s| s.host == host && s.subject == task)
                            .map(|s| s.sequence)
                            .max();
                        if let Some(revision) = revision {
                            stopped = computers.stop_task(&host, &task, revision).is_ok();
                        }
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
                Err(refusal) => return Err(format!("{refusal:?}")),
            }
        }
    }

    /// Whether a current chat pairing is linked to Computers host `host`.
    #[cfg(test)]
    pub(crate) fn chats_linked(&self, host: &str) -> bool {
        self.chats.linked(host, now())
    }

    #[cfg(test)]
    pub(crate) fn hosts(&self) -> Vec<String> {
        self.computers
            .as_ref()
            .map(|c| c.snapshot().hosts.iter().map(|h| h.key.clone()).collect())
            .unwrap_or_default()
    }
}

/// Leave out sections this app does not offer, by node key: the owner
/// directory, which asks for an owner secret key. Activations still resolve
/// against the controller's full view.
fn without(mut view: serde_json::Value, keys: &[&str]) -> serde_json::Value {
    let mut pending = vec![&mut view];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(serde_json::Value::Array(children)) = object
                    .get_mut("element")
                    .and_then(|element| element.get_mut("props"))
                    .and_then(|props| props.get_mut("children"))
                {
                    children.retain(|child| {
                        !child["key"].as_str().is_some_and(|key| keys.contains(&key))
                    });
                }
                pending.extend(object.values_mut());
            }
            serde_json::Value::Array(items) => pending.extend(items.iter_mut()),
            _ => {}
        }
    }
    view
}

/// Draw Coder's shared screens in OpenAgents' neutral palette: each color
/// keeps its brightness (the red channel of Coder's amber) as a gray. In
/// the light look the gray is inverted, so light ink on a dark fill becomes
/// dark ink on a light fill (#11028).
fn neutral(mut view: serde_json::Value) -> serde_json::Value {
    let light = crate::appearance::light();
    let mut pending = vec![&mut view];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(serde_json::Value::Object(style)) = object.get_mut("style") {
                    for field in ["foreground", "background"] {
                        if let Some(serde_json::Value::Object(color)) = style.get_mut(field)
                            && let Some(red) = color.get("red").and_then(serde_json::Value::as_u64)
                        {
                            let gray = if light { 255 - red.min(255) } else { red };
                            for channel in ["red", "green", "blue"] {
                                color.insert(channel.into(), gray.into());
                            }
                        }
                    }
                }
                pending.extend(object.values_mut());
            }
            serde_json::Value::Array(items) => pending.extend(items.iter_mut()),
            _ => {}
        }
    }
    view
}

/// Draw the shared screens on the app's black with its page margins.
fn framed(mut view: serde_json::Value) -> serde_json::Value {
    if let Some(style) = view["root"]["style"].as_object_mut() {
        style.remove("background");
        for side in ["padding_start", "padding_end"] {
            style.insert(side.into(), "md".into());
        }
        style.insert("padding_top".into(), "sm".into());
        style.insert("gap".into(), "md".into());
    }
    view
}

/// The live service's pairing, as the Connect screen's [`Pair`].
///
/// [`Pair`]: crate::connect::Pair
struct LivePair(coder_computers::live::Pairing);

impl crate::connect::Pair for LivePair {
    fn pair(&self, code: String) -> crate::connect::Pairing {
        let pairing = self.0.clone();
        Box::pin(async move { pairing.pair(&code).await })
    }
}

/// A 32-byte secret from 64 hex characters.
fn secret_bytes(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(bytes)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn id() -> String {
    use secp256k1::rand::RngCore;
    let mut bytes = [0u8; 16];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod exit_vault_tests {
    use super::*;
    use crate::wallet::{SavedExit, Vault};

    #[test]
    fn a_large_exit_state_round_trips_encrypted_in_parts_and_clears() {
        let dir = tempfile::tempdir().expect("temp dir");
        let secret = SecretKey::from_byte_array([3; 32]).expect("key");
        let vault = ExitVault(Cache::open(dir.path(), &secret).expect("cache"));
        assert_eq!(vault.load(), None);
        // Larger than one store item, with multibyte text at part edges.
        let state: String = std::iter::repeat_n("é{\"leaf\":\"0a1b\"}", 30_000).collect();
        let saved = SavedExit {
            state: state.clone(),
            saved_at: 1_790_000_000,
        };
        vault.save(&saved).expect("saved");
        assert!(std::fs::read_dir(dir.path()).unwrap().count() > 3);
        assert_eq!(vault.load(), Some(saved));
        // Nothing on disk is the plaintext. The ciphertext is base64, which
        // spells a bare `leaf` by chance in a few percent of runs; a quote
        // is outside its alphabet, so only plaintext can hold `"leaf"`.
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("\"leaf\""));
        }
        vault.clear();
        assert_eq!(vault.load(), None);
        let too_big = SavedExit {
            state: "x".repeat(EXIT_PART * EXIT_PARTS + 1),
            saved_at: 1,
        };
        assert!(vault.save(&too_big).is_err());
    }
}
