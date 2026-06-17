use spacetimedb::{Identity, ReducerContext, Table, Timestamp};

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

#[spacetimedb::table(accessor = world_event, public)]
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
