//! Breez's storage conformance suite, run against [`FileStorage`], and a
//! check that a reopened file restores what was written.

use breez_sdk_spark::Storage;
use breez_sdk_spark::storage_tests as suite;
use openagents_spark::store::FileStorage;

fn store() -> (tempfile::TempDir, Box<dyn Storage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let storage =
        FileStorage::open(&dir.path().join("wallet/store.json")).expect("the store opens");
    (dir, Box::new(storage))
}

#[tokio::test]
async fn test_sync_storage() {
    let (_dir, storage) = store();
    Box::pin(suite::test_sync_storage(storage)).await;
}

#[tokio::test]
async fn test_storage() {
    let (_dir, storage) = store();
    Box::pin(suite::test_storage(storage)).await;
}

#[tokio::test]
async fn test_watched_deposit_addresses() {
    let (_dir, storage) = store();
    Box::pin(suite::test_watched_deposit_addresses(storage)).await;
}

#[tokio::test]
async fn test_unclaimed_deposits_crud() {
    let (_dir, storage) = store();
    Box::pin(suite::test_unclaimed_deposits_crud(storage)).await;
}

#[tokio::test]
async fn test_deposit_refunds() {
    let (_dir, storage) = store();
    Box::pin(suite::test_deposit_refunds(storage)).await;
}

#[tokio::test]
async fn test_instant_claim_status() {
    let (_dir, storage) = store();
    Box::pin(suite::test_instant_claim_status(storage)).await;
}

#[tokio::test]
async fn test_deposit_max_claim_fee() {
    let (_dir, storage) = store();
    Box::pin(suite::test_deposit_max_claim_fee(storage)).await;
}

#[tokio::test]
async fn test_payment_type_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_type_filtering(storage)).await;
}

#[tokio::test]
async fn test_payment_status_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_status_filtering(storage)).await;
}

#[tokio::test]
async fn test_asset_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_asset_filtering(storage)).await;
}

#[tokio::test]
async fn test_spark_htlc_status_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_spark_htlc_status_filtering(storage)).await;
}

#[tokio::test]
async fn test_conversion_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_conversion_filtering(storage)).await;
}

#[tokio::test]
async fn test_token_transaction_type_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_token_transaction_type_filtering(storage)).await;
}

#[tokio::test]
async fn test_timestamp_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_timestamp_filtering(storage)).await;
}

#[tokio::test]
async fn test_combined_filters() {
    let (_dir, storage) = store();
    Box::pin(suite::test_combined_filters(storage)).await;
}

#[tokio::test]
async fn test_sort_order() {
    let (_dir, storage) = store();
    Box::pin(suite::test_sort_order(storage)).await;
}

#[tokio::test]
async fn test_payment_metadata() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_metadata(storage)).await;
}

#[tokio::test]
async fn test_payment_details_update_persistence() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_details_update_persistence(storage)).await;
}

#[tokio::test]
async fn test_payment_terminal_status_is_not_replaced() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_terminal_status_is_not_replaced(storage)).await;
}

#[tokio::test]
async fn test_payment_metadata_merge() {
    let (_dir, storage) = store();
    Box::pin(suite::test_payment_metadata_merge(storage)).await;
}

#[tokio::test]
async fn test_lightning_htlc_details_and_status_filtering() {
    let (_dir, storage) = store();
    Box::pin(suite::test_lightning_htlc_details_and_status_filtering(
        storage,
    ))
    .await;
}

#[tokio::test]
async fn test_contacts_crud() {
    let (_dir, storage) = store();
    Box::pin(suite::test_contacts_crud(storage)).await;
}

#[tokio::test]
async fn test_cross_chain_swaps_crud() {
    let (_dir, storage) = store();
    Box::pin(suite::test_cross_chain_swaps_crud(storage)).await;
}

#[tokio::test]
async fn test_conversion_status_persistence() {
    let (_dir, storage) = store();
    Box::pin(suite::test_conversion_status_persistence(storage)).await;
}

#[tokio::test]
async fn test_insert_boltz_conversion_info() {
    let (_dir, storage) = store();
    Box::pin(suite::test_insert_boltz_conversion_info(storage)).await;
}

#[tokio::test]
async fn test_update_boltz_status_to_completed() {
    let (_dir, storage) = store();
    Box::pin(suite::test_update_boltz_status_to_completed(storage)).await;
}

#[tokio::test]
async fn a_reopened_store_keeps_its_records() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("wallet/store.json");
    {
        let storage = FileStorage::open(&path).expect("the store opens");
        storage
            .set_cached_item("key".into(), "value".into())
            .await
            .expect("set");
        storage
            .add_deposit("txid".into(), 1, 5_000, true)
            .await
            .expect("deposit");
    }
    let storage = FileStorage::open(&path).expect("the store opens again");
    assert_eq!(
        storage.get_cached_item("key".into()).await.expect("get"),
        Some("value".into())
    );
    let deposits = storage.list_deposits().await.expect("list");
    assert_eq!(deposits.len(), 1);
    assert_eq!(deposits[0].amount_sats, 5_000);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .expect("the directory")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700);
    }
}
