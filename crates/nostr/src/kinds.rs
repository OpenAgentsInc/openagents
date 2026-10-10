//! The OpenAgents event-kind registry: every kind an OpenAgents NIP under
//! `nips/openagents` claims, listed once, with the one specification that
//! owns it.
//!
//! This is the single source for OpenAgents kind numbers. The protocol
//! modules take their kind constants from here, and the "Kind registry"
//! table in `nips/openagents/README.md` lists the same claims. The tests
//! refuse a kind claimed twice, a README table that disagrees with
//! [`REGISTRY`], a kind a NIP declares without registering it, a
//! registered kind no NIP declares, a kind-table row that restates another
//! NIP's kind without naming its owner, and a claim on a kind the official
//! NIP list already assigns. Kinds that OpenAgents specifications only use
//! (official NIP kinds such as `1985`, or Block kinds such as `24200`) are
//! not claims and don't appear here.
//!
//! These are draft assignments, not upstream registrations.

/// One claimed kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    /// The kind number.
    pub kind: u16,
    /// The owning specification's file stem under `nips/openagents`, such as
    /// `NIP-EVAL`, or `contracts` for the shared contracts.
    pub owner: &'static str,
    /// What an event of this kind is.
    pub meaning: &'static str,
}

/// NIP-CJ conversation job request.
pub const CJ_CONVERSATION_REQUEST: u16 = 25_900;
/// NIP-CJ conversation job result.
pub const CJ_CONVERSATION_RESULT: u16 = 26_900;
/// NIP-CJ conversation job feedback.
pub const CJ_CONVERSATION_FEEDBACK: u16 = 27_000;
/// NIP-DEC decision job request or cancel (the NIP-CJ decision family).
pub const CJ_DECISION_REQUEST: u16 = 25_910;
/// NIP-DEC decision job result.
pub const CJ_DECISION_RESULT: u16 = 26_910;
/// NIP-DEC decision job status.
pub const CJ_DECISION_FEEDBACK: u16 = 27_010;
/// NIP-CJ execution request or control.
pub const CJ_EXECUTION_REQUEST: u16 = 25_920;
/// NIP-CJ execution result or control answer.
pub const CJ_EXECUTION_RESULT: u16 = 26_920;
/// NIP-CJ execution admission and progress.
pub const CJ_EXECUTION_FEEDBACK: u16 = 27_020;
/// NIP-CAP capability discovery head.
pub const CAP_DISCOVERY: u16 = 30_180;
/// NIP-CAP operator preference head.
pub const CAP_PREFERENCE: u16 = 30_181;
/// NIP-PRG program discovery head.
pub const PRG_DISCOVERY: u16 = 30_182;
/// NIP-PRG module announcement.
pub const PRG_MODULE: u16 = 30_183;
/// NIP-EXT immutable release declaration.
pub const EXT_RELEASE: u16 = 3_184;
/// NIP-EXT release revocation.
pub const EXT_REVOCATION: u16 = 3_185;
/// NIP-EXT namespace migration attestation.
pub const EXT_MIGRATION: u16 = 3_186;
/// NIP-EXT package listing.
pub const EXT_LISTING: u16 = 30_184;
/// NIP-EXT revocation checkpoint.
pub const EXT_CHECKPOINT: u16 = 30_185;
/// NIP-RUN encrypted durable run record.
pub const RUN_RECORD: u16 = 3_187;
/// NIP-RUN encrypted current-head hint.
pub const RUN_HEAD: u16 = 30_186;
/// The shared private artifact envelope.
pub const PRIVATE_ARTIFACT: u16 = 3_188;
/// NIP-EVAL public evaluation declaration.
pub const EVAL_DECLARATION: u16 = 3_189;
/// NIP-EVAL Gym results publication.
pub const EVAL_GYM_RESULTS: u16 = 3_195;
/// NIP-KB immutable entry version.
pub const KB_ENTRY: u16 = 3_190;
/// NIP-KB entry withdrawal.
pub const KB_WITHDRAWAL: u16 = 3_191;
/// NIP-KB current entry head.
pub const KB_HEAD: u16 = 30_190;
/// NIP-MKT immutable public offering.
pub const MKT_OFFERING: u16 = 3_192;
/// NIP-MKT current offering head.
pub const MKT_HEAD: u16 = 30_192;
/// NIP-XP award.
pub const XP_AWARD: u16 = 3_193;
/// NIP-XP award revocation.
pub const XP_REVOCATION: u16 = 3_194;
/// NIP-XP playtest session record.
pub const XP_PLAYTEST_SESSION: u16 = 3_196;
/// NIP-XP content-free playtest report.
pub const XP_PLAYTEST_REPORT: u16 = 3_197;
/// NIP-ATIF public trajectory declaration.
pub const ATIF_DECLARATION: u16 = 3_198;
/// NIP-ATIF public trajectory chunk.
pub const ATIF_CHUNK: u16 = 3_199;
/// NIP-PYLON service receipt.
pub const PYLON_RECEIPT: u16 = 3_201;
/// NIP-ATT attested workload release.
pub const ATT_RELEASE: u16 = 3_202;
/// NIP-ATT release head.
pub const ATT_HEAD: u16 = 30_202;
/// NIP-ATT attested endpoint.
pub const ATT_ENDPOINT: u16 = 30_203;
/// NIP-PYLON pylon beacon.
pub const PYLON_BEACON: u16 = 30_200;
/// NIP-PYLON pool aggregate.
pub const PYLON_POOL: u16 = 30_201;
/// NIP-XP frozen quest version.
pub const XP_QUEST: u16 = 30_193;
/// NIP-XP trainer profile.
pub const XP_PROFILE: u16 = 13_193;
/// NIP-XP key link.
pub const XP_LINK: u16 = 13_195;
/// NIP-XP trainer card.
pub const XP_CARD: u16 = 30_194;
/// NIP-MV pose frame.
pub const MV_FRAME: u16 = 23_300;
/// NIP-MV gesture.
pub const MV_GESTURE: u16 = 23_301;
/// NIP-MV zone command.
pub const MV_COMMAND: u16 = 23_302;
/// NIP-MV world definition.
pub const MV_WORLD: u16 = 33_300;
/// NIP-MV entity state.
pub const MV_STATE: u16 = 33_301;

/// Every OpenAgents kind claim, in kind order.
pub const REGISTRY: &[Claim] = &[
    claim(EXT_RELEASE, "NIP-EXT", "Immutable release declaration"),
    claim(EXT_REVOCATION, "NIP-EXT", "Release revocation"),
    claim(EXT_MIGRATION, "NIP-EXT", "Namespace migration attestation"),
    claim(RUN_RECORD, "NIP-RUN", "Encrypted durable run record"),
    claim(PRIVATE_ARTIFACT, "contracts", "Private artifact envelope"),
    claim(
        EVAL_DECLARATION,
        "NIP-EVAL",
        "Public evaluation declaration",
    ),
    claim(KB_ENTRY, "NIP-KB", "Immutable entry version"),
    claim(KB_WITHDRAWAL, "NIP-KB", "Entry withdrawal"),
    claim(MKT_OFFERING, "NIP-MKT", "Immutable public offering"),
    claim(XP_AWARD, "NIP-XP", "Award"),
    claim(XP_REVOCATION, "NIP-XP", "Award revocation"),
    claim(EVAL_GYM_RESULTS, "NIP-EVAL", "Gym results publication"),
    claim(XP_PLAYTEST_SESSION, "NIP-XP", "Playtest session record"),
    claim(XP_PLAYTEST_REPORT, "NIP-XP", "Content-free playtest report"),
    claim(
        ATIF_DECLARATION,
        "NIP-ATIF",
        "Public trajectory declaration",
    ),
    claim(ATIF_CHUNK, "NIP-ATIF", "Public trajectory chunk"),
    claim(PYLON_RECEIPT, "NIP-PYLON", "Service receipt"),
    claim(ATT_RELEASE, "NIP-ATT", "Attested workload release"),
    claim(XP_PROFILE, "NIP-XP", "Trainer profile"),
    claim(XP_LINK, "NIP-XP", "Key link"),
    claim(MV_FRAME, "NIP-MV", "Pose frame"),
    claim(MV_GESTURE, "NIP-MV", "Gesture"),
    claim(MV_COMMAND, "NIP-MV", "Zone command"),
    claim(
        CJ_CONVERSATION_REQUEST,
        "NIP-CJ",
        "Conversation job request",
    ),
    claim(
        CJ_DECISION_REQUEST,
        "NIP-DEC",
        "Decision job request or cancel",
    ),
    claim(
        CJ_EXECUTION_REQUEST,
        "NIP-CJ",
        "Execution request or control",
    ),
    claim(CJ_CONVERSATION_RESULT, "NIP-CJ", "Conversation job result"),
    claim(CJ_DECISION_RESULT, "NIP-DEC", "Decision job result"),
    claim(
        CJ_EXECUTION_RESULT,
        "NIP-CJ",
        "Execution result or control answer",
    ),
    claim(
        CJ_CONVERSATION_FEEDBACK,
        "NIP-CJ",
        "Conversation job feedback",
    ),
    claim(CJ_DECISION_FEEDBACK, "NIP-DEC", "Decision job status"),
    claim(
        CJ_EXECUTION_FEEDBACK,
        "NIP-CJ",
        "Execution admission and progress",
    ),
    claim(CAP_DISCOVERY, "NIP-CAP", "Capability discovery head"),
    claim(CAP_PREFERENCE, "NIP-CAP", "Operator preference head"),
    claim(PRG_DISCOVERY, "NIP-PRG", "Program discovery head"),
    claim(PRG_MODULE, "NIP-PRG", "Module announcement"),
    claim(EXT_LISTING, "NIP-EXT", "Package listing"),
    claim(EXT_CHECKPOINT, "NIP-EXT", "Revocation checkpoint"),
    claim(RUN_HEAD, "NIP-RUN", "Encrypted current-head hint"),
    claim(KB_HEAD, "NIP-KB", "Current entry head"),
    claim(MKT_HEAD, "NIP-MKT", "Current offering head"),
    claim(XP_QUEST, "NIP-XP", "Frozen quest version"),
    claim(XP_CARD, "NIP-XP", "Trainer card"),
    claim(PYLON_BEACON, "NIP-PYLON", "Pylon beacon"),
    claim(PYLON_POOL, "NIP-PYLON", "Pool aggregate"),
    claim(ATT_HEAD, "NIP-ATT", "Release head"),
    claim(ATT_ENDPOINT, "NIP-ATT", "Attested endpoint"),
    claim(MV_WORLD, "NIP-MV", "World definition"),
    claim(MV_STATE, "NIP-MV", "Entity state"),
];

const fn claim(kind: u16, owner: &'static str, meaning: &'static str) -> Claim {
    Claim {
        kind,
        owner,
        meaning,
    }
}

/// The claim on `kind`, if an OpenAgents specification owns it.
#[must_use]
pub fn claim_of(kind: u16) -> Option<&'static Claim> {
    REGISTRY.iter().find(|claim| claim.kind == kind)
}

#[cfg(test)]
mod tests;
