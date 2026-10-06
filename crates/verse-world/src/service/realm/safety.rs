//! Private, restart-durable account safety records selected by the realm head.
use super::super::safety as api;
use super::*;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Preferences {
    account: u64,
    blocked: Vec<u64>,
    day: u64,
    reports: u8,
    #[serde(default)]
    changes: u16,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "safety", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Record {
    Preferences(Preferences),
    Queue { ids: Vec<api::Id> },
    Report(api::Report),
    Receipt(api::Receipt),
}
fn key(domain: &[u8], value: &[u8]) -> api::Id {
    let mut bytes = b"verse.realm.safety.v1\0".to_vec();
    bytes.extend_from_slice(domain);
    bytes.push(0);
    bytes.extend_from_slice(value);
    disk::digest(&bytes)
}
fn receipt_key(account: u64, operation: [u8; 16]) -> api::Id {
    key(
        b"receipt",
        &[account.to_be_bytes().as_slice(), &operation].concat(),
    )
}
impl Record {
    pub(super) fn key(&self) -> api::Id {
        match self {
            Self::Preferences(p) => key(b"preferences", &p.account.to_be_bytes()),
            Self::Queue { .. } => key(b"queue", &[]),
            Self::Report(r) => key(b"report", &r.id),
            Self::Receipt(r) => receipt_key(r.account, r.operation),
        }
    }
}
impl Realm {
    fn safety_record(&self, id: api::Id) -> Result<Option<Record>, String> {
        match registry::get(self, self.manifest.registry_root, id)? {
            Some(registry::Record::Safety(r)) if r.key() == id => Ok(Some(r)),
            None => Ok(None),
            _ => Err("Account safety record identity differs".into()),
        }
    }
    fn preferences(&self, account: u64) -> Result<Preferences, String> {
        self.account(account)?;
        let p = match self.safety_record(key(b"preferences", &account.to_be_bytes()))? {
            Some(Record::Preferences(p))
                if p.account == account && p.reports <= 16 && p.changes <= 128 =>
            {
                p
            }
            None => Preferences {
                account,
                blocked: vec![],
                day: 0,
                reports: 0,
                changes: 0,
            },
            _ => return Err("Invalid account safety preferences".into()),
        };
        api::View {
            realm: self.manifest.id,
            account,
            blocked: p.blocked.clone(),
        }
        .validate()?;
        Ok(p)
    }
    fn safety_account(&self, lease: &Lease, connection: ConnectionId) -> Result<u64, String> {
        let public = self.games[&lease.instance].principal(connection)?.0;
        Ok(self
            .account_for_key(public)?
            .ok_or("Safety requires a current realm account")?
            .id)
    }
    fn queue(&self) -> Result<Vec<api::Id>, String> {
        match self.safety_record(key(b"queue", &[]))? {
            Some(Record::Queue { ids: q })
                if q.len() <= 256
                    && q.windows(2).all(|a| a[0] < a[1])
                    && q.iter().all(|id| *id != [0; 32]) =>
            {
                Ok(q)
            }
            None => Ok(vec![]),
            _ => Err("Invalid safety moderation queue".into()),
        }
    }
    fn safety_commit(&mut self, records: Vec<Record>) -> Result<(), String> {
        self.manifest.registry_root = registry::put(
            self,
            self.manifest.registry_root,
            records.into_iter().map(registry::Record::Safety).collect(),
        )?;
        self.safety_commit = true;
        let result = self.publish(&[]);
        self.safety_commit = false;
        result
    }
    pub fn safety_view(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        now: u64,
    ) -> Result<api::View, String> {
        self.check(lease, now)?;
        let account = self.safety_account(lease, connection)?;
        let view = api::View {
            realm: self.manifest.id,
            account,
            blocked: self.preferences(account)?.blocked,
        };
        view.validate()?;
        Ok(view)
    }
    pub fn safety_action(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        realm: api::Id,
        operation: [u8; 16],
        action: api::Action,
        now: u64,
    ) -> Result<api::Receipt, String> {
        self.check(lease, now)?;
        let account = self.safety_account(lease, connection)?;
        if realm != self.manifest.id || operation == [0; 16] {
            return Err("Safety realm or operation identity differs".into());
        }
        let digest = action.digest()?;
        if let Some(Record::Receipt(receipt)) =
            self.safety_record(receipt_key(account, operation))?
        {
            receipt.validate(realm, account, operation, &action)?;
            return Ok(receipt);
        }
        let target = action.target();
        self.account(target)?;
        if target == account {
            return Err("Safety targets must name another account".into());
        }
        let mut prefs = self.preferences(account)?;
        let day = now / 86_400_000;
        if prefs.day != day {
            prefs.day = day;
            prefs.reports = 0;
            prefs.changes = 0;
        }
        let mut records = vec![];
        let outcome = match action {
            api::Action::Block { blocked, .. } => {
                if prefs.changes >= 128 {
                    return Err("Account block change budget is 128 per UTC day".into());
                }
                prefs.changes += 1;
                match (prefs.blocked.binary_search(&target), blocked) {
                    (Err(index), true) => {
                        if prefs.blocked.len() >= 64 {
                            return Err("Account block budget exceeded".into());
                        }
                        prefs.blocked.insert(index, target);
                    }
                    (Ok(index), false) => {
                        prefs.blocked.remove(index);
                    }
                    _ => (),
                }
                api::Outcome::Block {
                    account: target,
                    blocked,
                }
            }
            api::Action::Report {
                reason, evidence, ..
            } => {
                if prefs.reports >= 16 {
                    return Err("Account report budget is 16 per UTC day".into());
                }
                let mut queue = self.queue()?;
                if queue.len() >= 256 {
                    return Err("Moderation queue is full; no report was admitted".into());
                }
                let id = api::report_id(realm, account, operation);
                let index = queue.binary_search(&id).unwrap_err();
                queue.insert(index, id);
                records.push(Record::Queue { ids: queue });
                records.push(Record::Report(api::Report {
                    id,
                    realm,
                    reporter: account,
                    target,
                    reason,
                    evidence,
                    created_ms: now,
                    status: api::Status::Open,
                }));
                prefs.reports += 1;
                api::Outcome::Report { id }
            }
        };
        let receipt = api::Receipt {
            realm,
            account,
            operation,
            digest,
            outcome,
        };
        records.push(Record::Preferences(prefs));
        records.push(Record::Receipt(receipt.clone()));
        self.safety_commit(records)?;
        Ok(receipt)
    }
    /// Contact admission for every character of either account; never disclose whose block matched.
    pub(super) fn contact(&self, from: u64, to: u64) -> Result<(), String> {
        let a = self.character(from)?.account;
        let b = self.character(to)?.account;
        if self.preferences(a)?.blocked.binary_search(&b).is_ok()
            || self.preferences(b)?.blocked.binary_search(&a).is_ok()
        {
            Err("Contact is unavailable under account safety policy".into())
        } else {
            Ok(())
        }
    }
    /// Local operator projection, never a public request body.
    pub fn pending_reports(&self) -> Result<Vec<api::Report>, String> {
        if self.poisoned {
            return Err("Realm requires recovery after an uncertain commit".into());
        }
        self.queue()?
            .into_iter()
            .map(|id| match self.safety_record(key(b"report", &id))? {
                Some(Record::Report(r))
                    if r.id == id
                        && r.realm == self.manifest.id
                        && r.status == api::Status::Open =>
                {
                    Ok(r)
                }
                _ => Err("Moderation queue report differs".into()),
            })
            .collect()
    }
    /// A completed report retains its receipt and releases queue capacity.
    pub fn resolve_report(
        &mut self,
        id: api::Id,
        status: api::Status,
        now: u64,
    ) -> Result<api::Report, String> {
        self.clock(now)?;
        if status == api::Status::Open {
            return Err("Supply a completed moderation verdict".into());
        }
        let mut report = match self.safety_record(key(b"report", &id))? {
            Some(Record::Report(r)) if r.id == id && r.realm == self.manifest.id => r,
            _ => return Err("Moderation report is missing".into()),
        };
        if report.status != api::Status::Open {
            return if report.status == status {
                Ok(report)
            } else {
                Err("Report already binds another moderation verdict".into())
            };
        }
        let mut queue = self.queue()?;
        let index = queue
            .binary_search(&id)
            .map_err(|_| "Report is absent from moderation queue")?;
        queue.remove(index);
        report.status = status;
        self.safety_commit(vec![
            Record::Queue { ids: queue },
            Record::Report(report.clone()),
        ])?;
        Ok(report)
    }
    pub(super) fn safety_request(
        &mut self,
        lease: &Lease,
        connection: ConnectionId,
        now: u64,
        body: &Body,
    ) -> Option<Result<super::super::wire::Reply, String>> {
        use super::super::wire::Reply;
        match body {
            Body::Safety {} => Some(
                self.safety_view(lease, connection, now)
                    .map(|view| Reply::Safety { view }),
            ),
            Body::SafetyAction {
                realm,
                operation,
                action,
            } => Some(
                self.safety_action(lease, connection, *realm, *operation, action.clone(), now)
                    .map(|receipt| Reply::SafetyApplied { receipt }),
            ),
            _ => None,
        }
    }
}
