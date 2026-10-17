//! Operator entry point for an explicitly reviewed CAD history copy.
use std::sync::Arc;

/// Copy one selected account or project without changing the legacy objects.
/// The caller supplies operator storage access and the approved source checksum.
pub async fn copy(
    source: crate::chat_store::Store,
    backend: Arc<dyn lab_store::backend::Backend>,
    account: &str,
    project: Option<&str>,
    approved_checksum: &str,
) -> Result<serde_json::Value, String> {
    if account.is_empty()
        || account.len() > 256
        || approved_checksum.len() != 64
        || !approved_checksum
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("Choose an account and its approved CAD review checksum.".into());
    }
    let owner = crate::chat_store::account_owner(account);
    let source = source.with_labs(Some(backend.clone()));
    lab_store::Store::new(
        backend,
        &lab_kit::SpaceRef::Personal {
            account: account.into(),
        },
    )
    .map_err(|_| "The CAD destination couldn't be opened.")?
    .bind_alias(&owner)
    .await
    .map_err(|_| "The CAD destination couldn't be opened.")?;
    crate::cad_web::copy_history(&source, &owner, project, approved_checksum).await
}
