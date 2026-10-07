//! Narrow commission holds and explicit native-owner review. Evidence labels
//! describe the reviewer's claim; they are not remote attestation or payment.
use crate::{Error, Ledger, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RULE: &str = "original-commission-abuse-review-v1";
pub(crate) const TABLES: &str = "
CREATE TABLE IF NOT EXISTS commission_abuse(admission TEXT PRIMARY KEY REFERENCES commission_admission(id), version INTEGER NOT NULL, state TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commission_abuse_event(request TEXT PRIMARY KEY, admission TEXT NOT NULL REFERENCES commission_admission(id), digest TEXT NOT NULL, json TEXT NOT NULL);
";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub schema: String,
    pub rule: String,
    pub reviewer: String,
    pub review_secs: u64,
    pub valid_until: u64,
    pub digest: String,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(v).expect("review serializes"))
    )
}
fn bounded(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
}
impl Rules {
    pub fn seal(mut self) -> Result<Self> {
        if self.schema != "openagents.commission-abuse-rules.v1"
            || self.rule != RULE
            || !bounded(&self.reviewer)
            || !(1..=604800).contains(&self.review_secs)
            || self.valid_until > i64::MAX as u64
        {
            return Err(Error::Invalid("commission abuse rules"));
        }
        self.digest.clear();
        self.digest = digest(&self);
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    SelfReferral,
    IdentityOverlap,
    RecycledFunding,
    DuplicateObligation,
    DestinationRebinding,
    UnknownFunding,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Finding {
    Present,
    Absent,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    NativeAccount,
    NativePayment,
    NativeFunding,
    OperatorReview,
    AdvisoryModel,
    Reputation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub reason: Reason,
    pub finding: Finding,
    pub provenance: Provenance,
    /// A bounded digest or receipt reference; never identity or credential text.
    pub reference: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Hold,
    Release,
    Reject,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub request: String,
    pub expected_version: u64,
    pub action: Action,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub admission: String,
    pub version: u64,
    pub rules: String,
    pub reviewer: String,
    pub review: Review,
    pub at: u64,
    /// Expiry means review is due. It never releases a liability.
    pub review_due: u64,
    pub digest: String,
}
fn read(c: &Connection, id: &str) -> Result<Option<Record>> {
    let value: Option<String> = c
        .query_row(
            "SELECT json FROM commission_abuse WHERE admission=?",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| serde_json::from_str(&v).map_err(|_| Error::Invalid("retained abuse review")))
        .transpose()
}
pub(crate) fn held(c: &Connection, id: &str) -> Result<bool> {
    Ok(read(c, id)?.is_some_and(|v| v.review.action != Action::Release))
}
impl Ledger {
    pub fn commission_abuse(&self, id: &str) -> Result<Option<Record>> {
        read(&self.connection, id)
    }
    /// The native adapter authenticates the original merchant owner and private
    /// reviewed rules before calling this method. Advisory evidence cannot release.
    pub fn review_commission_abuse(
        &mut self,
        id: &str,
        rules: &Rules,
        approved: &str,
        reviewer: &str,
        review: &Review,
        now: u64,
    ) -> Result<Record> {
        let admission = self
            .commission_admission(id)?
            .ok_or(Error::Invalid("original abuse admission"))?;
        if rules.clone().seal()? != *rules
            || approved != rules.digest
            || rules.reviewer != reviewer
            || admission.operator_account != reviewer
            || now > rules.valid_until
            || now > i64::MAX as u64
            || review.expected_version >= i64::MAX as u64
            || !bounded(&review.request)
            || review.evidence.is_empty()
            || review.evidence.len() > 16
            || review.evidence.iter().any(|e| {
                !e.reference.starts_with("sha256:")
                    || e.reference.len() != 71
                    || !e.reference[7..].bytes().all(|v| v.is_ascii_hexdigit())
            })
        {
            return Err(Error::Invalid(
                "original reviewed abuse authority and evidence",
            ));
        }
        let decisive = |e: &Evidence| {
            !matches!(
                e.provenance,
                Provenance::AdvisoryModel | Provenance::Reputation
            )
        };
        if review.action == Action::Release
            && review
                .evidence
                .iter()
                .any(|e| e.finding != Finding::Absent || !decisive(e))
            || review.action == Action::Reject
                && !review
                    .evidence
                    .iter()
                    .any(|e| e.finding == Finding::Present && decisive(e))
        {
            return Err(Error::Denied(
                "unknown or advisory evidence cannot release or reject a commission",
            ));
        }
        self.append_abuse(id, &rules.digest, reviewer, review, now, rules.review_secs)
    }
    fn append_abuse(
        &mut self,
        id: &str,
        rules: &str,
        reviewer: &str,
        review: &Review,
        now: u64,
        secs: u64,
    ) -> Result<Record> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = append(&tx, id, rules, reviewer, review, now, secs)?;
        tx.commit()?;
        Ok(record)
    }
    /// A native signal may only add a hold. It grants no review or release right.
    pub fn hold_commission_signal(
        &mut self,
        id: &str,
        request: &str,
        reason: Reason,
        reference: &str,
        now: u64,
    ) -> Result<Record> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = signal(&tx, id, request, reason, reference, now)?;
        tx.commit()?;
        Ok(record)
    }
}
fn signal(
    c: &Connection,
    id: &str,
    request: &str,
    reason: Reason,
    reference: &str,
    now: u64,
) -> Result<Record> {
    if !bounded(request)
        || reference.len() != 71
        || !reference.starts_with("sha256:")
        || !reference[7..].bytes().all(|v| v.is_ascii_hexdigit())
        || now > i64::MAX as u64
    {
        return Err(Error::Invalid("native abuse signal"));
    }
    if !c.query_row(
        "SELECT EXISTS(SELECT 1 FROM commission_admission WHERE id=?)",
        [id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(Error::Invalid("original signal admission"));
    }
    let key = digest(&(id, request));
    let prior: Option<String> = c
        .query_row(
            "SELECT json FROM commission_abuse_event WHERE request=?",
            [key],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(json) = prior {
        let record: Record =
            serde_json::from_str(&json).map_err(|_| Error::Invalid("native abuse event"))?;
        if record.reviewer != "native-account-signal"
            || record.review.evidence.len() != 1
            || record.review.evidence[0].reason != reason
            || record.review.evidence[0].reference != reference
        {
            return Err(Error::Conflict("native abuse signal changed"));
        }
        return Ok(record);
    }
    let old = read(c, id)?;
    if old
        .as_ref()
        .is_some_and(|v| v.review.action == Action::Reject)
    {
        return Ok(old.unwrap());
    }
    let review = Review {
        request: request.into(),
        expected_version: old.map_or(0, |v| v.version),
        action: Action::Hold,
        evidence: vec![Evidence {
            reason,
            finding: Finding::Unknown,
            provenance: Provenance::NativeAccount,
            reference: reference.into(),
        }],
    };
    append(c, id, "", "native-account-signal", &review, now, 86400)
}
/// Append the destination signal in the same transaction as its version change.
pub(crate) fn hold_destination_in(
    c: &Connection,
    party: &str,
    version: u64,
    now: u64,
) -> Result<()> {
    let mut q=c.prepare("SELECT id FROM commission_admission WHERE json_extract(json,'$.party')=? ORDER BY id LIMIT 4097")?;
    let ids = q
        .query_map([party], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(q);
    if ids.len() > 4096 {
        return Err(Error::Invalid("destination abuse scope bound"));
    }
    let proof = digest(&(party, version));
    for id in ids {
        signal(
            c,
            &id,
            &format!("destination:{version}"),
            Reason::DestinationRebinding,
            &proof,
            now,
        )?;
    }
    Ok(())
}

/// Cached native payee or relay sources use the same original-claim hold.
pub(crate) fn hold_payee_rebinding_in(
    c: &Connection,
    party: &str,
    kind: &str,
    value: &str,
    now: u64,
) -> Result<()> {
    let mut q=c.prepare("SELECT id FROM commission_admission WHERE json_extract(json,'$.party')=? ORDER BY id LIMIT 4097")?;
    let ids = q
        .query_map([party], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(q);
    if ids.len() > 4096 {
        return Err(Error::Invalid("payee abuse scope bound"));
    }
    let proof = digest(&(party, kind, value));
    for id in ids {
        let version = read(c, &id)?.map_or(0, |r| r.version);
        signal(
            c,
            &id,
            &format!("payee-rebind:{version}"),
            Reason::DestinationRebinding,
            &proof,
            now,
        )?;
    }
    Ok(())
}

fn append(
    c: &Connection,
    id: &str,
    rules: &str,
    reviewer: &str,
    review: &Review,
    now: u64,
    secs: u64,
) -> Result<Record> {
    let request = digest(&(id, &review.request));
    let input = digest(&(id, rules, reviewer, review));
    let previous: Option<(String, String)> = c
        .query_row(
            "SELECT digest,json FROM commission_abuse_event WHERE request=?",
            [&request],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((old, json)) = previous {
        if old != input {
            return Err(Error::Conflict("abuse review request changed"));
        }
        return serde_json::from_str(&json).map_err(|_| Error::Invalid("retained abuse event"));
    }
    let old = read(c, id)?;
    if old.as_ref().map_or(0, |v| v.version) != review.expected_version
        || old.as_ref().is_some_and(|v| {
            v.review.action == Action::Reject
                || !v.rules.is_empty() && v.rules != rules && !rules.is_empty()
        })
    {
        return Err(Error::Conflict("abuse review version or frozen rules"));
    }
    if review.action != Action::Hold && old.as_ref().is_none_or(|v| v.review.action != Action::Hold)
    {
        return Err(Error::Denied("original active abuse hold required"));
    }
    let events: i64 = c.query_row(
        "SELECT COUNT(*) FROM commission_abuse_event WHERE admission=?",
        [id],
        |r| r.get(0),
    )?;
    if events >= 256 {
        return Err(Error::Invalid("abuse review audit bound"));
    }
    let mut record = Record {
        admission: id.into(),
        version: review.expected_version + 1,
        rules: if rules.is_empty() {
            old.as_ref().map_or(String::new(), |v| v.rules.clone())
        } else {
            rules.into()
        },
        reviewer: reviewer.into(),
        review: review.clone(),
        at: now,
        review_due: now
            .checked_add(secs)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Invalid("abuse review clock"))?,
        digest: String::new(),
    };
    record.digest = digest(&record);
    let json =
        serde_json::to_string(&record).map_err(|_| Error::Invalid("abuse review encoding"))?;
    let state = if review.action == Action::Release {
        "released"
    } else {
        "held"
    };
    c.execute("INSERT INTO commission_abuse VALUES(?,?,?,?) ON CONFLICT(admission) DO UPDATE SET version=excluded.version,state=excluded.state,json=excluded.json",params![id,record.version as i64,state,&json])?;
    c.execute(
        "INSERT INTO commission_abuse_event VALUES(?,?,?,?)",
        params![request, id, input, json],
    )?;
    Ok(record)
}
