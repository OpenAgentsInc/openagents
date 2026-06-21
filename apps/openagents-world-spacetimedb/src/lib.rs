use spacetimedb::{Identity, ReducerContext, Table, Timestamp};

const DEFAULT_TASSADAR_RUN_REF: &str = "run.tassadar.executor.20260615";
const DEFAULT_TASSADAR_REGION_REF: &str = "region.run.tassadar.executor.20260615.main";
const DEFAULT_TASSADAR_REGION_PREV_REF: &str = "region.run.tassadar.executor.20260615.street.prev";
const DEFAULT_TASSADAR_REGION_NEXT_REF: &str = "region.run.tassadar.executor.20260615.street.next";
const REGION_MIN_X: f64 = -160.0;
const REGION_MAX_X: f64 = 160.0;
const REGION_MIN_Y: f64 = 0.0;
const REGION_MAX_Y: f64 = 40.0;
const REGION_MIN_Z: f64 = -160.0;
const REGION_MAX_Z: f64 = 160.0;
const REGION_ROAD_DIRECTION_X: f64 = 0.0;
const REGION_ROAD_DIRECTION_Y: f64 = 0.0;
const REGION_ROAD_DIRECTION_Z: f64 = 1.0;
const REGION_LOCAL_ORIGIN_X: f64 = 0.0;
const REGION_LOCAL_ORIGIN_Y: f64 = 0.0;
const REGION_LOCAL_ORIGIN_Z: f64 = 0.0;
const REGION_STARTER_PYLON_SITE_OFFSET_X: f64 = 24.0;
const REGION_STARTER_PYLON_SITE_OFFSET_Y: f64 = 0.0;
const REGION_STARTER_PYLON_SITE_OFFSET_Z: f64 = 0.0;
const DEFAULT_REGION_PROXIMITY_RADIUS_METERS: f64 = 12.0;
const MAX_REGION_PROXIMITY_RADIUS_METERS: f64 = 120.0;
const MAX_AVATAR_MOVE_METERS_PER_SECOND: f64 = 14.0;
const AVATAR_POSITION_MIN_INTERVAL_MS: i64 = 100;
const STALE_AVATAR_POSITION_MS: i64 = 20_000;
const ATTENTION_TTL_MS: i64 = 8_000;
const CHAT_TTL_MS: i64 = 90_000;
const CHAT_BUBBLE_TTL_MS: i64 = 8_000;
const CHAT_MESSAGE_MIN_INTERVAL_MS: i64 = 1_000;
const EMOTE_TTL_MS: i64 = 8_000;
const INTENT_TTL_MS: i64 = 15_000;

#[spacetimedb::table(accessor = module_owner)]
pub struct ModuleOwner {
    #[primary_key]
    owner_identity: Identity,
    recorded_at: Timestamp,
}

#[spacetimedb::table(accessor = service_identity)]
pub struct ServiceIdentity {
    #[primary_key]
    identity: Identity,
    label: String,
    added_at: Timestamp,
}

#[spacetimedb::table(accessor = training_run, public)]
pub struct TrainingRun {
    #[primary_key]
    run_ref: String,
    run_state: String,
    source_url: String,
    source_generated_at: String,
    staleness_kind: String,
    max_staleness_seconds: u32,
    public_summary_hash: String,
    projected_at: Timestamp,
}

#[spacetimedb::table(accessor = run_entity, public)]
pub struct RunEntity {
    #[primary_key]
    entity_ref: String,
    run_ref: String,
    entity_kind: String,
    label: String,
    lane: String,
    status: String,
    source_ref: String,
    proof_count: u32,
    updated_at: Timestamp,
}

#[spacetimedb::table(accessor = world_edge, public)]
pub struct WorldEdge {
    #[primary_key]
    edge_ref: String,
    run_ref: String,
    from_entity_ref: String,
    to_entity_ref: String,
    edge_kind: String,
    source_ref: String,
    updated_at: Timestamp,
}

#[spacetimedb::table(accessor = proof_ref, public)]
pub struct ProofRef {
    #[primary_key]
    proof_ref: String,
    run_ref: String,
    entity_ref: String,
    proof_kind: String,
    url: String,
    title: String,
    updated_at: Timestamp,
}

#[spacetimedb::table(accessor = settlement_ref, public)]
pub struct SettlementRef {
    #[primary_key]
    settlement_ref: String,
    run_ref: String,
    entity_ref: String,
    receipt_ref: String,
    movement_mode: String,
    real_bitcoin_moved: bool,
    amount_sats: u64,
    url: String,
    updated_at: Timestamp,
}

#[spacetimedb::table(
    accessor = world_event,
    public,
    index(accessor = world_event_run, btree(columns = [run_ref]))
)]
pub struct WorldEvent {
    #[primary_key]
    event_ref: String,
    run_ref: String,
    event_kind: String,
    entity_ref: String,
    source_ref: String,
    source_generated_at: String,
    observed_at: Timestamp,
    summary: String,
}

#[spacetimedb::table(accessor = projection_cursor, public)]
pub struct ProjectionCursor {
    #[primary_key]
    cursor_ref: String,
    source_url: String,
    source_generated_at: String,
    source_hash: String,
    projected_at: Timestamp,
    row_count: u32,
}

#[spacetimedb::table(accessor = bridge_health, public)]
pub struct BridgeHealth {
    #[primary_key]
    bridge_ref: String,
    source_url: String,
    last_success_at: Option<Timestamp>,
    last_failure_at: Option<Timestamp>,
    last_failure_summary: Option<String>,
    heartbeat_at: Timestamp,
}

#[spacetimedb::table(accessor = world_region, public)]
pub struct WorldRegion {
    #[primary_key]
    region_ref: String,
    run_ref: String,
    label: String,
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    road_direction_x: f64,
    road_direction_y: f64,
    road_direction_z: f64,
    local_origin_x: f64,
    local_origin_y: f64,
    local_origin_z: f64,
    starter_pylon_site_offset_x: f64,
    starter_pylon_site_offset_y: f64,
    starter_pylon_site_offset_z: f64,
    street_prev_region_ref: Option<String>,
    street_next_region_ref: Option<String>,
    proximity_radius_meters: f64,
    avatar_position_min_interval_ms: i64,
    stale_avatar_position_ms: i64,
    updated_at: Timestamp,
}

#[spacetimedb::table(
    accessor = pylon_station,
    public,
    index(accessor = pylon_station_region, btree(columns = [region_ref]))
)]
pub struct PylonStation {
    #[primary_key]
    pylon_ref: String,
    run_ref: String,
    region_ref: String,
    label: String,
    source_url: String,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    heading_yaw: f64,
    interaction_radius_meters: f64,
    updated_at: Timestamp,
}

#[spacetimedb::table(accessor = agent_avatar, public)]
pub struct AgentAvatar {
    #[primary_key]
    avatar_ref: String,
    owner_identity: Identity,
    actor_ref: String,
    actor_kind: String,
    display_name: String,
    home_pylon_ref: Option<String>,
    public_profile_url: Option<String>,
    created_at: Timestamp,
    last_seen_at: Timestamp,
}

#[spacetimedb::table(
    accessor = avatar_position,
    public,
    index(accessor = avatar_position_region, btree(columns = [region_ref]))
)]
pub struct AvatarPosition {
    #[primary_key]
    avatar_ref: String,
    region_ref: String,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    yaw: f64,
    pitch: f64,
    movement_mode: String,
    last_seen_epoch_ms: i64,
    updated_at: Timestamp,
}

#[spacetimedb::table(
    accessor = pylon_attention,
    public,
    index(accessor = pylon_attention_pylon, btree(columns = [pylon_ref]))
)]
pub struct PylonAttention {
    #[primary_key]
    attention_ref: String,
    pylon_ref: String,
    avatar_ref: String,
    attention_kind: String,
    distance_meters: f64,
    source_entity_ref: Option<String>,
    first_seen_at: Timestamp,
    last_seen_at: Timestamp,
    expires_at_epoch_ms: i64,
}

#[spacetimedb::table(
    accessor = local_chat_message,
    public,
    index(accessor = local_chat_message_region, btree(columns = [region_ref]))
)]
pub struct LocalChatMessage {
    #[primary_key]
    message_ref: String,
    region_ref: String,
    speaker_avatar_ref: String,
    target_ref: Option<String>,
    channel_kind: String,
    radius_meters: f64,
    body: String,
    body_format: String,
    created_at: Timestamp,
    expires_at_epoch_ms: i64,
    moderation_state: String,
}

#[spacetimedb::table(
    accessor = chat_bubble,
    public,
    index(accessor = chat_bubble_message, btree(columns = [message_ref]))
)]
pub struct ChatBubble {
    #[primary_key]
    bubble_ref: String,
    message_ref: String,
    speaker_avatar_ref: String,
    anchor_entity_ref: String,
    created_at: Timestamp,
    expires_at_epoch_ms: i64,
}

#[spacetimedb::table(
    accessor = local_emote,
    public,
    index(accessor = local_emote_region, btree(columns = [region_ref]))
)]
pub struct LocalEmote {
    #[primary_key]
    emote_ref: String,
    avatar_ref: String,
    region_ref: String,
    emote_kind: String,
    target_ref: Option<String>,
    created_at: Timestamp,
    expires_at_epoch_ms: i64,
}

#[spacetimedb::table(accessor = agent_intent, public)]
pub struct AgentIntent {
    #[primary_key]
    avatar_ref: String,
    intent_kind: String,
    target_ref: Option<String>,
    updated_at: Timestamp,
    expires_at_epoch_ms: i64,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) -> Result<(), String> {
    let owner_identity = ctx.sender();
    if ctx
        .db
        .module_owner()
        .owner_identity()
        .find(owner_identity)
        .is_none()
    {
        ctx.db.module_owner().insert(ModuleOwner {
            owner_identity,
            recorded_at: ctx.timestamp,
        });
    }
    if ctx
        .db
        .service_identity()
        .identity()
        .find(owner_identity)
        .is_none()
    {
        ctx.db.service_identity().insert(ServiceIdentity {
            identity: owner_identity,
            label: "module-owner".to_string(),
            added_at: ctx.timestamp,
        });
    }
    let database_identity = ctx.database_identity();
    if ctx
        .db
        .service_identity()
        .identity()
        .find(database_identity)
        .is_none()
    {
        ctx.db.service_identity().insert(ServiceIdentity {
            identity: database_identity,
            label: "database-scheduler".to_string(),
            added_at: ctx.timestamp,
        });
    }
    upsert_world_region_row(ctx, default_tassadar_world_region(ctx.timestamp));
    Ok(())
}

#[spacetimedb::reducer]
pub fn authorize_service_identity(
    ctx: &ReducerContext,
    identity: Identity,
    label: String,
) -> Result<(), String> {
    ensure_owner(ctx)?;
    upsert_service_identity(
        ctx,
        ServiceIdentity {
            identity,
            label,
            added_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_service_identity(ctx: &ReducerContext, identity: Identity) -> Result<(), String> {
    ensure_owner(ctx)?;
    if is_owner(ctx, identity) {
        return Err("cannot revoke module owner service identity".to_string());
    }
    ctx.db.service_identity().identity().delete(identity);
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_training_run(
    ctx: &ReducerContext,
    run_ref: String,
    run_state: String,
    source_url: String,
    source_generated_at: String,
    staleness_kind: String,
    max_staleness_seconds: u32,
    public_summary_hash: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_training_run_row(
        ctx,
        TrainingRun {
            run_ref,
            run_state,
            source_url,
            source_generated_at,
            staleness_kind,
            max_staleness_seconds,
            public_summary_hash,
            projected_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_run_entity(
    ctx: &ReducerContext,
    entity_ref: String,
    run_ref: String,
    entity_kind: String,
    label: String,
    lane: String,
    status: String,
    source_ref: String,
    proof_count: u32,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_run_entity_row(
        ctx,
        RunEntity {
            entity_ref,
            run_ref,
            entity_kind,
            label,
            lane,
            status,
            source_ref,
            proof_count,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_world_edge(
    ctx: &ReducerContext,
    edge_ref: String,
    run_ref: String,
    from_entity_ref: String,
    to_entity_ref: String,
    edge_kind: String,
    source_ref: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_world_edge_row(
        ctx,
        WorldEdge {
            edge_ref,
            run_ref,
            from_entity_ref,
            to_entity_ref,
            edge_kind,
            source_ref,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_proof_ref(
    ctx: &ReducerContext,
    proof_ref: String,
    run_ref: String,
    entity_ref: String,
    proof_kind: String,
    url: String,
    title: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_proof_ref_row(
        ctx,
        ProofRef {
            proof_ref,
            run_ref,
            entity_ref,
            proof_kind,
            url,
            title,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_settlement_ref(
    ctx: &ReducerContext,
    settlement_ref: String,
    run_ref: String,
    entity_ref: String,
    receipt_ref: String,
    movement_mode: String,
    real_bitcoin_moved: bool,
    amount_sats: u64,
    url: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_settlement_ref_row(
        ctx,
        SettlementRef {
            settlement_ref,
            run_ref,
            entity_ref,
            receipt_ref,
            movement_mode,
            real_bitcoin_moved,
            amount_sats,
            url,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn append_world_event(
    ctx: &ReducerContext,
    event_ref: String,
    run_ref: String,
    event_kind: String,
    entity_ref: String,
    source_ref: String,
    source_generated_at: String,
    summary: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    if source_ref.is_empty() && source_generated_at.is_empty() {
        return Err("world_event requires a source_ref or source_generated_at".to_string());
    }
    if ctx
        .db
        .world_event()
        .event_ref()
        .find(event_ref.clone())
        .is_some()
    {
        return Ok(());
    }
    ctx.db.world_event().insert(WorldEvent {
        event_ref,
        run_ref,
        event_kind,
        entity_ref,
        source_ref,
        source_generated_at,
        observed_at: ctx.timestamp,
        summary,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn record_projection_cursor(
    ctx: &ReducerContext,
    cursor_ref: String,
    source_url: String,
    source_generated_at: String,
    source_hash: String,
    row_count: u32,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_projection_cursor_row(
        ctx,
        ProjectionCursor {
            cursor_ref,
            source_url,
            source_generated_at,
            source_hash,
            projected_at: ctx.timestamp,
            row_count,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn record_bridge_health(
    ctx: &ReducerContext,
    bridge_ref: String,
    source_url: String,
    last_success_at: Option<Timestamp>,
    last_failure_at: Option<Timestamp>,
    last_failure_summary: Option<String>,
) -> Result<(), String> {
    ensure_service(ctx)?;
    upsert_bridge_health_row(
        ctx,
        BridgeHealth {
            bridge_ref,
            source_url,
            last_success_at,
            last_failure_at,
            last_failure_summary,
            heartbeat_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn record_bridge_success(
    ctx: &ReducerContext,
    bridge_ref: String,
    source_url: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    let existing = ctx.db.bridge_health().bridge_ref().find(bridge_ref.clone());
    upsert_bridge_health_row(
        ctx,
        BridgeHealth {
            bridge_ref,
            source_url,
            last_success_at: Some(ctx.timestamp),
            last_failure_at: existing.as_ref().and_then(|row| row.last_failure_at),
            last_failure_summary: existing.and_then(|row| row.last_failure_summary),
            heartbeat_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn record_bridge_failure(
    ctx: &ReducerContext,
    bridge_ref: String,
    source_url: String,
    failure_summary: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    let existing = ctx.db.bridge_health().bridge_ref().find(bridge_ref.clone());
    upsert_bridge_health_row(
        ctx,
        BridgeHealth {
            bridge_ref,
            source_url,
            last_success_at: existing.as_ref().and_then(|row| row.last_success_at),
            last_failure_at: Some(ctx.timestamp),
            last_failure_summary: Some(failure_summary),
            heartbeat_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_world_region(
    ctx: &ReducerContext,
    region_ref: String,
    run_ref: String,
    label: String,
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    road_direction_x: f64,
    road_direction_y: f64,
    road_direction_z: f64,
    local_origin_x: f64,
    local_origin_y: f64,
    local_origin_z: f64,
    starter_pylon_site_offset_x: f64,
    starter_pylon_site_offset_y: f64,
    starter_pylon_site_offset_z: f64,
    street_prev_region_ref: String,
    street_next_region_ref: String,
    proximity_radius_meters: f64,
    avatar_position_min_interval_ms: i64,
    stale_avatar_position_ms: i64,
) -> Result<(), String> {
    ensure_service(ctx)?;
    validate_region_bounds(min_x, min_y, min_z, max_x, max_y, max_z)?;
    validate_region_metadata(
        min_x,
        min_y,
        min_z,
        max_x,
        max_y,
        max_z,
        road_direction_x,
        road_direction_y,
        road_direction_z,
        local_origin_x,
        local_origin_y,
        local_origin_z,
        starter_pylon_site_offset_x,
        starter_pylon_site_offset_y,
        starter_pylon_site_offset_z,
    )?;
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    let street_prev_region_ref =
        clean_optional_ref_string(street_prev_region_ref, "street_prev_region_ref", 160)?;
    let street_next_region_ref =
        clean_optional_ref_string(street_next_region_ref, "street_next_region_ref", 160)?;
    validate_adjacent_region_ref(
        &region_ref,
        street_prev_region_ref.as_deref(),
        "street_prev_region_ref",
    )?;
    validate_adjacent_region_ref(
        &region_ref,
        street_next_region_ref.as_deref(),
        "street_next_region_ref",
    )?;
    let proximity_radius_meters =
        validate_radius(proximity_radius_meters, MAX_REGION_PROXIMITY_RADIUS_METERS)?;
    upsert_world_region_row(
        ctx,
        WorldRegion {
            region_ref,
            run_ref: clean_ref(run_ref, "run_ref", 160)?,
            label: clean_text(label, "label", 80)?,
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
            road_direction_x,
            road_direction_y,
            road_direction_z,
            local_origin_x,
            local_origin_y,
            local_origin_z,
            starter_pylon_site_offset_x,
            starter_pylon_site_offset_y,
            starter_pylon_site_offset_z,
            street_prev_region_ref,
            street_next_region_ref,
            proximity_radius_meters,
            avatar_position_min_interval_ms: validate_ms_window(
                avatar_position_min_interval_ms,
                "avatar_position_min_interval_ms",
                50,
                1_000,
            )?,
            stale_avatar_position_ms: validate_ms_window(
                stale_avatar_position_ms,
                "stale_avatar_position_ms",
                1_000,
                120_000,
            )?,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn upsert_pylon_station_from_projection(
    ctx: &ReducerContext,
    pylon_ref: String,
    run_ref: String,
    region_ref: String,
    label: String,
    source_url: String,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    heading_yaw: f64,
    interaction_radius_meters: f64,
) -> Result<(), String> {
    ensure_service(ctx)?;
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    validate_position_in_region(ctx, &region_ref, position_x, position_y, position_z)?;
    if !heading_yaw.is_finite() {
        return Err("heading_yaw must be finite".to_string());
    }
    let interaction_radius_meters = validate_radius(interaction_radius_meters, 40.0)?;
    upsert_pylon_station_row(
        ctx,
        PylonStation {
            pylon_ref: clean_ref(pylon_ref, "pylon_ref", 160)?,
            run_ref: clean_ref(run_ref, "run_ref", 160)?,
            region_ref,
            label: clean_text(label, "label", 80)?,
            source_url: clean_optional_text(source_url, 512).unwrap_or_default(),
            position_x,
            position_y,
            position_z,
            heading_yaw,
            interaction_radius_meters,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn ensure_pylon_agent_avatar(
    ctx: &ReducerContext,
    avatar_ref: String,
    pylon_ref: String,
    display_name: String,
    region_ref: String,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    yaw: f64,
) -> Result<(), String> {
    ensure_service(ctx)?;
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    validate_position_in_region(ctx, &region_ref, position_x, position_y, position_z)?;
    if !yaw.is_finite() {
        return Err("yaw must be finite".to_string());
    }
    let pylon_ref = clean_ref(pylon_ref, "pylon_ref", 160)?;
    if ctx
        .db
        .pylon_station()
        .pylon_ref()
        .find(pylon_ref.clone())
        .is_none()
    {
        return Err("pylon agent avatar requires an existing pylon_station".to_string());
    }
    let avatar_ref = clean_ref(avatar_ref, "avatar_ref", 160)?;
    let existing = ctx.db.agent_avatar().avatar_ref().find(avatar_ref.clone());
    upsert_agent_avatar_row(
        ctx,
        AgentAvatar {
            avatar_ref: avatar_ref.clone(),
            owner_identity: ctx.sender(),
            actor_ref: format!("pylon_agent.{pylon_ref}"),
            actor_kind: "pylon_agent".to_string(),
            display_name: clean_text(display_name, "display_name", 64)?,
            home_pylon_ref: Some(pylon_ref),
            public_profile_url: None,
            created_at: existing
                .as_ref()
                .map(|row| row.created_at)
                .unwrap_or(ctx.timestamp),
            last_seen_at: ctx.timestamp,
        },
    );
    upsert_avatar_position_row(
        ctx,
        AvatarPosition {
            avatar_ref,
            region_ref,
            position_x,
            position_y,
            position_z,
            yaw,
            pitch: 0.0,
            movement_mode: "idle".to_string(),
            last_seen_epoch_ms: ctx_epoch_ms(ctx),
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn record_system_world_message(
    ctx: &ReducerContext,
    region_ref: String,
    target_ref: Option<String>,
    radius_meters: f64,
    body: String,
) -> Result<(), String> {
    ensure_service(ctx)?;
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    ensure_world_region(ctx, &region_ref)?;
    let system_avatar_ref = ensure_system_avatar(ctx);
    insert_local_message(
        ctx,
        region_ref,
        system_avatar_ref,
        clean_optional_ref(target_ref, "target_ref", 160)?,
        "system".to_string(),
        validate_radius(radius_meters, 80.0)?,
        body,
    )?;
    Ok(())
}

#[spacetimedb::reducer]
pub fn join_region(
    ctx: &ReducerContext,
    region_ref: String,
    display_name: String,
) -> Result<(), String> {
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    validate_position_in_region(ctx, &region_ref, 0.0, 0.0, 0.0)?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    let display_name = clean_optional_text(display_name, 64)
        .unwrap_or_else(|| format!("agent {}", ctx.sender().to_abbreviated_hex()));
    let existing = ctx.db.agent_avatar().avatar_ref().find(avatar_ref.clone());
    upsert_agent_avatar_row(
        ctx,
        AgentAvatar {
            avatar_ref: avatar_ref.clone(),
            owner_identity: ctx.sender(),
            actor_ref: format!("identity.{}", ctx.sender()),
            actor_kind: existing
                .as_ref()
                .map(|row| row.actor_kind.clone())
                .unwrap_or_else(|| "guest".to_string()),
            display_name,
            home_pylon_ref: existing.as_ref().and_then(|row| row.home_pylon_ref.clone()),
            public_profile_url: existing.and_then(|row| row.public_profile_url),
            created_at: ctx
                .db
                .agent_avatar()
                .avatar_ref()
                .find(avatar_ref.clone())
                .map(|row| row.created_at)
                .unwrap_or(ctx.timestamp),
            last_seen_at: ctx.timestamp,
        },
    );
    upsert_avatar_position_row(
        ctx,
        AvatarPosition {
            avatar_ref,
            region_ref,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            movement_mode: "idle".to_string(),
            last_seen_epoch_ms: ctx_epoch_ms(ctx),
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn leave_region(ctx: &ReducerContext, region_ref: String) -> Result<(), String> {
    let avatar_ref = avatar_ref_for_sender(ctx);
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    if let Some(position) = ctx
        .db
        .avatar_position()
        .avatar_ref()
        .find(avatar_ref.clone())
    {
        if position.region_ref == region_ref {
            ctx.db
                .avatar_position()
                .avatar_ref()
                .delete(avatar_ref.clone());
        }
    }
    ctx.db
        .agent_intent()
        .avatar_ref()
        .delete(avatar_ref.clone());
    let attention_refs: Vec<String> = ctx
        .db
        .pylon_attention()
        .iter()
        .filter(|row| row.avatar_ref == avatar_ref)
        .map(|row| row.attention_ref)
        .collect();
    for attention_ref in attention_refs {
        ctx.db
            .pylon_attention()
            .attention_ref()
            .delete(attention_ref);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_avatar_position(
    ctx: &ReducerContext,
    region_ref: String,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    yaw: f64,
    pitch: f64,
    movement_mode: String,
) -> Result<(), String> {
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    let region = validate_position_in_region(ctx, &region_ref, position_x, position_y, position_z)?;
    if !yaw.is_finite() || !pitch.is_finite() {
        return Err("yaw and pitch must be finite".to_string());
    }
    let movement_mode = validate_choice(
        movement_mode,
        "movement_mode",
        &["idle", "walking", "running", "ghost", "inspecting"],
    )?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    let now_ms = ctx_epoch_ms(ctx);
    if let Some(existing) = ctx
        .db
        .avatar_position()
        .avatar_ref()
        .find(avatar_ref.clone())
    {
        if now_ms < existing.last_seen_epoch_ms + region.avatar_position_min_interval_ms {
            return Ok(());
        }
        if existing.region_ref != region_ref {
            return Err(
                "set_avatar_position cannot move between regions; call join_region first"
                    .to_string(),
            );
        }
        validate_movement_delta(&existing, position_x, position_y, position_z, now_ms)?;
    }
    upsert_avatar_position_row(
        ctx,
        AvatarPosition {
            avatar_ref,
            region_ref,
            position_x,
            position_y,
            position_z,
            yaw,
            pitch,
            movement_mode,
            last_seen_epoch_ms: now_ms,
            updated_at: ctx.timestamp,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn focus_pylon(
    ctx: &ReducerContext,
    pylon_ref: String,
    attention_kind: String,
    distance_meters: f64,
    source_entity_ref: Option<String>,
) -> Result<(), String> {
    let pylon_ref = clean_ref(pylon_ref, "pylon_ref", 160)?;
    if ctx
        .db
        .pylon_station()
        .pylon_ref()
        .find(pylon_ref.clone())
        .is_none()
    {
        return Err("pylon attention requires an existing pylon_station".to_string());
    }
    let attention_kind = validate_choice(
        attention_kind,
        "attention_kind",
        &["approaching", "nearby", "looking", "inspecting", "talking"],
    )?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    let attention_ref = format!("attention.{pylon_ref}.{avatar_ref}");
    let existing = ctx
        .db
        .pylon_attention()
        .attention_ref()
        .find(attention_ref.clone());
    upsert_pylon_attention_row(
        ctx,
        PylonAttention {
            attention_ref,
            pylon_ref,
            avatar_ref,
            attention_kind,
            distance_meters: validate_distance(distance_meters)?,
            source_entity_ref: clean_optional_ref(source_entity_ref, "source_entity_ref", 160)?,
            first_seen_at: existing
                .as_ref()
                .map(|row| row.first_seen_at)
                .unwrap_or(ctx.timestamp),
            last_seen_at: ctx.timestamp,
            expires_at_epoch_ms: ctx_epoch_ms(ctx) + ATTENTION_TTL_MS,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn clear_pylon_focus(ctx: &ReducerContext, pylon_ref: String) -> Result<(), String> {
    let pylon_ref = clean_ref(pylon_ref, "pylon_ref", 160)?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ctx.db
        .pylon_attention()
        .attention_ref()
        .delete(format!("attention.{pylon_ref}.{avatar_ref}"));
    Ok(())
}

#[spacetimedb::reducer]
pub fn send_local_message(
    ctx: &ReducerContext,
    region_ref: String,
    target_ref: Option<String>,
    radius_meters: f64,
    body: String,
) -> Result<(), String> {
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    ensure_world_region(ctx, &region_ref)?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    insert_local_message(
        ctx,
        region_ref,
        avatar_ref,
        clean_optional_ref(target_ref, "target_ref", 160)?,
        "local".to_string(),
        validate_radius(radius_meters, 40.0)?,
        body,
    )?;
    Ok(())
}

#[spacetimedb::reducer]
pub fn send_pylon_message(
    ctx: &ReducerContext,
    pylon_ref: String,
    body: String,
) -> Result<(), String> {
    let pylon_ref = clean_ref(pylon_ref, "pylon_ref", 160)?;
    let station = ctx
        .db
        .pylon_station()
        .pylon_ref()
        .find(pylon_ref.clone())
        .ok_or_else(|| "pylon message requires an existing pylon_station".to_string())?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    insert_local_message(
        ctx,
        station.region_ref,
        avatar_ref,
        Some(pylon_ref),
        "pylon".to_string(),
        validate_radius(station.interaction_radius_meters.max(8.0), 40.0)?,
        body,
    )?;
    Ok(())
}

#[spacetimedb::reducer]
pub fn send_emote(
    ctx: &ReducerContext,
    region_ref: String,
    emote_kind: String,
    target_ref: Option<String>,
) -> Result<(), String> {
    let region_ref = clean_ref(region_ref, "region_ref", 160)?;
    ensure_world_region(ctx, &region_ref)?;
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    let emote_kind = validate_choice(
        emote_kind,
        "emote_kind",
        &["wave", "ping", "point", "confused", "working"],
    )?;
    let emote_ref = next_ref(ctx, "emote");
    ctx.db.local_emote().insert(LocalEmote {
        emote_ref,
        avatar_ref,
        region_ref,
        emote_kind,
        target_ref: clean_optional_ref(target_ref, "target_ref", 160)?,
        created_at: ctx.timestamp,
        expires_at_epoch_ms: ctx_epoch_ms(ctx) + EMOTE_TTL_MS,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_agent_intent(
    ctx: &ReducerContext,
    intent_kind: String,
    target_ref: Option<String>,
) -> Result<(), String> {
    let avatar_ref = avatar_ref_for_sender(ctx);
    ensure_avatar_for_sender(ctx, avatar_ref.clone(), None)?;
    let intent_kind = validate_choice(
        intent_kind,
        "intent_kind",
        &[
            "idle",
            "patrol",
            "inspect_pylon",
            "inspect_proof",
            "talk",
            "return_home",
        ],
    )?;
    upsert_agent_intent_row(
        ctx,
        AgentIntent {
            avatar_ref,
            intent_kind,
            target_ref: clean_optional_ref(target_ref, "target_ref", 160)?,
            updated_at: ctx.timestamp,
            expires_at_epoch_ms: ctx_epoch_ms(ctx) + INTENT_TTL_MS,
        },
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn expire_interaction_rows(ctx: &ReducerContext) -> Result<(), String> {
    ensure_service(ctx)?;
    let now_ms = ctx_epoch_ms(ctx);

    let position_refs: Vec<String> = ctx
        .db
        .avatar_position()
        .iter()
        .filter(|row| {
            let stale_avatar_position_ms = ctx
                .db
                .world_region()
                .region_ref()
                .find(row.region_ref.clone())
                .map(|region| region.stale_avatar_position_ms)
                .unwrap_or(STALE_AVATAR_POSITION_MS);
            row.last_seen_epoch_ms + stale_avatar_position_ms <= now_ms
                && ctx
                    .db
                    .agent_avatar()
                    .avatar_ref()
                    .find(row.avatar_ref.clone())
                    .map(|avatar| {
                        avatar.actor_kind != "pylon_agent" && avatar.actor_kind != "service_agent"
                    })
                    .unwrap_or(true)
        })
        .map(|row| row.avatar_ref)
        .collect();
    for avatar_ref in position_refs {
        ctx.db.avatar_position().avatar_ref().delete(avatar_ref);
    }

    let attention_refs: Vec<String> = ctx
        .db
        .pylon_attention()
        .iter()
        .filter(|row| row.expires_at_epoch_ms <= now_ms)
        .map(|row| row.attention_ref)
        .collect();
    for attention_ref in attention_refs {
        ctx.db
            .pylon_attention()
            .attention_ref()
            .delete(attention_ref);
    }

    let message_refs: Vec<String> = ctx
        .db
        .local_chat_message()
        .iter()
        .filter(|row| row.expires_at_epoch_ms <= now_ms)
        .map(|row| row.message_ref)
        .collect();
    for message_ref in message_refs {
        ctx.db
            .local_chat_message()
            .message_ref()
            .delete(message_ref);
    }

    let bubble_refs: Vec<String> = ctx
        .db
        .chat_bubble()
        .iter()
        .filter(|row| row.expires_at_epoch_ms <= now_ms)
        .map(|row| row.bubble_ref)
        .collect();
    for bubble_ref in bubble_refs {
        ctx.db.chat_bubble().bubble_ref().delete(bubble_ref);
    }

    let emote_refs: Vec<String> = ctx
        .db
        .local_emote()
        .iter()
        .filter(|row| row.expires_at_epoch_ms <= now_ms)
        .map(|row| row.emote_ref)
        .collect();
    for emote_ref in emote_refs {
        ctx.db.local_emote().emote_ref().delete(emote_ref);
    }

    let intent_refs: Vec<String> = ctx
        .db
        .agent_intent()
        .iter()
        .filter(|row| row.expires_at_epoch_ms <= now_ms)
        .map(|row| row.avatar_ref)
        .collect();
    for avatar_ref in intent_refs {
        ctx.db.agent_intent().avatar_ref().delete(avatar_ref);
    }

    Ok(())
}

fn ensure_owner(ctx: &ReducerContext) -> Result<(), String> {
    if is_owner(ctx, ctx.sender()) {
        Ok(())
    } else {
        Err("unauthorized: service identity changes require module owner".to_string())
    }
}

fn ensure_service(ctx: &ReducerContext) -> Result<(), String> {
    if ctx
        .db
        .service_identity()
        .identity()
        .find(ctx.sender())
        .is_some()
    {
        Ok(())
    } else {
        Err("unauthorized: reducer requires service identity".to_string())
    }
}

fn is_owner(ctx: &ReducerContext, identity: Identity) -> bool {
    ctx.db
        .module_owner()
        .owner_identity()
        .find(identity)
        .is_some()
}

fn upsert_service_identity(ctx: &ReducerContext, row: ServiceIdentity) {
    if ctx
        .db
        .service_identity()
        .identity()
        .find(row.identity)
        .is_some()
    {
        ctx.db.service_identity().identity().update(row);
    } else {
        ctx.db.service_identity().insert(row);
    }
}

fn upsert_training_run_row(ctx: &ReducerContext, row: TrainingRun) {
    if ctx
        .db
        .training_run()
        .run_ref()
        .find(row.run_ref.clone())
        .is_some()
    {
        ctx.db.training_run().run_ref().update(row);
    } else {
        ctx.db.training_run().insert(row);
    }
}

fn upsert_run_entity_row(ctx: &ReducerContext, row: RunEntity) {
    if ctx
        .db
        .run_entity()
        .entity_ref()
        .find(row.entity_ref.clone())
        .is_some()
    {
        ctx.db.run_entity().entity_ref().update(row);
    } else {
        ctx.db.run_entity().insert(row);
    }
}

fn upsert_world_edge_row(ctx: &ReducerContext, row: WorldEdge) {
    if ctx
        .db
        .world_edge()
        .edge_ref()
        .find(row.edge_ref.clone())
        .is_some()
    {
        ctx.db.world_edge().edge_ref().update(row);
    } else {
        ctx.db.world_edge().insert(row);
    }
}

fn upsert_proof_ref_row(ctx: &ReducerContext, row: ProofRef) {
    if ctx
        .db
        .proof_ref()
        .proof_ref()
        .find(row.proof_ref.clone())
        .is_some()
    {
        ctx.db.proof_ref().proof_ref().update(row);
    } else {
        ctx.db.proof_ref().insert(row);
    }
}

fn upsert_settlement_ref_row(ctx: &ReducerContext, row: SettlementRef) {
    if ctx
        .db
        .settlement_ref()
        .settlement_ref()
        .find(row.settlement_ref.clone())
        .is_some()
    {
        ctx.db.settlement_ref().settlement_ref().update(row);
    } else {
        ctx.db.settlement_ref().insert(row);
    }
}

fn upsert_projection_cursor_row(ctx: &ReducerContext, row: ProjectionCursor) {
    if ctx
        .db
        .projection_cursor()
        .cursor_ref()
        .find(row.cursor_ref.clone())
        .is_some()
    {
        ctx.db.projection_cursor().cursor_ref().update(row);
    } else {
        ctx.db.projection_cursor().insert(row);
    }
}

fn upsert_bridge_health_row(ctx: &ReducerContext, row: BridgeHealth) {
    if ctx
        .db
        .bridge_health()
        .bridge_ref()
        .find(row.bridge_ref.clone())
        .is_some()
    {
        ctx.db.bridge_health().bridge_ref().update(row);
    } else {
        ctx.db.bridge_health().insert(row);
    }
}

fn default_tassadar_world_region(updated_at: Timestamp) -> WorldRegion {
    WorldRegion {
        region_ref: DEFAULT_TASSADAR_REGION_REF.to_string(),
        run_ref: DEFAULT_TASSADAR_RUN_REF.to_string(),
        label: "Tassadar main run space".to_string(),
        min_x: REGION_MIN_X,
        min_y: REGION_MIN_Y,
        min_z: REGION_MIN_Z,
        max_x: REGION_MAX_X,
        max_y: REGION_MAX_Y,
        max_z: REGION_MAX_Z,
        road_direction_x: REGION_ROAD_DIRECTION_X,
        road_direction_y: REGION_ROAD_DIRECTION_Y,
        road_direction_z: REGION_ROAD_DIRECTION_Z,
        local_origin_x: REGION_LOCAL_ORIGIN_X,
        local_origin_y: REGION_LOCAL_ORIGIN_Y,
        local_origin_z: REGION_LOCAL_ORIGIN_Z,
        starter_pylon_site_offset_x: REGION_STARTER_PYLON_SITE_OFFSET_X,
        starter_pylon_site_offset_y: REGION_STARTER_PYLON_SITE_OFFSET_Y,
        starter_pylon_site_offset_z: REGION_STARTER_PYLON_SITE_OFFSET_Z,
        street_prev_region_ref: Some(DEFAULT_TASSADAR_REGION_PREV_REF.to_string()),
        street_next_region_ref: Some(DEFAULT_TASSADAR_REGION_NEXT_REF.to_string()),
        proximity_radius_meters: DEFAULT_REGION_PROXIMITY_RADIUS_METERS,
        avatar_position_min_interval_ms: AVATAR_POSITION_MIN_INTERVAL_MS,
        stale_avatar_position_ms: STALE_AVATAR_POSITION_MS,
        updated_at,
    }
}

fn upsert_world_region_row(ctx: &ReducerContext, row: WorldRegion) {
    if ctx
        .db
        .world_region()
        .region_ref()
        .find(row.region_ref.clone())
        .is_some()
    {
        ctx.db.world_region().region_ref().update(row);
    } else {
        ctx.db.world_region().insert(row);
    }
}

fn upsert_pylon_station_row(ctx: &ReducerContext, row: PylonStation) {
    if ctx
        .db
        .pylon_station()
        .pylon_ref()
        .find(row.pylon_ref.clone())
        .is_some()
    {
        ctx.db.pylon_station().pylon_ref().update(row);
    } else {
        ctx.db.pylon_station().insert(row);
    }
}

fn upsert_agent_avatar_row(ctx: &ReducerContext, row: AgentAvatar) {
    if ctx
        .db
        .agent_avatar()
        .avatar_ref()
        .find(row.avatar_ref.clone())
        .is_some()
    {
        ctx.db.agent_avatar().avatar_ref().update(row);
    } else {
        ctx.db.agent_avatar().insert(row);
    }
}

fn upsert_avatar_position_row(ctx: &ReducerContext, row: AvatarPosition) {
    if ctx
        .db
        .avatar_position()
        .avatar_ref()
        .find(row.avatar_ref.clone())
        .is_some()
    {
        ctx.db.avatar_position().avatar_ref().update(row);
    } else {
        ctx.db.avatar_position().insert(row);
    }
}

fn upsert_pylon_attention_row(ctx: &ReducerContext, row: PylonAttention) {
    if ctx
        .db
        .pylon_attention()
        .attention_ref()
        .find(row.attention_ref.clone())
        .is_some()
    {
        ctx.db.pylon_attention().attention_ref().update(row);
    } else {
        ctx.db.pylon_attention().insert(row);
    }
}

fn upsert_agent_intent_row(ctx: &ReducerContext, row: AgentIntent) {
    if ctx
        .db
        .agent_intent()
        .avatar_ref()
        .find(row.avatar_ref.clone())
        .is_some()
    {
        ctx.db.agent_intent().avatar_ref().update(row);
    } else {
        ctx.db.agent_intent().insert(row);
    }
}

fn ensure_avatar_for_sender(
    ctx: &ReducerContext,
    avatar_ref: String,
    display_name: Option<String>,
) -> Result<(), String> {
    let existing = ctx.db.agent_avatar().avatar_ref().find(avatar_ref.clone());
    let display_name = display_name.unwrap_or_else(|| {
        existing
            .as_ref()
            .map(|row| row.display_name.clone())
            .unwrap_or_else(|| format!("agent {}", ctx.sender().to_abbreviated_hex()))
    });
    upsert_agent_avatar_row(
        ctx,
        AgentAvatar {
            avatar_ref,
            owner_identity: ctx.sender(),
            actor_ref: format!("identity.{}", ctx.sender()),
            actor_kind: existing
                .as_ref()
                .map(|row| row.actor_kind.clone())
                .unwrap_or_else(|| "guest".to_string()),
            display_name,
            home_pylon_ref: existing.as_ref().and_then(|row| row.home_pylon_ref.clone()),
            public_profile_url: existing
                .as_ref()
                .and_then(|row| row.public_profile_url.clone()),
            created_at: existing
                .as_ref()
                .map(|row| row.created_at)
                .unwrap_or(ctx.timestamp),
            last_seen_at: ctx.timestamp,
        },
    );
    Ok(())
}

fn ensure_system_avatar(ctx: &ReducerContext) -> String {
    let avatar_ref = "avatar.system.openagents-world".to_string();
    let existing = ctx.db.agent_avatar().avatar_ref().find(avatar_ref.clone());
    upsert_agent_avatar_row(
        ctx,
        AgentAvatar {
            avatar_ref: avatar_ref.clone(),
            owner_identity: ctx.database_identity(),
            actor_ref: "system.openagents-world".to_string(),
            actor_kind: "service_agent".to_string(),
            display_name: "OpenAgents world".to_string(),
            home_pylon_ref: None,
            public_profile_url: None,
            created_at: existing
                .as_ref()
                .map(|row| row.created_at)
                .unwrap_or(ctx.timestamp),
            last_seen_at: ctx.timestamp,
        },
    );
    avatar_ref
}

fn insert_local_message(
    ctx: &ReducerContext,
    region_ref: String,
    speaker_avatar_ref: String,
    target_ref: Option<String>,
    channel_kind: String,
    radius_meters: f64,
    body: String,
) -> Result<(), String> {
    let channel_kind =
        validate_choice(channel_kind, "channel_kind", &["local", "pylon", "system"])?;
    let now_ms = ctx_epoch_ms(ctx);
    if ctx.db.local_chat_message().iter().any(|row| {
        row.speaker_avatar_ref == speaker_avatar_ref
            && row.expires_at_epoch_ms > now_ms + CHAT_TTL_MS - CHAT_MESSAGE_MIN_INTERVAL_MS
    }) {
        return Err("chat messages are rate limited".to_string());
    }
    let message_ref = next_ref(ctx, "message");
    let bubble_ref = format!("bubble.{message_ref}");
    let anchor_entity_ref = target_ref
        .as_ref()
        .cloned()
        .unwrap_or_else(|| speaker_avatar_ref.clone());
    ctx.db.local_chat_message().insert(LocalChatMessage {
        message_ref: message_ref.clone(),
        region_ref,
        speaker_avatar_ref: speaker_avatar_ref.clone(),
        target_ref,
        channel_kind,
        radius_meters,
        body: clean_text(body, "body", 280)?,
        body_format: "plain_text".to_string(),
        created_at: ctx.timestamp,
        expires_at_epoch_ms: now_ms + CHAT_TTL_MS,
        moderation_state: "visible".to_string(),
    });
    ctx.db.chat_bubble().insert(ChatBubble {
        bubble_ref,
        message_ref,
        speaker_avatar_ref,
        anchor_entity_ref,
        created_at: ctx.timestamp,
        expires_at_epoch_ms: now_ms + CHAT_BUBBLE_TTL_MS,
    });
    Ok(())
}

fn avatar_ref_for_sender(ctx: &ReducerContext) -> String {
    format!("avatar.identity.{}", ctx.sender())
}

fn ctx_epoch_ms(ctx: &ReducerContext) -> i64 {
    ctx.timestamp.to_micros_since_unix_epoch() / 1_000
}

fn next_ref(ctx: &ReducerContext, prefix: &str) -> String {
    let sequence = ctx.db.local_chat_message().count()
        + ctx.db.chat_bubble().count()
        + ctx.db.local_emote().count()
        + ctx.db.pylon_attention().count();
    format!(
        "{prefix}.{}.{}.{}",
        ctx.sender().to_abbreviated_hex(),
        ctx.timestamp.to_micros_since_unix_epoch(),
        sequence
    )
}

fn ensure_world_region(ctx: &ReducerContext, region_ref: &str) -> Result<WorldRegion, String> {
    ctx.db
        .world_region()
        .region_ref()
        .find(region_ref.to_string())
        .ok_or_else(|| "region_ref is not registered in world_region".to_string())
}

fn validate_position_in_region(
    ctx: &ReducerContext,
    region_ref: &str,
    position_x: f64,
    position_y: f64,
    position_z: f64,
) -> Result<WorldRegion, String> {
    let region = ensure_world_region(ctx, region_ref)?;
    if !position_x.is_finite() || !position_y.is_finite() || !position_z.is_finite() {
        return Err("position coordinates must be finite".to_string());
    }
    if !(region.min_x..=region.max_x).contains(&position_x)
        || !(region.min_y..=region.max_y).contains(&position_y)
        || !(region.min_z..=region.max_z).contains(&position_z)
    {
        return Err("position is outside the registered world_region bounds".to_string());
    }
    Ok(region)
}

fn validate_region_bounds(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
) -> Result<(), String> {
    if !min_x.is_finite()
        || !min_y.is_finite()
        || !min_z.is_finite()
        || !max_x.is_finite()
        || !max_y.is_finite()
        || !max_z.is_finite()
    {
        return Err("world_region bounds must be finite".to_string());
    }
    if min_x >= max_x || min_y >= max_y || min_z >= max_z {
        return Err("world_region min bounds must be lower than max bounds".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_region_metadata(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    road_direction_x: f64,
    road_direction_y: f64,
    road_direction_z: f64,
    local_origin_x: f64,
    local_origin_y: f64,
    local_origin_z: f64,
    starter_pylon_site_offset_x: f64,
    starter_pylon_site_offset_y: f64,
    starter_pylon_site_offset_z: f64,
) -> Result<(), String> {
    if !road_direction_x.is_finite()
        || !road_direction_y.is_finite()
        || !road_direction_z.is_finite()
        || !local_origin_x.is_finite()
        || !local_origin_y.is_finite()
        || !local_origin_z.is_finite()
        || !starter_pylon_site_offset_x.is_finite()
        || !starter_pylon_site_offset_y.is_finite()
        || !starter_pylon_site_offset_z.is_finite()
    {
        return Err("world_region metadata coordinates must be finite".to_string());
    }
    let road_length = (road_direction_x * road_direction_x
        + road_direction_y * road_direction_y
        + road_direction_z * road_direction_z)
        .sqrt();
    if road_length <= 0.000001 {
        return Err("world_region road direction must be nonzero".to_string());
    }
    if !point_inside_bounds(
        min_x,
        min_y,
        min_z,
        max_x,
        max_y,
        max_z,
        local_origin_x,
        local_origin_y,
        local_origin_z,
    ) {
        return Err("world_region local origin must be inside bounds".to_string());
    }
    let pylon_site_x = local_origin_x + starter_pylon_site_offset_x;
    let pylon_site_y = local_origin_y + starter_pylon_site_offset_y;
    let pylon_site_z = local_origin_z + starter_pylon_site_offset_z;
    if !pylon_site_x.is_finite() || !pylon_site_y.is_finite() || !pylon_site_z.is_finite() {
        return Err("world_region starter pylon site must be finite".to_string());
    }
    if !point_inside_bounds(
        min_x,
        min_y,
        min_z,
        max_x,
        max_y,
        max_z,
        pylon_site_x,
        pylon_site_y,
        pylon_site_z,
    ) {
        return Err("world_region starter pylon site must resolve inside bounds".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn point_inside_bounds(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
    x: f64,
    y: f64,
    z: f64,
) -> bool {
    (min_x..=max_x).contains(&x) && (min_y..=max_y).contains(&y) && (min_z..=max_z).contains(&z)
}

fn validate_adjacent_region_ref(
    region_ref: &str,
    adjacent_region_ref: Option<&str>,
    field: &str,
) -> Result<(), String> {
    if adjacent_region_ref.is_some_and(|adjacent| adjacent == region_ref) {
        Err(format!("{field} must not point to region_ref"))
    } else {
        Ok(())
    }
}

fn validate_movement_delta(
    existing: &AvatarPosition,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    now_ms: i64,
) -> Result<(), String> {
    let elapsed_ms = now_ms.saturating_sub(existing.last_seen_epoch_ms);
    if elapsed_ms <= 0 {
        return Ok(());
    }
    let dx = position_x - existing.position_x;
    let dy = position_y - existing.position_y;
    let dz = position_z - existing.position_z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let allowed_distance = MAX_AVATAR_MOVE_METERS_PER_SECOND * (elapsed_ms as f64 / 1_000.0) + 0.75;
    if distance > allowed_distance {
        return Err("position jump exceeds the MVP avatar movement limit".to_string());
    }
    Ok(())
}

fn validate_radius(radius_meters: f64, max_radius_meters: f64) -> Result<f64, String> {
    if !radius_meters.is_finite() || radius_meters <= 0.0 || radius_meters > max_radius_meters {
        return Err("radius_meters is outside the allowed range".to_string());
    }
    Ok(radius_meters)
}

fn validate_ms_window(
    value: i64,
    field: &str,
    min_value: i64,
    max_value: i64,
) -> Result<i64, String> {
    if value < min_value || value > max_value {
        return Err(format!("{field} is outside the allowed range"));
    }
    Ok(value)
}

fn validate_distance(distance_meters: f64) -> Result<f64, String> {
    if !distance_meters.is_finite() || !(0.0..=100.0).contains(&distance_meters) {
        return Err("distance_meters is outside the allowed range".to_string());
    }
    Ok(distance_meters)
}

fn validate_choice(value: String, field: &str, allowed: &[&str]) -> Result<String, String> {
    let cleaned = clean_ref(value, field, 64)?;
    if allowed.iter().any(|allowed| *allowed == cleaned) {
        Ok(cleaned)
    } else {
        Err(format!("{field} is not an allowed value"))
    }
}

fn clean_ref(value: String, field: &str, max_chars: usize) -> Result<String, String> {
    let cleaned = clean_optional_text(value, max_chars)
        .ok_or_else(|| format!("{field} must not be empty"))?;
    if cleaned.chars().any(char::is_whitespace) {
        return Err(format!("{field} must not contain whitespace"));
    }
    Ok(cleaned)
}

fn clean_optional_ref(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> Result<Option<String>, String> {
    match value {
        Some(value) => clean_ref(value, field, max_chars).map(Some),
        None => Ok(None),
    }
}

fn clean_optional_ref_string(
    value: String,
    field: &str,
    max_chars: usize,
) -> Result<Option<String>, String> {
    match clean_optional_text(value, max_chars) {
        Some(value) => clean_ref(value, field, max_chars).map(Some),
        None => Ok(None),
    }
}

fn clean_text(value: String, field: &str, max_chars: usize) -> Result<String, String> {
    clean_optional_text(value, max_chars).ok_or_else(|| format!("{field} must not be empty"))
}

fn clean_optional_text(value: String, max_chars: usize) -> Option<String> {
    let cleaned = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(max_chars)
        .collect::<String>();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
