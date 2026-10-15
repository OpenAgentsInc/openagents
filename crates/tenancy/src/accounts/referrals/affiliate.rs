//! The affiliate program (#11698): its published terms, its numbers, and
//! the accrual of commission on Pro sales that came through an affiliate's
//! `/r/{token}` link.
//!
//! The owner set the terms on 2026-10-15:
//!
//! - 20% of net Pro revenue from each referred account, for its first 12
//!   paid months. Net is after refunds, disputes and payment fees; tax is
//!   left out.
//! - Last click, 30-day window, from the `oa_ref` click recorded at
//!   sign-up. Only `/r/` affiliate links earn; our own `/go/` codes never do.
//! - No self-referral: the affiliate's own account, or an account that
//!   shares a workspace with it, earns nothing.
//! - Commission becomes payable 30 days after the payment it comes from. A
//!   refund or dispute inside that window reverses it; a later one is
//!   clawed back from future commission.
//! - Paid in sats over Lightning, monthly, once the payable balance is at
//!   least 10,000 sats, converted from USD at the payout date's rate.
//!
//! [`terms`] is the text affiliates accept: it is published as the
//! referral system's attribution [`Policy`] ([`policy`], through the
//! operator's `tenant-referrals publish`) and shown on the public
//! `/affiliates` page. [`PROGRAM`] holds the numbers; [`ledger`] is the
//! accrual, a pure function over the sales the gateway records
//! (`gateway::conversions`). Nothing here moves money: payouts are listed
//! for the owner, who sends them through the payout bridge.

use super::attribution::Policy;
use super::*;
use std::collections::BTreeSet;

/// The published terms' version.
pub const TERMS_VERSION: &str = "affiliate-2026-10-15";

/// The terms, one point per paragraph, in plain words. [`terms`] joins
/// them; the `/affiliates` page shows each as a paragraph.
pub const TERMS_POINTS: &[&str] = &[
    "We pay you 20% of the net Pro revenue from each account you refer, for that account's first 12 paid months. Net means after refunds, disputes and payment fees. Tax is not counted.",
    "An account is yours when its person last clicked your /r/ link within 30 days before signing up. The last link clicked wins. Our own /go/ campaign links never earn commission.",
    "Your own account, and accounts that share a workspace with yours, earn nothing.",
    "Commission becomes payable 30 days after the payment it comes from. A refund or dispute inside those 30 days cancels it. A refund or dispute after that is taken back from your future commission.",
    "We pay in bitcoin (sats) over Lightning only, once a month, when your payable balance is at least 10,000 sats. We convert from US dollars at our bitcoin price on the day we pay.",
    "You must mark paid promotion clearly, for example with #ad or \"paid partnership\", as the FTC endorsement guides require. If you use an AI persona, never present it as a real person. Do not bid on the OpenAgents name in ads.",
    "If you break these rules, you lose your unpaid commission, and we may remove your account from the program.",
    "We may change these terms with 30 days' notice. Commission you already earned stays under the terms you earned it under.",
];

/// The terms affiliates accept, as one line of text: the policy refuses
/// control characters, so the points are joined by spaces (well under its
/// 4096-byte bound).
#[must_use]
pub fn terms() -> String {
    TERMS_POINTS.join(" ")
}

/// The attribution policy that publishes [`terms`]. The operator publishes
/// it with `tenant-referrals publish` (input `{"version": TERMS_VERSION,
/// "terms": terms()}`); affiliates accept its digest before they earn.
pub fn policy() -> Result<Policy, Error> {
    Policy::new(TERMS_VERSION.into(), terms())
}

/// The program's numbers. Every number the accrual uses is here, never a
/// literal in the code that reads it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Program {
    /// The share of net revenue paid, as `rate_numerator / rate_denominator`.
    pub rate_numerator: u64,
    pub rate_denominator: u64,
    /// How many of a referred account's paid months earn commission.
    pub paid_months: u32,
    /// Seconds after a payment before its commission is payable.
    pub hold_secs: u64,
    /// The smallest balance paid out, in sats.
    pub minimum_sats: u64,
    /// The card fee taken from a payment whose actual fee was not
    /// recorded: `fee_basis_points / 10,000` of the amount, plus
    /// `fee_fixed_cents`. Stripe's US card price.
    pub fee_basis_points: u64,
    pub fee_fixed_cents: u64,
    /// Days of notice before the terms change.
    pub notice_days: u64,
}

const DAY: u64 = 86_400;

/// The program as published in [`TERMS_POINTS`].
pub const PROGRAM: Program = Program {
    rate_numerator: 20,
    rate_denominator: 100,
    paid_months: 12,
    hold_secs: 30 * DAY,
    minimum_sats: 10_000,
    fee_basis_points: 290,
    fee_fixed_cents: 30,
    notice_days: 30,
};

impl Program {
    /// The rate as a whole percent, for display.
    #[must_use]
    pub fn rate_percent(&self) -> u64 {
        self.rate_numerator * 100 / self.rate_denominator.max(1)
    }
    /// The hold in whole days, for display.
    #[must_use]
    pub fn hold_days(&self) -> u64 {
        self.hold_secs / DAY
    }
    /// The payment fee on `cents` when none was recorded.
    #[must_use]
    pub fn card_fee_cents(&self, cents: u64) -> u64 {
        (cents * self.fee_basis_points).div_ceil(10_000) + self.fee_fixed_cents
    }
    /// Commission, in millionths of a dollar, on `net_cents`, rounded down.
    #[must_use]
    pub fn commission_micros(&self, net_cents: u64) -> u64 {
        let micros = u128::from(net_cents) * 10_000;
        u64::try_from(
            micros * u128::from(self.rate_numerator) / u128::from(self.rate_denominator.max(1)),
        )
        .unwrap_or(u64::MAX)
    }
}

/// What a recorded sale was.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaleKind {
    /// A paid Pro month.
    Paid,
    /// A full refund or a lost dispute of an earlier paid month.
    Reversed,
}

/// One recorded sale or its reversal, as the gateway's conversions hold it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Sale {
    pub kind: SaleKind,
    pub link: LinkKind,
    /// The affiliate (the referrer id); `None` for a `/go/` code.
    pub referrer: Option<String>,
    /// The paying account.
    pub account: String,
    pub workspace: Option<String>,
    pub invoice: String,
    /// When it was paid (or reversed), in Unix seconds.
    pub at: u64,
    /// What the customer paid, in cents, tax included.
    pub amount_cents: u64,
    /// Upper-case currency code; only `USD` earns.
    pub currency: Option<String>,
    /// Tax inside `amount_cents`, when the invoice named it.
    pub tax_cents: Option<u64>,
    /// The payment fee, when it was recorded; otherwise the card fee.
    pub fee_cents: Option<u64>,
}

/// A payout the owner sent, recorded after the fact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Payout {
    pub referrer: String,
    /// The commission it settled, in millionths of a dollar.
    pub usd_micros: u64,
    /// The sats sent.
    pub sats: u64,
    pub at: u64,
}

/// Who an affiliate is: the account that owns the referrer, its label, and
/// the short link keys the click records use (`aff-` and 16 hex digits of
/// the link token's SHA-256).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Affiliate {
    pub owner: String,
    pub label: String,
    pub keys: Vec<String>,
}

/// The affiliates and which accounts are linked to them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Directory {
    /// Referrer id to affiliate.
    pub affiliates: BTreeMap<String, Affiliate>,
    /// Account id to every workspace it is or was a member of.
    pub workspaces: BTreeMap<String, BTreeSet<String>>,
}

impl Directory {
    /// Every affiliate and workspace membership in the account store.
    /// Revoked memberships count: leaving a shared workspace doesn't make
    /// two accounts strangers.
    #[must_use]
    pub fn of(store: &Store) -> Self {
        let mut affiliates: BTreeMap<String, Affiliate> = store
            .referrals
            .referrers
            .values()
            .map(|r| {
                (
                    r.id.clone(),
                    Affiliate {
                        owner: r.owner.clone(),
                        label: r.label.clone(),
                        keys: Vec::new(),
                    },
                )
            })
            .collect();
        for (digest, link) in &store.referrals.links {
            if let (Some(a), Some(short)) = (affiliates.get_mut(&link.referrer), digest.get(..16)) {
                a.keys.push(format!("aff-{short}"));
            }
        }
        let mut workspaces: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for ws in store.workspaces.values() {
            for account in ws.members.keys() {
                workspaces
                    .entry(account.clone())
                    .or_default()
                    .insert(ws.id.clone());
            }
        }
        Self {
            affiliates,
            workspaces,
        }
    }

    /// The referrer ids `account` owns.
    pub fn owned_by<'a>(&'a self, account: &'a str) -> impl Iterator<Item = &'a String> + 'a {
        self.affiliates
            .iter()
            .filter(move |(_, a)| a.owner == account)
            .map(|(id, _)| id)
    }

    /// Whether a sale by `account` (paying for `workspace`) is the
    /// affiliate's own: the same account, a member of the paying
    /// workspace, or an account sharing any workspace with it. An unknown
    /// affiliate counts as its own, so nothing is owed to no one.
    #[must_use]
    pub fn self_referral(&self, referrer: &str, account: &str, workspace: Option<&str>) -> bool {
        let Some(owner) = self.affiliates.get(referrer).map(|a| a.owner.as_str()) else {
            return true;
        };
        if owner == account {
            return true;
        }
        let empty = BTreeSet::new();
        let mine = self.workspaces.get(owner).unwrap_or(&empty);
        if workspace.is_some_and(|w| mine.contains(w)) {
            return true;
        }
        self.workspaces
            .get(account)
            .is_some_and(|theirs| !mine.is_disjoint(theirs))
    }
}

/// Where one commission stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Inside the hold: not payable yet.
    Held,
    /// Past the hold and not reversed.
    Payable,
    /// Refunded or disputed inside the hold: never earned.
    Reversed,
    /// Refunded or disputed after the hold: taken back from later
    /// commission.
    ClawedBack,
}

/// Why a sale earns nothing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Excluded {
    /// One of our own `/go/` campaign codes.
    Campaign,
    /// The affiliate's own account or one linked to it.
    SelfReferral,
    /// Past the referred account's earning months.
    PastTerm,
    /// Not in US dollars.
    Currency,
}

/// One paid month and its commission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Item {
    pub referrer: Option<String>,
    pub account: String,
    pub invoice: String,
    /// Which of the account's paid months this was, from 1.
    pub month: u32,
    pub paid_at: u64,
    /// When it is (or was) payable.
    pub payable_at: u64,
    pub net_cents: u64,
    pub commission_micros: u64,
    pub reversed_at: Option<u64>,
    pub state: State,
    pub excluded: Option<Excluded>,
}

/// One affiliate's totals, in millionths of a dollar.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Balance {
    /// Commission earned and not reversed inside the hold.
    pub accrued: u64,
    /// Earned, still inside the hold.
    pub held: u64,
    /// Ready to pay now: past the hold, less clawbacks and payouts.
    pub payable: u64,
    pub paid: u64,
    /// Taken back for refunds and disputes after the hold.
    pub clawed_back: u64,
    /// Clawbacks not yet covered by later commission.
    pub owed: u64,
    /// Paid months that earned commission.
    pub paid_conversions: u64,
    /// Months refunded or disputed.
    pub refunds: u64,
}

/// Every affiliate's balance and every month behind it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ledger {
    pub balances: BTreeMap<String, Balance>,
    pub items: Vec<Item>,
}

/// The commission ledger as of `now`: each paid month of a referred
/// account, in payment order, earns `program`'s rate on its net for the
/// account's first `paid_months` months, after the hold, unless reversed.
#[must_use]
pub fn ledger(
    program: &Program,
    sales: &[Sale],
    directory: &Directory,
    payouts: &[Payout],
    now: u64,
) -> Ledger {
    let reversals: BTreeMap<&str, u64> = sales
        .iter()
        .filter(|s| s.kind == SaleKind::Reversed)
        .fold(BTreeMap::new(), |mut m, s| {
            let at = m.entry(s.invoice.as_str()).or_insert(s.at);
            *at = (*at).min(s.at);
            m
        });
    let mut paid: Vec<&Sale> = sales.iter().filter(|s| s.kind == SaleKind::Paid).collect();
    paid.sort_by(|a, b| (a.at, &a.invoice).cmp(&(b.at, &b.invoice)));
    let mut seen = BTreeSet::new();
    let mut months: BTreeMap<&str, u32> = BTreeMap::new();
    let mut out = Ledger::default();
    for sale in paid {
        if !seen.insert(sale.invoice.as_str()) {
            continue;
        }
        let month = months.entry(sale.account.as_str()).or_default();
        *month += 1;
        let tax = sale.tax_cents.unwrap_or(0);
        let fee = sale
            .fee_cents
            .unwrap_or_else(|| program.card_fee_cents(sale.amount_cents));
        let net_cents = sale.amount_cents.saturating_sub(tax).saturating_sub(fee);
        let excluded = match (sale.link, sale.referrer.as_deref()) {
            (LinkKind::Go, _) | (LinkKind::Affiliate, None) => Some(Excluded::Campaign),
            (LinkKind::Affiliate, Some(referrer))
                if directory.self_referral(referrer, &sale.account, sale.workspace.as_deref()) =>
            {
                Some(Excluded::SelfReferral)
            }
            _ if sale.currency.as_deref() != Some("USD") => Some(Excluded::Currency),
            _ if *month > program.paid_months => Some(Excluded::PastTerm),
            _ => None,
        };
        let payable_at = sale.at.saturating_add(program.hold_secs);
        let reversed_at = reversals.get(sale.invoice.as_str()).copied();
        let state = match reversed_at {
            Some(r) if r <= payable_at => State::Reversed,
            Some(r) if r <= now => State::ClawedBack,
            _ if now < payable_at => State::Held,
            _ => State::Payable,
        };
        let commission_micros = if excluded.is_some() {
            0
        } else {
            program.commission_micros(net_cents)
        };
        if let (None, Some(referrer)) = (excluded, sale.referrer.as_ref()) {
            let b = out.balances.entry(referrer.clone()).or_default();
            if reversed_at.is_some_and(|r| r <= now) {
                b.refunds += 1;
            }
            match state {
                State::Held => {
                    b.accrued += commission_micros;
                    b.held += commission_micros;
                    b.paid_conversions += 1;
                }
                State::Payable => {
                    b.accrued += commission_micros;
                    b.payable += commission_micros;
                    b.paid_conversions += 1;
                }
                State::ClawedBack => {
                    // Earned and payable once, then taken back.
                    b.accrued += commission_micros;
                    b.payable += commission_micros;
                    b.clawed_back += commission_micros;
                    b.paid_conversions += 1;
                }
                State::Reversed => {}
            }
        }
        out.items.push(Item {
            referrer: sale.referrer.clone(),
            account: sale.account.clone(),
            invoice: sale.invoice.clone(),
            month: *month,
            paid_at: sale.at,
            payable_at,
            net_cents,
            commission_micros,
            reversed_at,
            state,
            excluded,
        });
    }
    for payout in payouts.iter().filter(|p| p.at <= now) {
        out.balances
            .entry(payout.referrer.clone())
            .or_default()
            .paid += payout.usd_micros;
    }
    for b in out.balances.values_mut() {
        let released = b.payable;
        let taken = b.clawed_back + b.paid;
        b.payable = released.saturating_sub(taken);
        b.owed = taken.saturating_sub(released);
    }
    out
}

/// One affiliate the owner may pay this month.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Due {
    pub referrer: String,
    pub usd_micros: u64,
    pub sats: u64,
}

/// Sats for `usd_micros` at `usd_per_btc`, rounded down to a whole sat.
#[must_use]
pub fn sats(usd_micros: u64, usd_per_btc: u64) -> Option<u64> {
    if usd_per_btc == 0 {
        return None;
    }
    // sats = micros / 1e6 USD * 1e8 sats per BTC / usd_per_btc.
    u64::try_from(u128::from(usd_micros) * 100 / u128::from(usd_per_btc)).ok()
}

/// The affiliates whose payable balance, at `usd_per_btc`, reaches the
/// program's minimum. A list for the owner; nothing is sent.
#[must_use]
pub fn payouts_due(program: &Program, ledger: &Ledger, usd_per_btc: u64) -> Vec<Due> {
    ledger
        .balances
        .iter()
        .filter_map(|(referrer, b)| {
            let sats = sats(b.payable, usd_per_btc)?;
            (sats >= program.minimum_sats).then(|| Due {
                referrer: referrer.clone(),
                usd_micros: b.payable,
                sats,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
