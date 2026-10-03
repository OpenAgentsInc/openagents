//! Projects the real payment ledger without exposing its private identifiers.
use super::*;
use rusqlite::OptionalExtension;

impl Store {
    /// Admit a signed, public NIP-EXT publication before disclosing its author.
    /// A payment's party must also match this signer.
    pub fn register_publication(&mut self, event: &nostr::domain::Event) -> Result<(), Error> {
        let body = nostr::ext::parse_record(event)?;
        if event.kind != nostr::ext::RELEASE_KIND
            && !(event.kind == nostr::ext::LISTING_KIND && body["state"] == "published")
        {
            return Err("Expected a public plugin listing or release".into());
        }
        let package = body["package"]
            .as_str()
            .ok_or("Missing publication package")?;
        let key: [u8; 32] = hex::decode(&event.pubkey)?
            .try_into()
            .map_err(|_| "Invalid publication key")?;
        let npub = nostr::nip19::encode_npub(&key);
        self.ingest_tables()?;
        for plugin in [package, package.rsplit(':').next().unwrap_or(package)] {
            self.db.execute(
                "INSERT OR IGNORE INTO flow_publication VALUES(?,?,?)",
                params![plugin, event.pubkey, npub],
            )?;
        }
        Ok(())
    }

    /// Whether the package `id` (`<publisher>:<slug>`) has a registered
    /// publication signed by its publisher.
    pub(crate) fn published(&self, id: &str) -> bool {
        let publisher = id.split_once(':').map_or("", |(publisher, _)| publisher);
        self.db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='flow_publication')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
            && self
                .db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM flow_publication WHERE plugin=? AND party=?)",
                    params![id, publisher],
                    |r| r.get::<_, bool>(0),
                )
                .unwrap_or(false)
    }

    fn ingest_tables(&self) -> Result<(), Error> {
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS flow_publication(plugin TEXT NOT NULL,party TEXT NOT NULL,npub TEXT NOT NULL,PRIMARY KEY(plugin,party));
            CREATE TABLE IF NOT EXISTS flow_cursor(source TEXT PRIMARY KEY,seq INTEGER NOT NULL);")?;
        Ok(())
    }

    fn public_author(&self, plugin: Option<&str>, party: &str) -> Result<Option<String>, Error> {
        let Some(plugin) = plugin else {
            return Ok(None);
        };
        let party = nostr::nip19::decode_npub(party)
            .map(hex::encode)
            .unwrap_or_else(|_| party.to_owned());
        Ok(self
            .db
            .query_row(
                "SELECT npub FROM flow_publication WHERE plugin=? AND party=?",
                params![plugin, party],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub(crate) fn canonical_author(
        &self,
        author: &str,
        public_plugin: Option<&str>,
    ) -> Result<String, Error> {
        let Some(public_plugin) = public_plugin else {
            return Ok(author.to_owned());
        };
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='flow_publication')",
            [],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(author.to_owned());
        }
        let mut stmt = self
            .db
            .prepare("SELECT plugin,party,npub FROM flow_publication")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (plugin, party, npub) = row?;
            let projected = self.public_plugin(&plugin);
            if projected != public_plugin {
                continue;
            }
            if author == self.alias("author", &party, None)
                || author == self.alias("author", &npub, None)
            {
                return Ok(npub);
            }
        }
        Ok(author.to_owned())
    }

    fn ledger_record(
        &self,
        source: String,
        at: i64,
        kind: EventType,
        resource: Resource,
        plugin: Option<&str>,
    ) -> Result<SourceRecord, Error> {
        let at = at.checked_mul(1000).ok_or("Ledger time overflow")?;
        let plugin = plugin.map(|id| self.public_plugin(id));
        let node = plugin
            .as_ref()
            .map(|id| format!("plugin:{id}"))
            .unwrap_or_else(|| {
                if matches!(resource, Resource::Coder) {
                    "coder"
                } else {
                    "front"
                }
                .into()
            });
        Ok(SourceRecord {
            source,
            at,
            kind,
            resource,
            plugin,
            node,
            amount_sats: None,
            rail: None,
            split: BTreeMap::new(),
            author_identity: None,
            published_author_npub: None,
            payer_identity: None,
        })
    }

    /// Read committed `pay-ledger` rows. Both databases must be separate; the
    /// source connection needs only read permission. Replay after a crash is safe.
    pub fn sync_sources(&mut self, source: &Connection) -> Result<(), Error> {
        self.ingest_tables()?;
        let cursor = self
            .db
            .query_row(
                "SELECT seq FROM flow_cursor WHERE source='ledger'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let call_cursor = self
            .db
            .query_row(
                "SELECT seq FROM flow_cursor WHERE source='ledger-call'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let read = source.unchecked_transaction()?;
        let mut last = cursor;
        let mut last_call = call_cursor;
        let mut records = Vec::new();
        // The front records every request that reaches a route, including
        // challenges and free calls. Prices and outcomes are not payment facts.
        let mut calls =
            read.prepare("SELECT seq,at,plugin_id FROM call WHERE seq>? ORDER BY seq")?;
        for row in calls.query_map([call_cursor], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (seq, at, plugin) = row?;
            records.push(self.ledger_record(
                format!("call:{seq}"),
                at,
                EventType::Call,
                if plugin.is_some() {
                    Resource::Plugin
                } else {
                    Resource::Route
                },
                plugin.as_deref(),
            )?);
            last_call = seq;
        }
        let mut stmt = read.prepare("SELECT seq,payment_hash,settled_at,resource,plugin_id,received_msat,rail,payer_alias FROM settlement WHERE seq>? ORDER BY seq")?;
        for row in stmt.query_map([cursor], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, u64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })? {
            let (seq, key, at, path, plugin, received, rail, payer) = row?;
            let shares = read.prepare("SELECT party,role,amount_msat FROM share WHERE settlement=? ORDER BY party,role")?
                .query_map([&key], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u64>(2)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let resource = if shares.iter().any(|(_, role, _)| role == "resource") {
                Resource::HostedResource
            } else if plugin.is_some() {
                Resource::Plugin
            } else if path.starts_with("/v1/coder") {
                Resource::Coder
            } else {
                Resource::Route
            };
            let mut payment = self.ledger_record(
                format!("settlement:{seq}"),
                at,
                EventType::Payment,
                resource,
                plugin.as_deref(),
            )?;
            payment.amount_sats = Some(Sats::from_msat(received));
            payment.rail = Some(serde_json::from_value(serde_json::json!(rail))?);
            payment.payer_identity = payer;
            for (_, role, amount) in &shares {
                let role: Role = serde_json::from_value(serde_json::json!(role))?;
                let value = payment.split.entry(role).or_default();
                *value = Sats::from_msat(
                    value
                        .msat()
                        .checked_add(*amount)
                        .ok_or("Split amount overflow")?,
                );
            }
            records.push(payment);
            for (party, role, amount) in shares {
                if amount == 0 {
                    continue;
                }
                let role: Role = serde_json::from_value(serde_json::json!(role))?;
                let mut event = self.ledger_record(
                    format!("share:{seq}:{party}:{role:?}"),
                    at,
                    if role == Role::Bonus {
                        EventType::Bonus
                    } else {
                        EventType::Share
                    },
                    resource,
                    plugin.as_deref(),
                )?;
                event.amount_sats = Some(Sats::from_msat(amount));
                event.split.insert(role, Sats::from_msat(amount));
                if matches!(role, Role::Author | Role::Resource | Role::Bonus) {
                    event.published_author_npub = self.public_author(plugin.as_deref(), &party)?;
                    event.author_identity = Some(party);
                }
                records.push(event);
            }
            let mut bonuses=read.prepare("SELECT party,amount_msat FROM bonus WHERE settlement=? AND kind='first_paid_call' AND amount_msat>0 ORDER BY party")?;
            for row in bonuses.query_map([&key], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
            })? {
                let (party, amount) = row?;
                let mut event = self.ledger_record(
                    format!("first-bonus:{seq}:{party}"),
                    at,
                    EventType::Bonus,
                    resource,
                    plugin.as_deref(),
                )?;
                event.amount_sats = Some(Sats::from_msat(amount));
                event.split.insert(Role::Bonus, Sats::from_msat(amount));
                event.published_author_npub = self.public_author(plugin.as_deref(), &party)?;
                event.author_identity = Some(party);
                records.push(event);
            }
            last = seq;
        }
        // A successful payout may drain several plugins. Project each item once
        // so author and plugin totals receive exactly their own paid amount.
        let mut payouts=read.prepare("SELECT p.id,p.party,p.rail,p.updated_at,i.settlement,i.role,s.amount_msat,t.plugin_id FROM payout p JOIN (SELECT * FROM payout_item UNION ALL SELECT * FROM bonus_payout_item) i ON i.payout=p.id JOIN payable_share s ON s.settlement=i.settlement AND s.party=i.party AND s.role=i.role JOIN settlement t ON t.payment_hash=i.settlement WHERE p.state IN ('sent','succeeded') ORDER BY p.updated_at,p.id,i.settlement,i.role")?;
        for row in payouts.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, u64>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })? {
            let (id, party, rail, at, settlement, role, amount, plugin) = row?;
            let source_key = format!("payout:{id}:{settlement}:{role}");
            let role: Role = if role == "first_paid_call" {
                Role::Bonus
            } else {
                serde_json::from_value(serde_json::json!(role))?
            };
            let mut event = self.ledger_record(
                source_key,
                at,
                EventType::Payout,
                if plugin.is_some() {
                    Resource::Plugin
                } else {
                    Resource::Route
                },
                plugin.as_deref(),
            )?;
            event.amount_sats = Some(Sats::from_msat(amount));
            event.rail = Some(serde_json::from_value(serde_json::json!(rail))?);
            event.split.insert(role, Sats::from_msat(amount));
            if matches!(role, Role::Author | Role::Resource | Role::Bonus) {
                event.published_author_npub = self.public_author(plugin.as_deref(), &party)?;
                event.author_identity = Some(party);
            }
            records.push(event);
        }
        for record in records {
            self.record(record)?;
        }
        self.db.execute("INSERT INTO flow_cursor VALUES('ledger',?) ON CONFLICT(source) DO UPDATE SET seq=excluded.seq",[last])?;
        self.db.execute("INSERT INTO flow_cursor VALUES('ledger-call',?) ON CONFLICT(source) DO UPDATE SET seq=excluded.seq",[last_call])?;
        Ok(())
    }
}
