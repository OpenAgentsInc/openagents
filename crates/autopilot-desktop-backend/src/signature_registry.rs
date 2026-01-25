use dsrs::signature_registry as registry;
use tauri::command;

use crate::contracts::ipc::{
    DsrsSignatureInfo, GetDsrsSignatureRequest, GetDsrsSignatureResponse,
    ListDsrsSignaturesResponse,
};

impl From<registry::DsrsSignatureInfo> for DsrsSignatureInfo {
    fn from(signature: registry::DsrsSignatureInfo) -> Self {
        Self {
            name: signature.name,
            instruction: signature.instruction,
            input_fields: signature.input_fields,
            output_fields: signature.output_fields,
        }
    }
}

#[command]
pub fn list_dsrs_signatures() -> ListDsrsSignaturesResponse {
    let signatures = registry::list_signatures()
        .into_iter()
        .map(DsrsSignatureInfo::from)
        .collect();

    ListDsrsSignaturesResponse { signatures }
}

#[command]
pub fn get_dsrs_signature(
    request: GetDsrsSignatureRequest,
) -> Result<GetDsrsSignatureResponse, String> {
    let signature = registry::get_signature(&request.name)
        .ok_or_else(|| format!("Signature not found: {}", request.name))?;

    Ok(GetDsrsSignatureResponse {
        signature: signature.into(),
    })
}
