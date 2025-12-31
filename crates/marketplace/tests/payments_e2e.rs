use marketplace::coalitions::{Contribution, PaymentPool};
use marketplace::ledger::{
    Balance, Direction, LedgerAmounts, LedgerEntry, LedgerEntryType, LedgerFilters,
    LedgerOperation, LedgerParties, LedgerReferences,
};

#[test]
fn test_ledger_amounts_creation() {
    let amounts = LedgerAmounts::new(10000, 500);

    assert_eq!(amounts.gross_sats, 10000);
    assert_eq!(amounts.platform_fee_sats, 500);
    assert_eq!(amounts.net_sats, 9500);
    assert!(amounts.validate().is_ok());
}

#[test]
fn test_ledger_amounts_from_rate() {
    let amounts = LedgerAmounts::from_gross_with_rate(10000, 0.05);

    assert_eq!(amounts.gross_sats, 10000);
    assert_eq!(amounts.platform_fee_sats, 500);
    assert_eq!(amounts.net_sats, 9500);
}

#[test]
fn test_ledger_parties() {
    let parties = LedgerParties::new("payer1", "payee1")
        .with_intermediary("platform")
        .with_intermediary("coordinator");

    assert_eq!(parties.payer, "payer1");
    assert_eq!(parties.payee, "payee1");
    assert_eq!(parties.intermediaries.len(), 2);
}

#[test]
fn test_ledger_references() {
    let refs = LedgerReferences::new()
        .with_job_id("job-123")
        .with_invoice_id("inv-456")
        .with_tx_hash("lntx789")
        .with_coalition_id("coalition-001");

    assert_eq!(refs.job_id.as_deref(), Some("job-123"));
    assert_eq!(refs.invoice_id.as_deref(), Some("inv-456"));
    assert_eq!(refs.tx_hash.as_deref(), Some("lntx789"));
    assert_eq!(refs.coalition_id.as_deref(), Some("coalition-001"));
}

#[test]
fn test_ledger_entry_creation_and_finalization() {
    let amounts = LedgerAmounts::new(10000, 500);
    let parties = LedgerParties::new("payer1", "payee1");
    let refs = LedgerReferences::new().with_job_id("job-123");

    let entry = LedgerEntry::new(
        "entry-001",
        LedgerEntryType::ComputePayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        amounts,
        parties,
        refs,
        "genesis",
    )
    .unwrap()
    .finalize();

    assert_eq!(entry.id, "entry-001");
    assert_eq!(entry.entry_type, LedgerEntryType::ComputePayment);
    assert_eq!(entry.direction, Direction::Outbound);
    assert_eq!(entry.operation, LedgerOperation::Debit);
    assert!(!entry.entry_hash.is_empty());
    assert!(entry.verify_hash().is_ok());
}

#[test]
fn test_ledger_entry_types() {
    assert_eq!(
        LedgerEntryType::SkillPayment.description(),
        "Payment for skill usage"
    );
    assert_eq!(
        LedgerEntryType::ComputePayment.description(),
        "Payment for compute job"
    );
    assert_eq!(
        LedgerEntryType::DataPayment.description(),
        "Payment for purchased data"
    );
    assert_eq!(
        LedgerEntryType::CoalitionSettlement.description(),
        "Coalition payment settlement"
    );
    assert_eq!(LedgerEntryType::Refund.description(), "Refund to payer");
    assert_eq!(
        LedgerEntryType::Payout.description(),
        "Payout to provider/creator"
    );
    assert_eq!(LedgerEntryType::TopUp.description(), "Account top-up");
}

#[test]
fn test_ledger_operations() {
    assert!(LedgerOperation::Credit.increases_available());
    assert!(LedgerOperation::Release.increases_available());
    assert!(LedgerOperation::Refund.increases_available());
    assert!(!LedgerOperation::Debit.increases_available());
    assert!(!LedgerOperation::Hold.increases_available());

    assert!(LedgerOperation::Debit.decreases_available());
    assert!(LedgerOperation::Hold.decreases_available());
    assert!(!LedgerOperation::Credit.decreases_available());
    assert!(!LedgerOperation::Release.decreases_available());
}

#[test]
fn test_balance_credit_operation() {
    let mut balance = Balance::new("account1");

    assert_eq!(balance.available_sats, 0);
    assert_eq!(balance.total_sats, 0);

    balance.credit(10000);
    assert_eq!(balance.available_sats, 10000);
    assert_eq!(balance.total_sats, 10000);
    assert_eq!(balance.held_sats, 0);

    balance.credit(5000);
    assert_eq!(balance.available_sats, 15000);
    assert_eq!(balance.total_sats, 15000);
}

#[test]
fn test_balance_debit_operation() {
    let mut balance = Balance::new("account1");
    balance.credit(10000);

    assert!(balance.debit(5000).is_ok());
    assert_eq!(balance.available_sats, 5000);
    assert_eq!(balance.total_sats, 5000);

    assert!(balance.debit(10000).is_err());
    assert_eq!(balance.available_sats, 5000);
}

#[test]
fn test_balance_hold_and_release() {
    let mut balance = Balance::new("account1");
    balance.credit(10000);

    assert!(balance.hold(3000).is_ok());
    assert_eq!(balance.available_sats, 7000);
    assert_eq!(balance.held_sats, 3000);
    assert_eq!(balance.total_sats, 10000);

    assert!(balance.release(1000).is_ok());
    assert_eq!(balance.available_sats, 8000);
    assert_eq!(balance.held_sats, 2000);
    assert_eq!(balance.total_sats, 10000);

    assert!(balance.release(5000).is_err());
}

#[test]
fn test_balance_hold_insufficient_funds() {
    let mut balance = Balance::new("account1");
    balance.credit(1000);

    assert!(balance.hold(2000).is_err());
    assert_eq!(balance.available_sats, 1000);
    assert_eq!(balance.held_sats, 0);
}

#[test]
fn test_balance_apply_ledger_entry() {
    let mut balance = Balance::new("account1");

    let credit_amounts = LedgerAmounts::new(10000, 0);
    let credit_parties = LedgerParties::new("external", "account1");
    let credit_refs = LedgerReferences::new();

    let credit_entry = LedgerEntry::new(
        "entry-credit",
        LedgerEntryType::TopUp,
        Direction::Inbound,
        LedgerOperation::Credit,
        credit_amounts,
        credit_parties,
        credit_refs,
        "genesis",
    )
    .unwrap()
    .finalize();

    balance.apply_entry(&credit_entry).unwrap();
    assert_eq!(balance.available_sats, 10000);

    let debit_amounts = LedgerAmounts::new(5000, 0);
    let debit_parties = LedgerParties::new("account1", "provider1");
    let debit_refs = LedgerReferences::new();

    let debit_entry = LedgerEntry::new(
        "entry-debit",
        LedgerEntryType::ComputePayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        debit_amounts,
        debit_parties,
        debit_refs,
        &credit_entry.entry_hash,
    )
    .unwrap()
    .finalize();

    balance.apply_entry(&debit_entry).unwrap();
    assert_eq!(balance.available_sats, 5000);
}

#[test]
fn test_ledger_filters() {
    let filters = LedgerFilters::new()
        .with_entry_type(LedgerEntryType::ComputePayment)
        .with_direction(Direction::Outbound)
        .with_limit(10);

    assert_eq!(filters.entry_type, Some(LedgerEntryType::ComputePayment));
    assert_eq!(filters.direction, Some(Direction::Outbound));
    assert_eq!(filters.limit, Some(10));
}

#[test]
fn test_ledger_filters_matching() {
    let filters = LedgerFilters::new()
        .with_entry_type(LedgerEntryType::SkillPayment)
        .with_direction(Direction::Inbound);

    let matching_amounts = LedgerAmounts::new(1000, 50);
    let matching_parties = LedgerParties::new("user", "creator");
    let matching_refs = LedgerReferences::new();

    let matching_entry = LedgerEntry::new(
        "entry-match",
        LedgerEntryType::SkillPayment,
        Direction::Inbound,
        LedgerOperation::Credit,
        matching_amounts,
        matching_parties,
        matching_refs,
        "genesis",
    )
    .unwrap()
    .finalize();

    assert!(filters.matches(&matching_entry));

    let non_matching_amounts = LedgerAmounts::new(2000, 100);
    let non_matching_parties = LedgerParties::new("user", "provider");
    let non_matching_refs = LedgerReferences::new();

    let non_matching_entry = LedgerEntry::new(
        "entry-nomatch",
        LedgerEntryType::ComputePayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        non_matching_amounts,
        non_matching_parties,
        non_matching_refs,
        "genesis",
    )
    .unwrap()
    .finalize();

    assert!(!filters.matches(&non_matching_entry));
}

#[test]
fn test_payment_pool_creation() {
    let pool = PaymentPool::new(100_000);

    assert_eq!(pool.total_sats, 100_000);
    assert!(pool.contributions.is_empty());
    assert!(!pool.settled);
}

#[test]
fn test_payment_pool_contributions() {
    let mut pool = PaymentPool::new(100_000);

    pool.add_contribution(Contribution::new("agent1", "development", 0.5));
    pool.add_contribution(Contribution::new("agent2", "testing", 0.3));
    pool.add_contribution(Contribution::new("agent3", "review", 0.2));

    assert_eq!(pool.contributions.len(), 3);
    assert_eq!(pool.total_weight(), 1.0);
}

#[test]
fn test_payment_pool_settlement() {
    let mut pool = PaymentPool::new(100_000);

    pool.add_contribution(Contribution::new("agent1", "development", 0.6));
    pool.add_contribution(Contribution::new("agent2", "testing", 0.4));

    let splits = pool.settle();

    assert_eq!(splits.len(), 2);
    assert!(pool.settled);

    let total: u64 = splits.iter().map(|s| s.amount_sats).sum();
    assert_eq!(total, 100_000);

    let agent1_split = splits.iter().find(|s| s.agent_id == "agent1").unwrap();
    let agent2_split = splits.iter().find(|s| s.agent_id == "agent2").unwrap();
    assert_eq!(agent1_split.amount_sats, 60_000);
    assert_eq!(agent2_split.amount_sats, 40_000);
}

#[test]
fn test_payment_pool_settlement_handles_remainder() {
    let mut pool = PaymentPool::new(100);

    pool.add_contribution(Contribution::new("agent1", "work", 1.0));
    pool.add_contribution(Contribution::new("agent2", "work", 1.0));
    pool.add_contribution(Contribution::new("agent3", "work", 1.0));

    let splits = pool.settle();

    let total: u64 = splits.iter().map(|s| s.amount_sats).sum();
    assert_eq!(total, 100);
}

#[test]
fn test_payment_pool_double_settle() {
    let mut pool = PaymentPool::new(100_000);
    pool.add_contribution(Contribution::new("agent1", "work", 1.0));

    let first_settle = pool.settle();
    assert!(!first_settle.is_empty());

    let second_settle = pool.settle();
    assert!(second_settle.is_empty());
}

#[test]
fn test_contribution_with_receipts() {
    let contrib = Contribution::new("agent1", "development", 0.5)
        .with_receipt("receipt-hash-1")
        .with_receipt("receipt-hash-2");

    assert_eq!(contrib.receipts.len(), 2);
    assert_eq!(contrib.agent_id, "agent1");
    assert_eq!(contrib.work_type, "development");
    assert_eq!(contrib.weight, 0.5);
}

#[test]
fn test_ledger_entry_chain_integrity() {
    let amounts1 = LedgerAmounts::new(10000, 0);
    let parties1 = LedgerParties::new("source", "account1");
    let refs1 = LedgerReferences::new();

    let entry1 = LedgerEntry::new(
        "entry-1",
        LedgerEntryType::TopUp,
        Direction::Inbound,
        LedgerOperation::Credit,
        amounts1,
        parties1,
        refs1,
        "genesis",
    )
    .unwrap()
    .finalize();

    let amounts2 = LedgerAmounts::new(5000, 250);
    let parties2 = LedgerParties::new("account1", "provider");
    let refs2 = LedgerReferences::new().with_job_id("job-1");

    let entry2 = LedgerEntry::new(
        "entry-2",
        LedgerEntryType::ComputePayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        amounts2,
        parties2,
        refs2,
        &entry1.entry_hash,
    )
    .unwrap()
    .finalize();

    assert_eq!(entry2.previous_hash, entry1.entry_hash);
    assert!(entry2.verify_hash().is_ok());

    let amounts3 = LedgerAmounts::new(1000, 50);
    let parties3 = LedgerParties::new("account1", "skill_creator");
    let refs3 = LedgerReferences::new();

    let entry3 = LedgerEntry::new(
        "entry-3",
        LedgerEntryType::SkillPayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        amounts3,
        parties3,
        refs3,
        &entry2.entry_hash,
    )
    .unwrap()
    .finalize();

    assert_eq!(entry3.previous_hash, entry2.entry_hash);
    assert!(entry3.verify_hash().is_ok());
}

#[test]
fn test_platform_fee_flow() {
    let mut user_balance = Balance::new("user1");
    let mut creator_balance = Balance::new("creator1");
    let mut platform_balance = Balance::new("platform");

    user_balance.credit(10000);

    let gross_amount: u64 = 1000;
    let platform_fee_rate = 0.10;
    let amounts = LedgerAmounts::from_gross_with_rate(gross_amount, platform_fee_rate);

    assert_eq!(amounts.net_sats, 900);
    assert_eq!(amounts.platform_fee_sats, 100);

    user_balance.debit(gross_amount).unwrap();
    creator_balance.credit(amounts.net_sats);
    platform_balance.credit(amounts.platform_fee_sats);

    assert_eq!(user_balance.available_sats, 9000);
    assert_eq!(creator_balance.available_sats, 900);
    assert_eq!(platform_balance.available_sats, 100);
}

#[test]
fn test_ledger_entry_serde() {
    let amounts = LedgerAmounts::new(5000, 250);
    let parties = LedgerParties::new("payer", "payee");
    let refs = LedgerReferences::new().with_job_id("job-1");

    let entry = LedgerEntry::new(
        "entry-serde",
        LedgerEntryType::ComputePayment,
        Direction::Outbound,
        LedgerOperation::Debit,
        amounts,
        parties,
        refs,
        "genesis",
    )
    .unwrap()
    .finalize();

    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: LedgerEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(entry.id, deserialized.id);
    assert_eq!(entry.entry_hash, deserialized.entry_hash);
    assert_eq!(entry.amounts.gross_sats, deserialized.amounts.gross_sats);
}

#[test]
fn test_balance_serde() {
    let mut balance = Balance::new("test-account");
    balance.credit(50000);
    balance.hold(10000).unwrap();

    let json = serde_json::to_string(&balance).unwrap();
    let deserialized: Balance = serde_json::from_str(&json).unwrap();

    assert_eq!(balance.account_id, deserialized.account_id);
    assert_eq!(balance.available_sats, deserialized.available_sats);
    assert_eq!(balance.held_sats, deserialized.held_sats);
    assert_eq!(balance.total_sats, deserialized.total_sats);
}

#[test]
fn test_refund_flow() {
    let mut user_balance = Balance::new("user1");
    let mut provider_balance = Balance::new("provider1");

    user_balance.credit(10000);
    user_balance.debit(5000).unwrap();
    provider_balance.credit(5000);

    assert_eq!(user_balance.available_sats, 5000);
    assert_eq!(provider_balance.available_sats, 5000);

    let refund_amount: u64 = 5000;
    provider_balance.debit(refund_amount).unwrap();

    let refund_amounts = LedgerAmounts::new(refund_amount, 0);
    let refund_parties = LedgerParties::new("provider1", "user1");
    let refund_refs = LedgerReferences::new();

    let refund_entry = LedgerEntry::new(
        "refund-1",
        LedgerEntryType::Refund,
        Direction::Inbound,
        LedgerOperation::Refund,
        refund_amounts,
        refund_parties,
        refund_refs,
        "previous-hash",
    )
    .unwrap()
    .finalize();

    user_balance.apply_entry(&refund_entry).unwrap();

    assert_eq!(user_balance.available_sats, 10000);
    assert_eq!(provider_balance.available_sats, 0);
}
