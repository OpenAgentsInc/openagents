use super::*;

pub(super) fn owner_from_parts(
    user_id: Option<u64>,
    guest_scope: Option<String>,
) -> Result<WorkerOwner, ApiError> {
    let owner = WorkerOwner {
        user_id,
        guest_scope,
    };
    if owner.is_valid() {
        Ok(owner)
    } else {
        Err(ApiError::InvalidRequest(
            "owner_user_id or owner_guest_scope must be provided (but not both)".to_string(),
        ))
    }
}

pub(super) fn fanout_window_details(
    window: Option<&FanoutTopicWindow>,
) -> (Option<u64>, Option<u64>, Option<usize>, Option<u64>) {
    match window {
        Some(window) => (
            Some(window.oldest_sequence.saturating_sub(1)),
            Some(window.head_sequence),
            Some(window.queue_depth),
            Some(window.dropped_messages),
        ),
        None => (None, None, None, None),
    }
}

pub(super) fn khala_principal_key(principal: &SyncPrincipal) -> String {
    let user = principal
        .user_id
        .map(|value| format!("user:{value}"))
        .unwrap_or_else(|| "user:none".to_string());
    let org = principal
        .org_id
        .clone()
        .unwrap_or_else(|| "org:none".to_string());
    let device = principal
        .device_id
        .clone()
        .unwrap_or_else(|| "device:none".to_string());
    format!("{user}|{org}|{device}")
}

pub(super) fn khala_consumer_key(principal: &SyncPrincipal, topic: &str) -> String {
    format!("{}|{topic}", khala_principal_key(principal))
}

pub(super) fn deterministic_jitter_ms(seed_key: &str, cursor: u64, max_jitter_ms: u64) -> u64 {
    if max_jitter_ms == 0 {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    seed_key.hash(&mut hasher);
    cursor.hash(&mut hasher);
    hasher.finish() % (max_jitter_ms.saturating_add(1))
}

pub(super) fn ensure_runtime_write_authority(state: &AppState) -> Result<(), ApiError> {
    if state.config.authority_write_mode.writes_enabled() {
        Ok(())
    } else {
        Err(ApiError::WritePathFrozen(format!(
            "runtime authority writes are disabled in mode {}",
            state.config.authority_write_mode.as_str()
        )))
    }
}

pub(super) const PHASE0_REQUIRED_PROVIDER_CAPABILITY: &str = "oa.sandbox_run.v1";

pub(super) fn metadata_has_role(metadata: &serde_json::Value, role: &str) -> bool {
    metadata_string_array(metadata, "roles")
        .iter()
        .any(|value| value == role)
}

pub(super) fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata.get(key)?.as_str().map(|value| value.to_string())
}

pub(super) fn metadata_string_array(metadata: &serde_json::Value, key: &str) -> Vec<String> {
    match metadata.get(key).and_then(|value| value.as_array()) {
        Some(values) => values
            .iter()
            .filter_map(|value| value.as_str())
            .map(|value| value.to_string())
            .collect(),
        None => Vec::new(),
    }
}

pub(super) fn merge_metadata_patch_shallow(
    target: &mut serde_json::Value,
    patch: &serde_json::Value,
) -> Result<(), ApiError> {
    let Some(patch_map) = patch.as_object() else {
        return Err(ApiError::InvalidRequest(
            "metadata_patch must be a JSON object".to_string(),
        ));
    };
    let target_map = target
        .as_object_mut()
        .ok_or_else(|| ApiError::Internal("worker metadata must be a JSON object".to_string()))?;
    for (key, value) in patch_map {
        target_map.insert(key.clone(), value.clone());
    }
    Ok(())
}

pub(super) fn metadata_patch_touches_provider_listing(patch: &serde_json::Value) -> bool {
    let Some(map) = patch.as_object() else {
        return false;
    };

    let listing_keys = [
        "name",
        "description",
        "website",
        "capabilities",
        "min_price_msats",
        "pricing_stage",
        "pricing_bands",
        "caps",
        "reserve_pool",
        "supply_class",
        "tier",
        "quarantined",
        "quarantine_reason",
    ];

    listing_keys.iter().any(|key| map.contains_key(*key))
}

pub(super) fn annotate_provider_metadata(metadata: &mut serde_json::Value) {
    if let Some(map) = metadata.as_object_mut() {
        map.insert("qualified".to_string(), serde_json::Value::Bool(true));
        map.insert(
            "qualified_at".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339()),
        );
        if !map.contains_key("failure_strikes") {
            map.insert(
                "failure_strikes".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0_u64)),
            );
        }
        if !map.contains_key("quarantined") {
            map.insert("quarantined".to_string(), serde_json::Value::Bool(false));
        }
        if !map.contains_key("success_count") {
            map.insert(
                "success_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(0_u64)),
            );
        }
    }
}

pub(super) const PROVIDER_PRICING_STAGE_MAX: PricingStage = PricingStage::Banded;

pub(super) fn pricing_stage_from_metadata(metadata: &serde_json::Value) -> PricingStage {
    let Some(value) = metadata
        .get("pricing_stage")
        .and_then(serde_json::Value::as_str)
    else {
        return PricingStage::Fixed;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "fixed" => PricingStage::Fixed,
        "banded" => PricingStage::Banded,
        "bidding" => PricingStage::Bidding,
        _ => PricingStage::Fixed,
    }
}

pub(super) fn pricing_stage_rank(stage: &PricingStage) -> u8 {
    match stage {
        PricingStage::Fixed => 0,
        PricingStage::Banded => 1,
        PricingStage::Bidding => 2,
    }
}

pub(super) fn qualify_provider_pricing(metadata: &serde_json::Value) -> Result<(), ApiError> {
    let stage = pricing_stage_from_metadata(metadata);
    if pricing_stage_rank(&stage) > pricing_stage_rank(&PROVIDER_PRICING_STAGE_MAX) {
        return Err(ApiError::InvalidRequest(format!(
            "pricing_stage {:?} is not enabled in this deployment",
            stage
        )));
    }

    if stage != PricingStage::Banded {
        return Ok(());
    }

    let capabilities = metadata_string_array(metadata, "capabilities");
    let bands_value = metadata
        .get("pricing_bands")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ApiError::InvalidRequest(
                "pricing_bands[] is required when pricing_stage=banded".to_string(),
            )
        })?;
    if bands_value.is_empty() {
        return Err(ApiError::InvalidRequest(
            "pricing_bands[] is required when pricing_stage=banded".to_string(),
        ));
    }

    let bands: Vec<PricingBand> =
        serde_json::from_value(serde_json::Value::Array(bands_value.clone()))
            .map_err(|err| ApiError::InvalidRequest(format!("invalid pricing_bands: {err}")))?;

    for (idx, band) in bands.iter().enumerate() {
        if band.capability.trim().is_empty() {
            return Err(ApiError::InvalidRequest(format!(
                "pricing_bands[{idx}].capability is required"
            )));
        }
        if !capabilities.iter().any(|cap| cap == band.capability.trim()) {
            return Err(ApiError::InvalidRequest(format!(
                "pricing_bands[{idx}].capability {} is not in capabilities[]",
                band.capability
            )));
        }
        if band.min_price_msats == 0 {
            return Err(ApiError::InvalidRequest(format!(
                "pricing_bands[{idx}].min_price_msats must be > 0"
            )));
        }
        if band.max_price_msats < band.min_price_msats {
            return Err(ApiError::InvalidRequest(format!(
                "pricing_bands[{idx}].max_price_msats must be >= min_price_msats"
            )));
        }
        if let Some(step) = band.step_msats {
            if step == 0 {
                return Err(ApiError::InvalidRequest(format!(
                    "pricing_bands[{idx}].step_msats must be > 0"
                )));
            }
        }
    }

    Ok(())
}

pub(super) async fn qualify_provider_metadata(
    metadata: &serde_json::Value,
) -> Result<(), ApiError> {
    let provider_base_url = metadata_string(metadata, "provider_base_url")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::InvalidRequest("provider_base_url is required for provider role".to_string())
        })?;

    let capabilities = metadata_string_array(metadata, "capabilities");
    if capabilities.is_empty() {
        return Err(ApiError::InvalidRequest(
            "capabilities[] is required for provider role".to_string(),
        ));
    }
    if !capabilities
        .iter()
        .any(|capability| capability == PHASE0_REQUIRED_PROVIDER_CAPABILITY)
    {
        return Err(ApiError::InvalidRequest(format!(
            "provider must advertise capability {PHASE0_REQUIRED_PROVIDER_CAPABILITY} for Phase 0"
        )));
    }

    qualify_provider_pricing(metadata)?;
    probe_provider_health(provider_base_url.as_str()).await?;
    Ok(())
}

pub(super) async fn probe_provider_health(base_url: &str) -> Result<(), ApiError> {
    let trimmed = base_url.trim_end_matches('/');
    let url = format!("{trimmed}/healthz");
    let resp = reqwest::Client::new()
        .get(url.as_str())
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| {
            ApiError::InvalidRequest(format!("provider health check failed ({url}): {error}"))
        })?;
    if !resp.status().is_success() {
        return Err(ApiError::InvalidRequest(format!(
            "provider health check returned {} ({url})",
            resp.status()
        )));
    }
    Ok(())
}

pub(super) fn owner_rate_key(owner: &WorkerOwner) -> String {
    if let Some(user_id) = owner.user_id {
        return format!("user:{user_id}");
    }
    owner
        .guest_scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("guest:{value}"))
        .unwrap_or_else(|| "guest:unknown".to_string())
}

pub(super) fn owners_match(left: &WorkerOwner, right: &WorkerOwner) -> bool {
    match (left.user_id, right.user_id) {
        (Some(left_id), Some(right_id)) => left_id == right_id,
        (None, None) => {
            left.guest_scope.as_deref().map(str::trim)
                == right.guest_scope.as_deref().map(str::trim)
        }
        _ => false,
    }
}

pub(super) fn provider_is_eligible_for_capability(
    provider: &ProviderCatalogEntry,
    capability: &str,
) -> bool {
    if provider
        .base_url
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        return false;
    }
    if provider.quarantined {
        return false;
    }
    if provider.status != WorkerStatus::Running {
        return false;
    }
    if provider.heartbeat_state != "fresh" {
        return false;
    }
    provider.capabilities.iter().any(|cap| cap == capability)
}
