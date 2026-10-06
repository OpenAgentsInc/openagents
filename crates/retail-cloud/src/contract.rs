//! The v1 retail classes as data (`docs/cloud/retail-contract.md`): the
//! request a customer sends, its bounds, and the effects, recipients, and
//! payer every `retail-repo-change-v1` admission carries.

use route_contract::snapshot::{
    Access, CommandScope, ContentClass, Effects, Network, OsDenySet, Payer, Platform, ReadScope,
    Recipient, RecipientKind, WriteScope,
};
use serde::{Deserialize, Serialize};

use crate::authority::{ADMISSION_SCHEMA, CONTRACT, RetailAdmission, Source};

pub const COMPUTER_CLASS: &str = "retail-boat-large-v1";
pub const TASK_CLASS: &str = "retail-repo-change-v1";
/// The most task text a request carries.
pub const TASK_TEXT_MAX: usize = 16 * 1024;
pub const CHECKS_MIN: usize = 1;
pub const CHECKS_MAX: usize = 8;
pub const CHECK_COMMAND_MAX: usize = 1024;
/// Wall time per check.
pub const CHECK_SECS_MAX: u64 = 15 * 60;
/// Wall time per task.
pub const TASK_SECS_MAX: u64 = 3_600;
/// Retail sandboxes at once, across all customers.
pub const SANDBOXES_MAX: usize = 4;
/// Retained artifacts are kept this long.
pub const RETENTION_DAYS: u64 = 30;

/// What a customer asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRequest {
    pub source: Source,
    pub task: String,
    /// Check commands, frozen before the candidate exists.
    pub checks: Vec<String>,
    /// The wall-time limit the customer chose, at most an hour.
    pub max_seconds: u64,
    /// The customer's ceiling in sats, when they set one.
    pub ceiling_sats: Option<u64>,
}

/// Why a request is outside the v1 task class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsupported {
    /// Not a public `github.com` repository at a 40-digit commit.
    Source,
    /// Empty, or longer than 16 KiB.
    TaskText,
    /// Fewer than 1 or more than 8 checks, or one over 1,024 bytes.
    Checks,
    /// No seconds, or more than an hour.
    WallTime,
}

impl TaskRequest {
    /// Checks the request against the v1 task class.
    ///
    /// # Errors
    ///
    /// The first [`Unsupported`] part.
    pub fn check(&self) -> Result<(), Unsupported> {
        if !self.source.supported() {
            return Err(Unsupported::Source);
        }
        if self.task.trim().is_empty() || self.task.len() > TASK_TEXT_MAX {
            return Err(Unsupported::TaskText);
        }
        if self.checks.len() < CHECKS_MIN
            || self.checks.len() > CHECKS_MAX
            || self
                .checks
                .iter()
                .any(|c| c.trim().is_empty() || c.len() > CHECK_COMMAND_MAX)
        {
            return Err(Unsupported::Checks);
        }
        if self.max_seconds == 0 || self.max_seconds > TASK_SECS_MAX {
            return Err(Unsupported::WallTime);
        }
        Ok(())
    }

    /// The digest of the task text and the checks, which the admission
    /// binds.
    #[must_use]
    pub fn digest(&self) -> String {
        route_contract::digest_of(&(&self.task, &self.checks)).to_string()
    }
}

/// The effects every v1 task runs under: writes in the clone, the engine's
/// provider and package registries on the network, the engine's tools in
/// Linux `bubblewrap`, and no publication.
#[must_use]
pub fn effects() -> Effects {
    Effects {
        reads: vec![ReadScope::Workspace, ReadScope::Toolchains],
        writes: WriteScope::Workspace,
        network: Network::Destinations {
            hosts: vec![
                "api.openai.com".into(),
                "github.com".into(),
                "crates.io".into(),
                "static.crates.io".into(),
                "index.crates.io".into(),
                "registry.npmjs.org".into(),
                "pypi.org".into(),
                "files.pythonhosted.org".into(),
            ],
        },
        commands: CommandScope::EngineTools,
        publication: Vec::new(),
        access: Access::Toolchains,
        os_deny: OsDenySet {
            platform: Platform::Linux,
            locations: Vec::new(),
            app_control: true,
        },
    }
}

/// Who receives material: the customer's OpenAI key's provider, and
/// OpenAgents' sandbox.
#[must_use]
pub fn recipients() -> Vec<Recipient> {
    vec![
        Recipient {
            kind: RecipientKind::ModelProvider,
            id: "openai".into(),
        },
        Recipient {
            kind: RecipientKind::OpenAgents,
            id: COMPUTER_CLASS.into(),
        },
    ]
}

/// What the recipients see.
#[must_use]
pub fn disclosed() -> Vec<ContentClass> {
    vec![
        ContentClass::Message,
        ContentClass::RepositorySource,
        ContentClass::CommandOutput,
    ]
}

/// The model payer: the customer's own OpenAI key.
#[must_use]
pub fn model_payer() -> Payer {
    Payer::CallerKey {
        provider: "openai".into(),
    }
}

/// The admission for `request` on `account` as `execution`, priced by
/// `quote`.
#[must_use]
pub fn admission(
    account: &str,
    execution: &str,
    request: &TaskRequest,
    quote: &route_contract::price_book::Quote,
    grant_generation: u64,
) -> RetailAdmission {
    RetailAdmission {
        schema: ADMISSION_SCHEMA.into(),
        contract: CONTRACT.into(),
        account: account.into(),
        execution: execution.into(),
        source: request.source.clone(),
        request: request.digest(),
        computer_class: quote.computer.clone(),
        task_class: quote.task.clone(),
        grant_generation,
        effects: effects(),
        recipients: recipients(),
        disclosed: disclosed(),
        model_payer: model_payer(),
        price_book: quote.version.clone(),
        price_book_digest: quote.book.clone(),
        max_charge_sats: quote.max_sats,
    }
}

/// The published price book, `retail-2026-10-06.1`.
///
/// # Panics
///
/// Never: the fixture is checked in and tested.
#[must_use]
pub fn price_book() -> route_contract::price_book::PriceBook {
    #[derive(Deserialize)]
    struct Fixture {
        book: route_contract::price_book::PriceBook,
    }
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../route-contract/fixtures/price-book-v1.json"
    ))
    .expect("the checked-in price book parses");
    fixture.book
}
