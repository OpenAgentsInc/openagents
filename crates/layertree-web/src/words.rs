//! What the page says, in plain words: the caption for each event, the
//! ticker's short lines, and the legend. The tests run the site's copy guard
//! over every sentence here.

/// A number with thousands separators.
#[must_use]
pub fn number(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// An amount of bitcoin in sats.
#[must_use]
pub fn sats(value: u64) -> String {
    format!("{} sats", number(value))
}

/// Which run the page plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The seeded, simulated run.
    Demo,
    /// The real run on Bitcoin's test network.
    Signet,
}

pub const TITLE: &str = "LayerTree";

/// The line under the title.
#[must_use]
pub fn subtitle(source: Source) -> &'static str {
    match source {
        Source::Demo => "A simulated run: thousands of people share one Bitcoin coin",
        Source::Signet => "The real test run on Bitcoin's test network",
    }
}

/// The opening caption.
#[must_use]
pub fn opened(members: usize, total: u64) -> String {
    format!(
        "{} people share one Bitcoin coin worth {}. One signer keeps the books.",
        number(members as u64),
        sats(total)
    )
}

pub fn joined(amount: u64) -> String {
    format!("Someone joins with {}.", sats(amount))
}

pub const UPDATED: &str = "A member moves their share to a new key.";

pub fn paid(amount: u64) -> String {
    format!("A member pays another member {}.", sats(amount))
}

pub fn split(amount: u64) -> String {
    format!("A member splits off {} into a second share.", sats(amount))
}

pub fn left(amount: u64) -> String {
    format!("A member leaves and takes {} out to Bitcoin.", sats(amount))
}

pub const REFRESHED: &str = "The signer renews the shared coin on Bitcoin.";
pub const CHECKED: &str = "The signer proves it runs the published code.";
pub const CHECK_FAILED: &str = "The signer failed its check.";
pub const FOLLOWED: &str = "Members' phones check each new block on their own.";
pub const FOLLOW_ALARM: &str = "A member's phone found a problem and raised the alarm.";
pub const SWITCHED_OFF: &str =
    "The signer is switched off. Nobody can move the money now, but members can still get it back.";

pub fn waiting(blocks: u32) -> String {
    if blocks == 1 {
        "One more block before members can take their money back.".into()
    } else {
        format!("Waiting {blocks} blocks before members can take their money back.")
    }
}

pub fn broadcast(level: u32) -> String {
    if level == 0 {
        "Members publish the tree to Bitcoin, one layer at a time.".into()
    } else {
        format!("Layer {} goes to Bitcoin.", level + 1)
    }
}

pub fn confirmed(level: u32) -> String {
    format!("Layer {} is confirmed on Bitcoin.", level + 1)
}

pub fn payouts(paid: usize, owed: usize, amount: u64) -> String {
    format!(
        "{} of {} members paid back: {}.",
        number(paid as u64),
        number(owed as u64),
        sats(amount)
    )
}

pub const EVERYONE_PAID: &str = "Everyone got their money back. Starting again.";

/// Short lines for the ticker of recent transactions.
pub mod ticker {
    use super::sats;

    pub fn joined(amount: u64) -> String {
        format!("Joined  {}", sats(amount))
    }
    pub const UPDATED: &str = "New key";
    pub fn paid(amount: u64) -> String {
        format!("Paid  {}", sats(amount))
    }
    pub fn split(amount: u64) -> String {
        format!("Split  {}", sats(amount))
    }
    pub fn left(amount: u64) -> String {
        format!("Left  {}", sats(amount))
    }
    pub const REFRESHED: &str = "Coin renewed";
}

/// The legend: a color or shape, and what it means.
pub const LEGEND: [&str; 5] = [
    "A member's money",
    "The shared Bitcoin coin",
    "The signer",
    "Members' phones following each block",
    "Being paid back",
];

pub const HINT: &str = "Drag to turn, scroll to zoom";

/// The hint to switch runs.
#[must_use]
pub fn switch_hint(source: Source) -> &'static str {
    match source {
        Source::Demo => "Press R for the real test run",
        Source::Signet => "Press R for the big simulated run",
    }
}

pub fn members(count: usize) -> String {
    format!("{} members", number(count as u64))
}

pub fn block(height: u32) -> String {
    format!("Block {}", number(u64::from(height)))
}

pub const SIGNER: &str = "Signer";
pub const SIGNER_OK: &str = "Running the published code";
pub const SIGNER_OFF: &str = "Switched off";
pub const SIGNER_UNCHECKED: &str = "Not checked yet";
pub const COIN: &str = "Shared Bitcoin coin";
pub const FOLLOWERS: &str = "Members' phones";
pub const BLOCKS: &str = "Bitcoin blocks";

#[cfg(test)]
mod tests {
    use super::*;

    fn every_sentence() -> Vec<String> {
        let mut out = vec![
            subtitle(Source::Demo).into(),
            subtitle(Source::Signet).into(),
            opened(2_600, 1_234_567),
            joined(5),
            UPDATED.into(),
            paid(5),
            split(5),
            left(5),
            REFRESHED.into(),
            CHECKED.into(),
            CHECK_FAILED.into(),
            FOLLOWED.into(),
            FOLLOW_ALARM.into(),
            SWITCHED_OFF.into(),
            waiting(1),
            waiting(9),
            broadcast(0),
            broadcast(2),
            confirmed(1),
            payouts(3, 9, 99),
            EVERYONE_PAID.into(),
            ticker::joined(5),
            ticker::UPDATED.into(),
            ticker::paid(5),
            ticker::split(5),
            ticker::left(5),
            ticker::REFRESHED.into(),
            HINT.into(),
            switch_hint(Source::Demo).into(),
            switch_hint(Source::Signet).into(),
            members(3),
            block(912_000),
            SIGNER.into(),
            SIGNER_OK.into(),
            SIGNER_OFF.into(),
            SIGNER_UNCHECKED.into(),
            COIN.into(),
            FOLLOWERS.into(),
            BLOCKS.into(),
            TITLE.into(),
        ];
        out.extend(LEGEND.iter().map(|s| s.to_string()));
        out
    }

    #[test]
    fn every_sentence_passes_the_copy_guard() {
        for sentence in every_sentence() {
            let found = oa_copy::violations(&sentence, &[]);
            assert!(found.is_empty(), "{sentence}: {found:?}");
        }
    }

    #[test]
    fn numbers_read_with_separators() {
        assert_eq!(number(0), "0");
        assert_eq!(number(999), "999");
        assert_eq!(number(1_000), "1,000");
        assert_eq!(number(1_234_567_890), "1,234,567,890");
        assert_eq!(sats(2_500), "2,500 sats");
    }
}
