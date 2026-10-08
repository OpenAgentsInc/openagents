//! Bounded original-record statements. The controller supplies reviewed read scope.
use super::*;
use std::collections::BTreeSet;
pub const STATEMENT_SCHEMA: &str = "openagents.joined-statement.v1";
pub const STATEMENT_PAGE_MAX: usize = 100;
const SCAN_MAX: usize = 10_000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementScope {
    pub bindings: Vec<String>,
    pub authority: String,
    /// Full customer scope is separately authorized. Member views omit pool totals.
    pub full_customer: bool,
    pub native_account: String,
    pub native_source: receipts::purchase::CommercialSource,
    pub native_origin: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    pub after_earning: Option<i64>,
    pub after_payout: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementRow {
    pub key: String,
    pub kind: String,
    pub state: String,
    pub source: receipts::purchase::CommercialSource,
    pub commercial: CommercialRef,
    pub binding: String,
    pub unit: Unit,
    /// Integer source units per whole currency unit.
    pub unit_scale: u64,
    pub conversion: Option<Conversion>,
    pub native_attempt: Option<String>,
    pub intent_digest: Option<String>,
    pub quote: Option<String>,
    pub execution: Option<String>,
    pub terms: Option<String>,
    pub payment_reference: Option<String>,
    pub units: Option<u64>,
    pub reserved_msat: i64,
    pub charged_msat: Option<i64>,
    pub credited_msat: Option<i64>,
    pub released_msat: i64,
    pub returned_msat: i64,
    pub loss_msat: i64,
    pub recovered_msat: i64,
    pub reduced_claim_msat: i64,
    pub fee_msat: Option<u64>,
    pub remainder: Option<u64>,
    pub denominator: Option<u64>,
    pub source_evidence: Option<String>,
    pub settlement_reference: Option<String>,
    pub allocation_rule_version: Option<i64>,
    /// Customer allocation references are separate from private payee statements.
    pub allocations: Vec<Value>,
    pub disclosure: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinedStatement {
    pub schema: String,
    pub origin: String,
    pub customer: String,
    pub workspace: String,
    pub unit: Unit,
    pub unit_scale: u64,
    pub balance: Option<ComputeBalance>,
    pub snapshot: String,
    pub rows: Vec<StatementRow>,
    pub next: Option<String>,
    pub scanned: usize,
    pub disclosure: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    scope: String,
    snapshot: String,
    after: String,
}
fn row(binding: &Binding, key: String, kind: &str, state: &str) -> StatementRow {
    StatementRow {
        key,
        kind: kind.into(),
        state: state.into(),
        source: binding.source.clone(),
        commercial: binding.commercial.clone(),
        binding: binding.id.clone(),
        unit: binding.conversion.source.clone(),
        unit_scale: binding.conversion.source.scale(),
        conversion: Some(binding.conversion.clone()),
        native_attempt: None,
        intent_digest: None,
        quote: None,
        execution: None,
        terms: None,
        payment_reference: None,
        units: None,
        reserved_msat: 0,
        charged_msat: None,
        credited_msat: None,
        released_msat: 0,
        returned_msat: 0,
        loss_msat: 0,
        recovered_msat: 0,
        reduced_claim_msat: 0,
        fee_msat: None,
        remainder: None,
        denominator: None,
        source_evidence: None,
        settlement_reference: None,
        allocation_rule_version: None,
        allocations: vec![],
        disclosure: vec![],
    }
}
impl Ledger {
    /// Read one SQLite snapshot. Pagination refuses changed financial facts;
    /// a caller restarts the read instead of combining incompatible totals.
    pub fn joined_statement(
        &mut self,
        scope: &StatementScope,
        query: &StatementQuery,
    ) -> Result<JoinedStatement> {
        let limit = query.limit.unwrap_or(50);
        if limit == 0
            || limit > STATEMENT_PAGE_MAX
            || scope.bindings.is_empty()
            || scope.bindings.len() > 128
            || scope.authority.len() != 64
            || scope.native_account.is_empty()
        {
            return Err(Error::Invalid("joined statement scope or bounds"));
        }
        let cursor: Option<Cursor> = query
            .cursor
            .as_ref()
            .map(|c| {
                if c.len() > 4096 {
                    return Err(invalid("cursor"));
                }
                let bytes = hex::decode(c).map_err(invalid)?;
                serde_json::from_slice(&bytes).map_err(invalid)
            })
            .transpose()?;
        let scope_digest = digest(scope);
        if cursor.as_ref().is_some_and(|c| c.scope != scope_digest) {
            return Err(Error::Denied(
                "statement cursor belongs to another current scope",
            ));
        }
        let tx = self.connection.transaction()?;
        let mut bindings = Vec::new();
        let mut identities = BTreeSet::new();
        for id in &scope.bindings {
            let b = binding_in(&tx, id)?.ok_or(Error::Denied("statement source is absent"))?;
            if !identities.insert(b.native_identity()) {
                return Err(Error::Invalid("duplicate statement source"));
            }
            bindings.push(b);
        }
        let first = &bindings[0];
        if bindings.iter().any(|b| {
            b.pool != first.pool
                || b.commercial.customer != first.commercial.customer
                || b.commercial.workspace != first.commercial.workspace
        }) {
            return Err(Error::Denied("statement cannot merge distinct customers"));
        }
        let pool = first.pool.clone();
        let customer = first.commercial.customer.clone();
        let workspace = first.commercial.workspace.clone();
        let origin = first.ledger_origin.clone();
        if scope.full_customer {
            let mut q = tx.prepare("SELECT bytes FROM shared_binding b JOIN shared_binding_head h ON h.head=b.id UNION ALL SELECT v.bytes FROM shared_binding_version v JOIN shared_binding_head h ON h.head=v.id")?;
            let active = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for bytes in active {
                let b: Binding = parse(&bytes)?;
                if b.pool == pool && !identities.contains(&b.native_identity()) {
                    return Err(Error::Denied(
                        "full statement requires every original customer source",
                    ));
                }
            }
        }
        let mut rows = Vec::new();
        let mut scanned = 0;
        let mut omitted = 0;
        for binding in &bindings {
            let mut q = tx.prepare("SELECT i.id,i.bytes FROM shared_intent i JOIN (SELECT id,native_identity FROM shared_binding UNION ALL SELECT id,native_identity FROM shared_binding_version) b ON b.id=i.binding WHERE b.native_identity=? ORDER BY i.rowid LIMIT ?")?;
            let intents = q
                .query_map(params![binding.native_identity(), SCAN_MAX + 1], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            drop(q);
            for (id, bytes) in intents {
                scanned += 1;
                if scanned > SCAN_MAX {
                    return Err(Error::Invalid(
                        "joined statement exceeds disclosed scan bound",
                    ));
                }
                let intent: Intent = parse(&bytes)?;
                if intent.binding.pool != pool
                    || intent.binding.commercial.customer != customer
                    || intent.binding.commercial.workspace != workspace
                    || intent.binding.ledger_origin != origin
                {
                    // A native source can move between customers. Its shared
                    // source identity never grants the new customer's history.
                    continue;
                }
                if !scope.full_customer {
                    let actor = tx
                        .query_row(
                            "SELECT bytes FROM shared_native_actor WHERE intent=?",
                            [&id],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()?
                        .map(|s| parse::<Value>(&s))
                        .transpose()?;
                    if intent.binding.source != scope.native_source
                        || intent.binding.native_origin != scope.native_origin
                        || actor.as_ref().and_then(|a| a["account"].as_str())
                            != Some(scope.native_account.as_str())
                    {
                        omitted += 1;
                        continue;
                    }
                }
                let hold = crate::compute::hold::read_hold(&tx, "id", &id)?
                    .ok_or(Error::Invalid("original statement hold missing"))?;
                let state: String =
                    tx.query_row("SELECT state FROM shared_outcome WHERE id=?", [&id], |r| {
                        r.get(0)
                    })?;
                let out = Outcome {
                    intent: intent.clone(),
                    hold,
                    state,
                    expense: None,
                    evidence: None,
                };
                let sealed = accounting::source_settlement_in(&tx, &id)?;
                let mut entry = row(
                    &intent.binding,
                    format!("charge:{id}"),
                    "liability",
                    &out.state,
                );
                entry.native_attempt = Some(intent.native_attempt.clone());
                entry.intent_digest = Some(intent.digest());
                entry.quote = Some(intent.quote.clone());
                entry.execution = Some(intent.execution.clone());
                entry.terms = Some(intent.terms.clone());
                entry.reserved_msat = out.hold.request.amount_msat;
                entry.charged_msat = out.hold.charge_msat;
                entry.released_msat = out
                    .hold
                    .charge_msat
                    .map_or(0, |charge| out.hold.request.amount_msat - charge);
                entry.payment_reference = intent.invoice.as_ref().map(|i| i.payment_hash.clone());
                if let Some(s) = sealed {
                    if s.intent_digest != intent.digest() {
                        return Err(Error::Invalid("statement source settlement changed"));
                    }
                    entry.units = Some(s.units);
                    entry.fee_msat = Some(s.fee_msat);
                    entry.remainder = Some(s.remainder);
                    entry.denominator = Some(s.denominator);
                    entry.source_evidence = Some(crate::digest(&s.evidence));
                } else if out.state == "settled" {
                    entry
                        .disclosure
                        .push("original_source_settlement_missing".into());
                }
                let settlement = format!("debit:{id}");
                let version = tx
                    .query_row(
                        "SELECT rule_version FROM settlement WHERE payment_hash=?",
                        [&settlement],
                        |r| r.get::<_, i64>(0),
                    )
                    .optional()?;
                if let Some(v) = version {
                    entry.settlement_reference = Some(settlement.clone());
                    entry.allocation_rule_version = Some(v);
                    let mut q = tx.prepare("SELECT party,role,amount_msat FROM payable_share WHERE settlement=? ORDER BY party,role LIMIT 129")?;
                    let shares = q
                        .query_map([&settlement], |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, i64>(2)?,
                            ))
                        })?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    if shares.len() > 128 {
                        return Err(Error::Invalid("statement allocation bound"));
                    }
                    for (party, role, amount) in shares {
                        let mut q = tx.prepare("SELECT DISTINCT p.id,p.state,p.wallet_reference FROM (SELECT payout,settlement,party,role FROM payout_item UNION ALL SELECT payout,settlement,party,role FROM commission_payout_item UNION ALL SELECT payout,settlement,party,role FROM bonus_payout_item) i JOIN payout p ON p.id=i.payout WHERE i.settlement=? AND i.party=? AND i.role=? ORDER BY p.id LIMIT 129")?;
                        let payouts = q.query_map(params![settlement,party,role], |r|Ok(serde_json::json!({"reference":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"rail_reference":r.get::<_,Option<String>>(2)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
                        if payouts.len() > 128 {
                            return Err(Error::Invalid("statement payout reference bound"));
                        }
                        entry.allocations.push(serde_json::json!({"party_reference":crate::digest(&party),"role":role,"amount_msat":amount,"payouts":payouts}));
                    }
                } else if let Liability::ExternalInvoice {
                    merchant,
                    plugin,
                    release,
                    author,
                    author_fee_msat,
                } = &intent.liability
                {
                    entry.allocations.push(serde_json::json!({"role":"author","party_reference":crate::digest(author),"amount_msat":author_fee_msat,"state":"original_declared_external_author_fee","plugin":plugin,"release":release,"merchant_reference":crate::digest(merchant),"payouts":[]}));
                    entry.disclosure.push("external_receiver_accrual_and_author_payout_require_the_original_receiver_statement".into());
                }
                rows.push(entry);
                let mut q = tx.prepare(
                    "SELECT bytes FROM shared_refund_plan WHERE intent=? ORDER BY id LIMIT ?",
                )?;
                let plans = q
                    .query_map(params![id, SCAN_MAX + 1], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                drop(q);
                for bytes in plans {
                    scanned += 1;
                    if scanned > SCAN_MAX {
                        return Err(Error::Invalid(
                            "joined statement exceeds disclosed scan bound",
                        ));
                    }
                    let plan: RefundPlan = parse(&bytes)?;
                    let refund = accounting::refund_in(&tx, &plan.review.id)?;
                    let mut entry = row(
                        &plan.binding,
                        format!("refund:{}", plan.review.id),
                        "refund",
                        if refund.is_some() {
                            "returned"
                        } else {
                            "unknown"
                        },
                    );
                    entry.native_attempt = Some(intent.native_attempt.clone());
                    entry.units = Some(plan.review.units);
                    entry.returned_msat = refund.as_ref().map_or(0, |r| r.returned_msat);
                    entry.loss_msat = refund.as_ref().map_or(0, |r| r.loss_msat);
                    entry.reduced_claim_msat = refund.as_ref().map_or(0, |r| r.reduced_msat);
                    entry.payment_reference = refund.as_ref().and_then(|r| r.incoming_hash.clone());
                    entry.source_evidence = Some(crate::digest(&plan.review.evidence));
                    if refund.is_none() {
                        entry
                            .disclosure
                            .push("prepared_refund_is_reserved_and_not_confirmed_return".into());
                    }
                    rows.push(entry);
                }
                let mut q = tx.prepare("SELECT bytes FROM shared_refund WHERE intent=? AND id NOT IN (SELECT id FROM shared_refund_plan) ORDER BY id LIMIT ?")?;
                let refunds = q
                    .query_map(params![id, SCAN_MAX + 1], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for bytes in refunds {
                    scanned += 1;
                    if scanned > SCAN_MAX {
                        return Err(Error::Invalid(
                            "joined statement exceeds disclosed scan bound",
                        ));
                    }
                    let refund: Refund = parse(&bytes)?;
                    let mut entry = row(
                        &intent.binding,
                        format!("refund:{}", refund.review.id),
                        "refund",
                        "returned",
                    );
                    entry.native_attempt = Some(intent.native_attempt.clone());
                    entry.units = Some(refund.review.units);
                    entry.returned_msat = refund.returned_msat;
                    entry.loss_msat = refund.loss_msat;
                    entry.reduced_claim_msat = refund.reduced_msat;
                    entry.payment_reference = refund.incoming_hash;
                    entry.source_evidence = Some(crate::digest(&refund.review.evidence));
                    rows.push(entry);
                }
            }
        }
        if scope.full_customer {
            let mut q=tx.prepare("SELECT f.id,f.binding,f.terms,f.invoice FROM shared_funding_record f JOIN (SELECT id,pool FROM shared_binding UNION ALL SELECT id,json_extract(bytes,'$.pool') AS pool FROM shared_binding_version) b ON b.id=f.binding WHERE b.pool=? ORDER BY f.id LIMIT ?")?;
            let funds = q
                .query_map(params![pool, SCAN_MAX + 1], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            drop(q);
            for (id, binding, terms, invoice) in funds {
                let original = binding_in(&tx, &binding)?
                    .ok_or(Error::Invalid("original funding source missing"))?;
                if !identities.contains(&original.native_identity()) {
                    continue;
                }
                scanned += 1;
                if scanned > SCAN_MAX {
                    return Err(Error::Invalid(
                        "joined statement exceeds disclosed scan bound",
                    ));
                }
                let terms: Value = parse(&terms)?;
                let purchase = tx
                    .query_row(
                        "SELECT state,payment_hash,amount_msat FROM compute_purchase WHERE id=?",
                        [&id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                let mut entry = row(
                    &original,
                    format!("funding:{id}"),
                    "funding",
                    purchase.as_ref().map_or("unknown", |p| p.0.as_str()),
                );
                entry.units = terms["amount_msat"].as_u64();
                // Funding is native BTC millisatoshis, independently of spend-price conversion.
                entry.unit = Unit::Millisatoshis;
                entry.conversion = None;
                entry.payment_reference = purchase.as_ref().map(|p| p.1.clone());
                let invoice: Option<Value> = invoice.map(|s| parse(&s)).transpose()?;
                if let Some(p) = &purchase {
                    let credit = tx
                        .query_row(
                            "SELECT amount_msat FROM compute_credit WHERE account=? AND source=?",
                            params![pool, format!("topup:{}", p.1)],
                            |r| r.get::<_, i64>(0),
                        )
                        .optional()?;
                    let consistent = terms["amount_msat"].as_u64() == u64::try_from(p.2).ok()
                        && invoice.as_ref().and_then(|i| i["payment_hash"].as_str())
                            == Some(p.1.as_str())
                        && if p.0 == "paid" {
                            credit == Some(p.2)
                        } else {
                            credit.is_none()
                        };
                    if consistent && p.0 == "paid" {
                        entry.credited_msat = Some(p.2);
                    } else if !consistent {
                        entry.state = "inconsistent".into();
                        entry.disclosure.push(
                            "original_funding_terms_invoice_and_posted_credit_do_not_reconcile"
                                .into(),
                        );
                    }
                }
                rows.push(entry);
                let mut q=tx.prepare("SELECT id,amount,recovered,loss,evidence FROM shared_funding_reversal WHERE funding=? ORDER BY id LIMIT ?")?;
                let reversals = q
                    .query_map(params![id, SCAN_MAX + 1], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, String>(4)?,
                        ))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (rid, amount, recovered, loss, evidence) in reversals {
                    scanned += 1;
                    if scanned > SCAN_MAX {
                        return Err(Error::Invalid(
                            "joined statement exceeds disclosed scan bound",
                        ));
                    }
                    let mut entry = row(
                        &original,
                        format!("reversal:{rid}"),
                        "funding-reversal",
                        "reviewed",
                    );
                    entry.units = Some(amount as u64);
                    entry.unit = Unit::Millisatoshis;
                    entry.conversion = None;
                    entry.recovered_msat = recovered;
                    entry.loss_msat = loss;
                    entry.payment_reference = Some(id.clone());
                    entry.source_evidence = Some(crate::digest(&evidence));
                    rows.push(entry);
                }
            }
        }
        for row in &mut rows {
            row.unit_scale = row.unit.scale();
        }
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        let balance = scope
            .full_customer
            .then(|| crate::compute::hold::balance_in(&tx, &pool))
            .transpose()?;
        let attributed_credit =
            rows.iter()
                .filter_map(|r| r.credited_msat)
                .try_fold(0i64, |sum, n| {
                    sum.checked_add(n)
                        .ok_or(Error::Invalid("statement credit overflow"))
                })?;
        let unmatched_credit = balance
            .as_ref()
            .map_or(0, |b| b.credited_msat - attributed_credit);
        let snapshot = digest(&(&scope_digest, &rows, &balance));
        let disclosed_scan = if scope.full_customer {
            scanned
        } else {
            rows.len()
        };
        let omitted_disclosure = if scope.full_customer {
            format!("omitted_unattributed_or_other_member:{omitted}")
        } else {
            "other_member_and_unattributed_counts_are_private".into()
        };
        if cursor
            .as_ref()
            .is_some_and(|c| c.snapshot != snapshot || !rows.iter().any(|r| r.key == c.after))
        {
            return Err(Error::Conflict(
                "statement snapshot or cursor changed; restart the bounded read",
            ));
        }
        let after = cursor.as_ref().map(|c| c.after.as_str());
        let mut page = rows
            .into_iter()
            .filter(|r| after.is_none_or(|a| r.key.as_str() > a))
            .take(limit + 1)
            .collect::<Vec<_>>();
        let next = if page.len() > limit {
            Some(hex::encode(
                serde_json::to_vec(&Cursor {
                    scope: scope_digest,
                    snapshot: snapshot.clone(),
                    after: page[limit - 1].key.clone(),
                })
                .map_err(invalid)?,
            ))
        } else {
            None
        };
        page.truncate(limit);
        tx.commit()?;
        Ok(JoinedStatement {
            schema: STATEMENT_SCHEMA.into(),
            origin,
            customer,
            workspace,
            unit: Unit::Millisatoshis,
            unit_scale: Unit::Millisatoshis.scale(),
            balance,
            snapshot,
            rows: page,
            next,
            scanned: disclosed_scan,
            disclosure: vec![
                format!("scan_max:{SCAN_MAX}"),
                format!("unattributed_or_inconsistent_funding_msat:{unmatched_credit}"),
                omitted_disclosure,
                "customer_spend_and_payee_earnings_are_distinct".into(),
                "unused_release_is_not_a_refund".into(),
                "source_price_units_are_not_summed_across_conversions".into(),
                "native_price_is_retained_by_original_quote_and_terms_references".into(),
                "credited_plus_refunded_minus_recovered_equals_available_plus_restricted_plus_held_plus_settled".into(),
            ],
        })
    }
}
