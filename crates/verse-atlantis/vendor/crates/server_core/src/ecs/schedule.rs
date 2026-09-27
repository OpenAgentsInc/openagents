//! ECS schedule and systems (phase 2).
//!
//! Moves authoritative logic out of `ServerState::step_authoritative` into
//! ordered systems operating on the ECS world + projectiles, with event buses
//! for damage and AoE.

use glam::{Vec2, Vec3};

use crate::ServerState;
use crate::actor::{ActorId, Faction};
use crate::ecs::geom::segment_hits_circle_xz;

// Phase-one scaffolding: stage files (unused public modules) and a test helper.
pub mod schedule_stages;

#[derive(Copy, Clone, Debug)]
pub struct DamageEvent {
    pub src: Option<ActorId>,
    pub dst: ActorId,
    pub amount: i32,
}

#[derive(Copy, Clone, Debug)]
pub struct ExplodeEvent {
    pub center_xz: Vec2,
    pub r2: f32,
    pub src: Option<ActorId>,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum HitKind {
    Direct,
    AoE,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub struct HitEvent {
    pub src: Option<ActorId>,
    pub dst: ActorId,
    pub world_xz: Vec2,
    pub kind: HitKind,
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub struct DeathEvent {
    pub id: ActorId,
    pub killer: Option<ActorId>,
}

#[derive(Default)]
pub struct Ctx {
    pub dt: f32,
    #[allow(dead_code)]
    pub time_s: f32,
    pub dmg: Vec<DamageEvent>,
    pub boom: Vec<ExplodeEvent>,
    #[allow(dead_code)]
    pub hits: Vec<HitEvent>,
    // Server-auth VFX hits to replicate this tick
    pub fx_hits: Vec<net_core::snapshot::HitFx>,
    // HUD toast codes (transient per-tick; platform drains and sends messages)
    pub hud_toasts: Vec<u8>,
    // Per-tick destructible carve requests (from projectiles/explosions)
    pub carves: Vec<ecs_core::components::CarveRequest>,
    pub deaths: Vec<DeathEvent>,
    pub spatial: SpatialGrid,
    pub cmd: crate::ecs::world::CmdBuf,
}

pub struct Schedule;

impl Schedule {
    pub fn run(&mut self, srv: &mut ServerState, ctx: &mut Ctx) {
        let _s = tracing::info_span!("system", name = "input_apply_intents").entered();
        input_apply_intents(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "cooldown_and_mana_tick").entered();
        cooldown_and_mana_tick(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "ai_caster_cast_and_face").entered();
        ai_caster_cast_and_face(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "cast_system").entered();
        cast_system(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "ingest_projectile_spawns").entered();
        ingest_projectile_spawns(srv, ctx);
        drop(_s);
        // Apply spawns immediately so integration/collision see them this tick
        srv.ecs.apply_cmds(&mut ctx.cmd);
        // Rebuild spatial grid once per frame after spawns
        let _s = tracing::info_span!("system", name = "spatial.rebuild").entered();
        ctx.spatial.rebuild(srv);
        drop(_s);
        let _s = tracing::info_span!("system", name = "effects_tick").entered();
        effects_tick(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "ai_move_hostiles").entered();
        ai_move_hostiles(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "separate_undead").entered();
        separate_undead(srv);
        drop(_s);
        let _s = tracing::info_span!("system", name = "melee_apply_when_contact").entered();
        melee_apply_when_contact(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "homing_acquire_targets").entered();
        homing_acquire_targets(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "homing_update").entered();
        homing_update(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "projectile_integrate_ecs").entered();
        projectile_integrate_ecs(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "projectile_collision_ecs").entered();
        projectile_collision_ecs(srv, ctx);
        tracing::info!(
            dmg = ctx.dmg.len(),
            boom = ctx.boom.len(),
            fx_hits = ctx.fx_hits.len(),
            "collision_done"
        );
        drop(_s);
        // Destructibles: derive carves, then apply budgeted mesh+colliders before AoE
        destructible_from_projectiles(srv, ctx);
        destructible_from_explosions(srv, ctx);
        crate::systems::destructible::destructible_apply_carves(srv, ctx);
        crate::systems::destructible::destructible_remesh_budgeted(srv);
        crate::systems::destructible::destructible_refresh_colliders(srv);
        let _s = tracing::info_span!("system", name = "aoe_apply_explosions").entered();
        aoe_apply_explosions(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "faction_flip_on_pc_hits_wizards").entered();
        faction_flip_on_pc_hits_wizards(srv, ctx);
        drop(_s);
        let _s = tracing::info_span!("system", name = "apply_damage_to_ecs").entered();
        apply_damage_to_ecs(srv, ctx);
        drop(_s);
        // death_fx_and_flags(srv, ctx); // hook reserved for SFX/analytics
        let _s = tracing::info_span!("system", name = "cleanup").entered();
        cleanup(srv, ctx);
        drop(_s);
        srv.ecs.apply_cmds(&mut ctx.cmd);
    }
}

// (test helper and stage modules declared earlier in file)

fn targets_by_faction(srv: &ServerState, f: crate::actor::Faction) -> Vec<(ActorId, Vec3, f32)> {
    srv.ecs
        .iter()
        .filter(|a| a.hp.alive() && a.faction == f)
        .map(|a| (a.id, a.tr.pos, a.tr.radius))
        .collect()
}

fn ingest_projectile_spawns(srv: &mut ServerState, ctx: &mut Ctx) {
    if srv.pending_projectiles.is_empty() {
        return;
    }
    let pending: Vec<_> = srv.pending_projectiles.drain(..).collect();
    for cmd in pending {
        let spec = srv.projectile_spec(cmd.kind);
        let dir_n = cmd.dir.normalize_or_zero();
        let yaw = dir_n.x.atan2(dir_n.z);
        let spawn_pos = cmd.pos + dir_n * 0.35; // offset forward to avoid immediate self-collision
        if matches!(cmd.kind, crate::ProjKind::MagicMissile) {
            // Acquire up to 3 distinct targets nearest-first within 30m
            let owner_team = cmd
                .owner
                .and_then(|id| srv.ecs.get(id).map(|a| a.faction))
                .unwrap_or(crate::actor::Faction::Pc);
            let mut cands: Vec<(f32, ActorId)> = srv
                .ecs
                .iter()
                .filter(|a| a.hp.alive() && a.id != cmd.owner.unwrap_or(ActorId(u32::MAX)))
                .filter(|a| srv.factions.effective_hostile(owner_team, a.faction))
                .map(|a| {
                    let dx = a.tr.pos.x - cmd.pos.x;
                    let dz = a.tr.pos.z - cmd.pos.z;
                    (dx * dx + dz * dz, a.id)
                })
                .filter(|(d2, _)| *d2 <= 30.0 * 30.0)
                .collect();
            cands.sort_by(|l, r| l.0.partial_cmp(&r.0).unwrap_or(std::cmp::Ordering::Equal));
            let picks: Vec<ActorId> = cands.into_iter().take(3).map(|(_, id)| id).collect();
            // slight fan for readability; homing will correct course
            let off = 8.0_f32.to_radians();
            let offs = [-off, 0.0, off];
            for (i, yaw_off) in offs.iter().enumerate() {
                let dir = rotate_y(dir_n, *yaw_off).normalize_or_zero();
                let v = dir * spec.speed_mps;
                let target = picks.get(i).copied();
                let homing = target.map(|tid| crate::ecs::world::Homing {
                    target: tid,
                    turn_rate: srv.specs.homing.mm_turn_rate,
                    max_range_m: srv.specs.homing.mm_max_range_m,
                    reacquire: srv.specs.homing.reacquire,
                });
                let comps = crate::ecs::world::Components {
                    id: crate::actor::ActorId(0),
                    kind: crate::actor::ActorKind::Zombie, // placeholder; projectiles are ephemeral, kind unused
                    faction: crate::actor::Faction::Neutral,
                    name: None,
                    tr: crate::actor::Transform {
                        pos: spawn_pos,
                        yaw: yaw + *yaw_off,
                        radius: 0.1,
                    },
                    hp: crate::actor::Health { hp: 1, max: 1 },
                    move_speed: None,
                    aggro: None,
                    attack: None,
                    melee: None,
                    projectile: Some(crate::ecs::world::Projectile {
                        kind: cmd.kind,
                        ttl_s: spec.life_s,
                        age_s: 0.0,
                    }),
                    velocity: Some(crate::ecs::world::Velocity { v }),
                    owner: cmd.owner.map(|id| crate::ecs::world::Owner { id }),
                    homing,
                    spellbook: None,
                    pool: None,
                    cooldowns: None,
                    intent_move: None,
                    intent_aim: None,
                    burning: None,
                    slow: None,
                    stunned: None,
                    despawn_after: None,
                };
                ctx.cmd.spawns.push(comps);
            }
        } else {
            let v = dir_n * spec.speed_mps;
            let comps = crate::ecs::world::Components {
                id: crate::actor::ActorId(0),
                kind: crate::actor::ActorKind::Zombie, // unused for projectile
                faction: crate::actor::Faction::Neutral,
                name: None,
                tr: crate::actor::Transform {
                    pos: spawn_pos,
                    yaw,
                    radius: 0.1,
                },
                hp: crate::actor::Health { hp: 1, max: 1 },
                move_speed: None,
                aggro: None,
                attack: None,
                melee: None,
                projectile: Some(crate::ecs::world::Projectile {
                    kind: cmd.kind,
                    ttl_s: spec.life_s,
                    age_s: 0.0,
                }),
                velocity: Some(crate::ecs::world::Velocity { v }),
                owner: cmd.owner.map(|id| crate::ecs::world::Owner { id }),
                homing: None,
                spellbook: None,
                pool: None,
                cooldowns: None,
                intent_move: None,
                intent_aim: None,
                burning: None,
                slow: None,
                stunned: None,
                despawn_after: None,
            };
            ctx.cmd.spawns.push(comps);
        }
    }
}

#[inline]
fn rotate_y(v: Vec3, yaw_off: f32) -> Vec3 {
    let (s, c) = yaw_off.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, v.z * c - v.x * s)
}

fn homing_update(srv: &mut ServerState, ctx: &mut Ctx) {
    // Pre-fetch MagicMissile speed to avoid borrow conflicts
    let mm_speed = srv
        .projectile_spec(crate::ProjKind::MagicMissile)
        .speed_mps
        .max(0.1);
    let dt = ctx.dt;
    use std::collections::HashMap;
    let pos_map: HashMap<ActorId, Vec3> = srv.ecs.iter().map(|a| (a.id, a.tr.pos)).collect();
    for c in srv.ecs.iter_mut() {
        if let (Some(_proj), Some(vel), Some(hm)) = (
            c.projectile.as_ref(),
            c.velocity.as_mut(),
            c.homing.as_ref(),
        ) {
            let Some(tpos) = pos_map.get(&hm.target).copied() else {
                continue;
            };
            let to = glam::vec3(tpos.x - c.tr.pos.x, 0.0, tpos.z - c.tr.pos.z);
            let dist2 = to.length_squared();
            if dist2 < 1e-6 {
                continue;
            }
            let cur = if vel.v.length_squared() > 1e-6 {
                vel.v.normalize()
            } else {
                glam::vec3(0.0, 0.0, 1.0)
            };
            let cur_yaw = cur.x.atan2(cur.z);
            let want = to.normalize();
            let want_yaw = want.x.atan2(want.z);
            let two_pi = std::f32::consts::TAU;
            let mut delta = want_yaw - cur_yaw;
            // Wrap to [-PI, PI]
            if delta > std::f32::consts::PI {
                delta -= two_pi;
            }
            if delta < -std::f32::consts::PI {
                delta += two_pi;
            }
            let max_step = hm.turn_rate * dt;
            let step = delta.clamp(-max_step, max_step);
            let new_yaw = cur_yaw + step;
            let new_dir = glam::vec3(new_yaw.sin(), 0.0, new_yaw.cos());
            let speed = vel.v.length().max(mm_speed);
            vel.v = new_dir * speed;
        }
    }
}

fn effects_tick(srv: &mut ServerState, ctx: &mut Ctx) {
    let dt = ctx.dt;
    for c in srv.ecs.iter_mut() {
        // Burning ticks damage
        if let Some(mut b) = c.burning {
            if b.remaining_s > 0.0 {
                let dmg = ((b.dps as f32) * dt).floor() as i32;
                if dmg > 0 {
                    ctx.dmg.push(DamageEvent {
                        src: b.src,
                        dst: c.id,
                        amount: dmg,
                    });
                }
                b.remaining_s = (b.remaining_s - dt).max(0.0);
                c.burning = if b.remaining_s > 0.0 { Some(b) } else { None };
            } else {
                c.burning = None;
            }
        }
        // Slow decay
        if let Some(mut s) = c.slow {
            s.remaining_s = (s.remaining_s - dt).max(0.0);
            c.slow = if s.remaining_s > 0.0 { Some(s) } else { None };
        }
        // Stun decay
        if let Some(mut s) = c.stunned {
            s.remaining_s = (s.remaining_s - dt).max(0.0);
            c.stunned = if s.remaining_s > 0.0 { Some(s) } else { None };
        }
        // Despawn timers tick in cleanup
        if let Some(mut d) = c.despawn_after {
            d.seconds = (d.seconds - dt).max(0.0);
            c.despawn_after = Some(d);
        }
    }
}

fn cooldown_and_mana_tick(srv: &mut ServerState, ctx: &Ctx) {
    let dt = ctx.dt;
    for c in srv.ecs.iter_mut() {
        if let Some(cd) = c.cooldowns.as_mut() {
            cd.gcd_ready = (cd.gcd_ready - dt).max(0.0);
            for v in cd.per_spell.values_mut() {
                *v = (*v - dt).max(0.0);
            }
        }
        if let Some(pool) = c.pool.as_mut()
            && pool.regen_per_s > 0.0
            && pool.mana < pool.max
        {
            pool.mana_frac += pool.regen_per_s * dt;
            // Account for float error around exact 1.0 boundaries
            if pool.mana_frac + 1e-6 >= 1.0 {
                let whole = ((pool.mana_frac + 1e-6).floor()) as i32;
                if whole > 0 {
                    pool.mana = (pool.mana + whole).min(pool.max);
                    pool.mana_frac -= whole as f32;
                }
            }
            if pool.mana >= pool.max {
                pool.mana = pool.max;
                pool.mana_frac = 0.0;
            }
        }
    }
}

pub fn cast_system(srv: &mut ServerState, ctx: &mut Ctx) {
    if srv.pending_casts.is_empty() {
        return;
    }
    let casts: Vec<_> = srv.pending_casts.drain(..).collect();
    for cmd in casts {
        let Some(caster) = cmd.caster else {
            continue;
        };
        let bypass_gating = std::env::var("RA_SKIP_CAST_GATING")
            .map(|v| v == "1")
            .unwrap_or(false);
        let (cost, cd_s, gcd_s) = srv.spell_cost_cooldown(cmd.spell);
        let Some(c) = srv.ecs.get_mut(caster) else {
            continue;
        };
        // Spellbook check: caster must know the spell
        if let Some(book) = c.spellbook.as_ref() {
            let known =
                book.known.contains(&cmd.spell) || book.known.contains(&spell_id_map(cmd.spell));
            if !known {
                if std::env::var("RA_LOG_CASTS").ok().as_deref() == Some("1") {
                    log::info!("srv: cast rejected {:?} (unknown spell)", cmd.spell);
                }
                continue;
            }
        }
        // Gating (stun/mana/GCD) unless bypassed by env
        // Gather pre-state for logging
        let mana_before: Option<i32> = c.pool.as_ref().map(|p| p.mana);
        let mut mana_after: Option<i32> = None;
        let mut gcd_ready_val: f32 = c.cooldowns.as_ref().map(|cd| cd.gcd_ready).unwrap_or(0.0);
        let mut spell_cd_val: f32 = c
            .cooldowns
            .as_ref()
            .and_then(|cd| cd.per_spell.get(&cmd.spell).copied())
            .unwrap_or(0.0);

        if !bypass_gating {
            // Stun blocks casting
            if c.stunned.is_some() {
                log::info!("srv: cast rejected (stunned)");
                continue;
            }
            // Cooldown & mana checks
            let mut ok = true;
            if let Some(cd) = c.cooldowns.as_mut() {
                if cd.gcd_ready > 0.0 {
                    ok = false;
                }
                if let Some(rem) = cd.per_spell.get(&cmd.spell)
                    && *rem > 0.0
                {
                    ok = false;
                }
                if ok {
                    cd.gcd_ready = cd.gcd_s.max(0.0);
                    cd.per_spell.insert(cmd.spell, cd_s.max(0.0));
                }
                // refresh for logging after potential writes
                gcd_ready_val = cd.gcd_ready;
                spell_cd_val = cd.per_spell.get(&cmd.spell).copied().unwrap_or(0.0);
            }
            let mut not_enough_mana = false;
            if ok && let Some(pool) = c.pool.as_mut() {
                if pool.mana < cost {
                    ok = false;
                    not_enough_mana = true;
                } else {
                    pool.mana -= cost;
                }
                mana_after = Some(pool.mana);
            }
            if !ok {
                // Emit HUD toast for insufficient mana for the local PC caster
                if not_enough_mana && c.faction == crate::actor::Faction::Pc {
                    ctx.hud_toasts.push(1u8); // 1 = Not enough mana
                }
                if std::env::var("RA_LOG_CASTS").ok().as_deref() == Some("1") {
                    let mb = mana_before.unwrap_or(-1);
                    let ma = mana_after.unwrap_or(mb);
                    log::info!(
                        target: "server_core::ecs::schedule",
                        "srv: cast rejected {:?} gcd_ready={:.2} spell_cd={:.2} mana={}/{} cost={}",
                        cmd.spell,
                        gcd_ready_val,
                        spell_cd_val,
                        ma,
                        c.pool.as_ref().map(|p| p.max).unwrap_or(0),
                        cost
                    );
                }
                continue;
            }
        }
        // Capture values for logging before releasing borrow of caster
        let mb = mana_before.unwrap_or(-1);
        let ma = mana_after.unwrap_or(mb);
        let spell = cmd.spell;
        let pos = cmd.pos;
        let dir = cmd.dir;
        let _ = c;
        // Translate spell to projectiles (no borrow of caster's components beyond id)
        match spell {
            crate::SpellId::Firebolt => {
                srv.spawn_projectile_from(caster, pos, dir, crate::ProjKind::Firebolt)
            }
            crate::SpellId::Fireball => {
                srv.spawn_projectile_from(caster, pos, dir, crate::ProjKind::Fireball)
            }
            crate::SpellId::MagicMissile => {
                srv.spawn_projectile_from(caster, pos, dir, crate::ProjKind::MagicMissile)
            }
        }
        if std::env::var("RA_LOG_CASTS").ok().as_deref() == Some("1") {
            log::info!(
                target: "server_core::ecs::schedule",
                "srv: cast accepted {:?} cost={} gcd_s={:.2} cd_s={:.2} mana_before={} mana_after={}",
                spell, cost, gcd_s, cd_s, mb, ma
            );
        }
    }
}

#[inline]
fn spell_id_map(s: crate::SpellId) -> crate::SpellId {
    s
}

fn ai_move_hostiles(srv: &mut ServerState, ctx: &Ctx) {
    // Build target set: NPC Wizards and the PC (Faction::Pc)
    let mut wiz = targets_by_faction(srv, crate::actor::Faction::Wizards);
    let mut pc = targets_by_faction(srv, crate::actor::Faction::Pc);
    wiz.append(&mut pc);
    if wiz.is_empty() {
        return;
    }
    // Any alive actor with MoveSpeed + AggroRadius and hostile to Wizards
    let mover_ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && a.move_speed.is_some() && a.aggro.is_some())
        .filter(|a| {
            srv.factions
                .effective_hostile(a.faction, crate::actor::Faction::Wizards)
        })
        .map(|a| a.id)
        .collect();
    for uid in mover_ids {
        let (pos, rad, speed, extra, aggro_m, stunned) = if let Some(a) = srv.ecs.get(uid) {
            (
                a.tr.pos,
                a.tr.radius,
                a.move_speed.map(|s| s.mps).unwrap_or(2.0) * a.slow.map(|s| s.mul).unwrap_or(1.0),
                a.attack.map(|r| r.m).unwrap_or(0.35),
                a.aggro.map(|ag| ag.m),
                a.stunned.is_some(),
            )
        } else {
            continue;
        };
        if stunned {
            continue;
        }
        // Find nearest wizard
        let mut best: Option<(f32, Vec3, f32)> = None;
        for (_tid, p, r) in &wiz {
            let dx = p.x - pos.x;
            let dz = p.z - pos.z;
            let d2 = dx * dx + dz * dz;
            if let Some(a) = aggro_m
                && d2 > a * a
            {
                continue;
            }
            if best.as_ref().map(|(b, _, _)| d2 < *b).unwrap_or(true) {
                best = Some((d2, *p, *r));
            }
        }
        if let Some((_d2, tp, tr)) = best {
            let to = Vec3::new(tp.x - pos.x, 0.0, tp.z - pos.z);
            let dist = to.length();
            let contact = rad + tr + extra;
            if dist > contact + 0.02 {
                let step = (speed * ctx.dt).min(dist - contact);
                if step > 1e-4
                    && let Some(a) = srv.ecs.get_mut(uid)
                {
                    a.tr.pos += to.normalize_or_zero() * step;
                }
            }
        }
    }
}

fn melee_apply_when_contact(srv: &mut ServerState, ctx: &mut Ctx) {
    let wiz = targets_by_faction(srv, crate::actor::Faction::Wizards);
    // Any hostile actor that has a Melee component
    let attacker_ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && a.melee.is_some())
        .filter(|a| {
            srv.factions
                .effective_hostile(a.faction, crate::actor::Faction::Wizards)
        })
        .map(|a| a.id)
        .collect();
    for uid in attacker_ids {
        let (pos_u, rad_u, extra, mut cd_ready, cd_total, dmg, stunned) =
            if let Some(a) = srv.ecs.get(uid) {
                (
                    a.tr.pos,
                    a.tr.radius,
                    a.attack.map(|r| r.m).unwrap_or(0.35),
                    a.melee.map(|m| m.ready_in_s).unwrap_or(0.0),
                    a.melee.map(|m| m.cooldown_s).unwrap_or(0.6),
                    a.melee.map(|m| m.damage).unwrap_or(5),
                    a.stunned.is_some(),
                )
            } else {
                continue;
            };
        if stunned {
            continue;
        }
        let mut best: Option<(ActorId, f32, Vec3, f32)> = None;
        for (tid, p, r) in &wiz {
            let dx = p.x - pos_u.x;
            let dz = p.z - pos_u.z;
            let d2 = dx * dx + dz * dz;
            if best.as_ref().map(|(_, b, _, _)| d2 < *b).unwrap_or(true) {
                best = Some((*tid, d2, *p, *r));
            }
        }
        if let Some((tid, _d2, tp, tr)) = best {
            let to = Vec3::new(tp.x - pos_u.x, 0.0, tp.z - pos_u.z);
            let dist = to.length();
            let reach = rad_u + tr + extra;
            // Cooldown update
            cd_ready = (cd_ready - ctx.dt).max(0.0);
            if dist <= reach && cd_ready <= 0.0 {
                ctx.dmg.push(DamageEvent {
                    src: Some(uid),
                    dst: tid,
                    amount: dmg,
                });
                // write back cooldown
                if let Some(u) = srv.ecs.get_mut(uid)
                    && let Some(m) = &mut u.melee
                {
                    m.ready_in_s = cd_total.max(0.05);
                }
            } else {
                // write back cd after decrement
                if let Some(u) = srv.ecs.get_mut(uid)
                    && let Some(m) = &mut u.melee
                {
                    m.ready_in_s = cd_ready;
                }
            }
        }
    }
}

/// Simple separation pass to keep Undead from stacking on the same spot.
/// O(N^2) over Undead; acceptable for demo sizes. Pushes each pair apart equally.
fn separate_undead(srv: &mut ServerState) {
    // Collect indices of alive undead (including Boss under Undead team)
    let ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && a.faction == crate::actor::Faction::Undead)
        .map(|a| a.id)
        .collect();
    let n = ids.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let (ai, aj) = (ids[i], ids[j]);
            let (pi, ri) = if let Some(a) = srv.ecs.get(ai) {
                (a.tr.pos, a.tr.radius)
            } else {
                continue;
            };
            let (pj, rj) = if let Some(a) = srv.ecs.get(aj) {
                (a.tr.pos, a.tr.radius)
            } else {
                continue;
            };
            let to = pj - pi;
            let min_d = ri + rj + 0.1; // pad
            let d = to.length();
            if d < 1e-6 {
                // same position; nudge along X
                if let Some(a) = srv.ecs.get_mut(ai) {
                    a.tr.pos.x -= min_d * 0.5;
                }
                if let Some(a) = srv.ecs.get_mut(aj) {
                    a.tr.pos.x += min_d * 0.5;
                }
                continue;
            }
            if d < min_d {
                let push = (min_d - d) * 0.5;
                let dir = to / d.max(1e-6);
                if let Some(a) = srv.ecs.get_mut(ai) {
                    a.tr.pos -= dir * push;
                }
                if let Some(a) = srv.ecs.get_mut(aj) {
                    a.tr.pos += dir * push;
                }
            }
        }
    }
}

fn ai_caster_cast_and_face(srv: &mut ServerState, _ctx: &mut Ctx) {
    use crate::{CastCmd, SpellId};
    // Snapshot alive actors (pos, radius, faction)
    let alive: Vec<(ActorId, Vec3, f32, crate::actor::Faction)> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive())
        .map(|a| (a.id, a.tr.pos, a.tr.radius, a.faction))
        .collect();
    if alive.is_empty() {
        return;
    }
    // Iterate any caster (has Spellbook and ResourcePool), excluding the player-controlled PC
    let caster_ids: Vec<ActorId> = srv
        .ecs
        .iter()
        .filter(|a| a.hp.alive() && a.spellbook.is_some() && a.pool.is_some())
        .filter(|a| a.faction != crate::actor::Faction::Pc)
        .map(|a| a.id)
        .collect();
    for cid in caster_ids {
        let (cpos, cfaction, stunned) = if let Some(c) = srv.ecs.get(cid) {
            (c.tr.pos, c.faction, c.stunned.is_some())
        } else {
            continue;
        };
        // Face nearest hostile
        let mut best: Option<(f32, Vec3, f32, crate::actor::Faction)> = None;
        for (_id, p, r, tf) in &alive {
            if !srv.factions.effective_hostile(cfaction, *tf) {
                continue;
            }
            let dx = p.x - cpos.x;
            let dz = p.z - cpos.z;
            let d2 = dx * dx + dz * dz;
            if best.as_ref().map(|(b, _, _, _)| d2 < *b).unwrap_or(true) {
                best = Some((d2, *p, *r, *tf));
            }
        }
        let Some((d2, tp, tr, _tf)) = best else {
            continue;
        };
        let dir = tp - cpos;
        let dir_n = if dir.length_squared() > 1e-6 {
            dir.normalize()
        } else {
            Vec3::Z
        };
        // update yaw
        if let Some(c) = srv.ecs.get_mut(cid) {
            c.tr.yaw = dir_n.x.atan2(dir_n.z);
            // Ensure resources exist (failsafe for old spawns)
            if c.cooldowns.is_none() {
                use std::collections::HashMap;
                c.cooldowns = Some(crate::ecs::world::Cooldowns {
                    gcd_s: 0.30,
                    gcd_ready: 0.0,
                    per_spell: HashMap::new(),
                });
            }
        }
        if stunned {
            continue;
        }
        let dist = d2.sqrt();
        // Prefer melee when in reach for actors that have it; let melee system handle the hit
        let prefer_melee = if let Some(c) = srv.ecs.get(cid) {
            let extra = c.attack.map(|r| r.m).unwrap_or(0.35);
            let reach = c.tr.radius + tr + extra;
            dist <= reach + 0.2
        } else {
            false
        };
        let fb_aoe_r = srv
            .projectile_spec(crate::ProjKind::Fireball)
            .aoe_radius_m
            .max(0.0);
        // Count hostiles near target within FB AoE radius
        let mut near_count = 0usize;
        for (_id, p, _r, tf) in &alive {
            if !srv.factions.effective_hostile(cfaction, *tf) {
                continue;
            }
            if (*p - tp).length() <= fb_aoe_r {
                near_count += 1;
            }
        }
        // Choose a desired spell based on distance/cluster, filtered by the caster's spellbook.
        let mut want_spell: Option<SpellId> = None;
        if !prefer_melee {
            let mut prefs: Vec<SpellId> = if near_count >= 2 && (12.0..=25.0).contains(&dist) {
                vec![SpellId::Fireball, SpellId::MagicMissile, SpellId::Firebolt]
            } else if dist <= 10.0 {
                vec![SpellId::MagicMissile, SpellId::Firebolt, SpellId::Fireball]
            } else {
                vec![SpellId::Firebolt, SpellId::MagicMissile, SpellId::Fireball]
            };
            if let Some(c) = srv.ecs.get(cid)
                && let Some(book) = c.spellbook.as_ref()
            {
                prefs.retain(|s| book.known.contains(s));
            }
            want_spell = prefs.into_iter().next();
        }
        // Gate on cooldowns & mana
        let mut ok = true;
        if let Some(c) = srv.ecs.get(cid) {
            if let Some(cd) = c.cooldowns.as_ref() {
                if cd.gcd_ready > 0.0 {
                    ok = false;
                }
                if let Some(sp) = want_spell
                    && cd.per_spell.get(&sp).copied().unwrap_or(0.0) > 0.0
                {
                    ok = false;
                }
            }
            if let (Some(pool), Some(sp)) = (c.pool.as_ref(), want_spell) {
                let (cost, _cd, _gcd) = srv.spell_cost_cooldown(sp);
                if pool.mana < cost {
                    ok = false;
                }
            }
        }
        if !ok {
            continue;
        }
        if let Some(sp) = want_spell {
            let muzzle = cpos + dir_n * 0.35;
            srv.pending_casts.push(CastCmd {
                pos: muzzle,
                dir: dir_n,
                spell: sp,
                caster: Some(cid),
            });
        }
    }
}

fn input_apply_intents(srv: &mut ServerState, ctx: &mut Ctx) {
    let dt = ctx.dt;
    for c in srv.ecs.iter_mut() {
        // Aim intent
        if let Some(aim) = c.intent_aim.take() {
            c.tr.yaw = aim.yaw;
        }
        // Move intent
        if let Some(mov) = c.intent_move.take() {
            if c.stunned.is_some() {
                continue;
            }
            let mut dir = Vec3::new(mov.dx, 0.0, mov.dz);
            if dir.length_squared() <= 1e-6 {
                continue;
            }
            dir = dir.normalize();
            let base = c.move_speed.map(|s| s.mps).unwrap_or(5.0);
            let speed =
                base * if mov.run { 1.6 } else { 1.0 } * c.slow.map(|s| s.mul).unwrap_or(1.0);
            c.tr.pos += dir * speed * dt;
        }
    }
}

fn projectile_integrate_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    let fb_aoe_r2 = {
        let s = srv.projectile_spec(crate::ProjKind::Fireball);
        (s.aoe_radius_m * s.aoe_radius_m).max(0.0)
    };
    for c in srv.ecs.iter_mut() {
        if let (Some(proj), Some(vel)) = (c.projectile.as_mut(), c.velocity.as_ref()) {
            c.tr.pos += vel.v * ctx.dt;
            proj.age_s += ctx.dt;
            if proj.age_s >= proj.ttl_s {
                if matches!(proj.kind, crate::ProjKind::Fireball) {
                    ctx.boom.push(ExplodeEvent {
                        center_xz: Vec2::new(c.tr.pos.x, c.tr.pos.z),
                        r2: fb_aoe_r2,
                        src: c.owner.map(|o| o.id),
                    });
                }
                ctx.cmd.despawns.push(c.id);
            }
        }
    }
}

fn projectile_collision_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    // copy list of ids to iterate without borrow issues
    let fireball_r2 = {
        let s = srv.projectile_spec(crate::ProjKind::Fireball);
        (s.aoe_radius_m * s.aoe_radius_m).max(0.0)
    };
    let mut list: Vec<(ActorId, Vec3, Vec3, crate::ProjKind, Option<ActorId>, f32)> = Vec::new();
    for c in srv.ecs.iter() {
        if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
            let p1 = c.tr.pos;
            let p0 = p1 - vel.v * ctx.dt;
            list.push((c.id, p0, p1, proj.kind, c.owner.map(|o| o.id), proj.age_s));
        }
    }
    let mut to_apply_slow: Vec<ActorId> = Vec::new();
    for (pid, p0, p1, kind, owner, age_s) in list {
        // Arming delay: skip collisions briefly to prevent immediate detonation on spawn and
        // ensure at least one snapshot includes the projectile for visuals.
        let arm_delay = srv.projectile_spec(kind).arming_delay_s.max(0.0);
        let arm_ok = age_s >= arm_delay;
        if !arm_ok {
            continue;
        }
        let owner_team = owner
            .and_then(|id| srv.ecs.get(id).map(|a| a.faction))
            .unwrap_or(Faction::Pc);
        // test against actors (broad-phase via spatial grid)
        let mut hit_any = false;
        let seg_a = Vec2::new(p0.x, p0.z);
        let seg_b = Vec2::new(p1.x, p1.z);
        // Conservative pad to include typical radii; precise test follows.
        let cand_ids = ctx.spatial.query_segment(seg_a, seg_b, 2.0);
        let mut local_hits = 0usize;
        for aid in &cand_ids {
            let Some(a) = srv.ecs.get(*aid) else { continue };
            if !a.hp.alive() {
                continue;
            }
            if a.projectile.is_some() {
                continue;
            }
            if let Some(owner_id) = owner
                && owner_id == a.id
            {
                continue;
            }
            // Allow PC→Wizard hits even if faction matrix is neutral (demo parity)
            let target_team = a.faction;
            let hostile = srv.factions.effective_hostile(owner_team, target_team)
                || (owner_team == Faction::Pc && target_team == Faction::Wizards);
            if !hostile {
                continue;
            }
            if segment_hits_circle_xz(p0, p1, a.tr.pos, a.tr.radius) {
                match kind {
                    crate::ProjKind::Fireball => {
                        ctx.boom.push(ExplodeEvent {
                            center_xz: Vec2::new(p1.x, p1.z),
                            r2: fireball_r2,
                            src: owner,
                        });
                    }
                    _ => {
                        ctx.dmg.push(DamageEvent {
                            src: owner,
                            dst: a.id,
                            amount: projectile_damage(srv, kind),
                        });
                        let kind_byte = match kind {
                            crate::ProjKind::Firebolt => 0u8,
                            crate::ProjKind::Fireball => 1u8,
                            crate::ProjKind::MagicMissile => 2u8,
                        };
                        ctx.fx_hits.push(net_core::snapshot::HitFx {
                            kind: kind_byte,
                            pos: [p1.x, p1.y, p1.z],
                        });
                        if matches!(kind, crate::ProjKind::MagicMissile) {
                            to_apply_slow.push(a.id);
                        }
                    }
                }
                local_hits += 1;
                hit_any = true;
                break;
            }
        }
        if hit_any {
            ctx.cmd.despawns.push(pid);
        } else if matches!(kind, crate::ProjKind::Fireball) {
            // proximity explode: nearest pass within AoE radius
            let r2 = fireball_r2;
            let seg_a = Vec2::new(p0.x, p0.z);
            let seg_b = Vec2::new(p1.x, p1.z);
            let ab = seg_b - seg_a;
            let len2 = ab.length_squared();
            let mut best_d2 = f32::INFINITY;
            let mut best_center = seg_b;
            let mid = (seg_a + seg_b) * 0.5;
            let seg_half = (seg_b - seg_a).length() * 0.5;
            let query_r = seg_half + r2.sqrt() + 1.0;
            // Additional safety: avoid self-detonation near the caster. Skip the owner actor
            // and require a brief extra delay before proximity checks so the projectile has
            // traveled away from the caster bubble.
            // Note: collisions are already gated by arming_delay; this adds a small margin
            // specifically for proximity explosion.
            let min_prox_delay_s: f32 = 0.20;
            if age_s < min_prox_delay_s {
                continue;
            }
            for aid in ctx.spatial.query_circle(mid, query_r) {
                let Some(act) = srv.ecs.get(aid) else {
                    continue;
                };
                if !act.hp.alive() {
                    continue;
                }
                if let Some(owner_id) = owner {
                    if act.id == owner_id {
                        // Never proximity-explode on the caster
                        continue;
                    }
                    // Also avoid proximity exploding on actors of the same faction as the caster.
                    let owner_team = srv
                        .ecs
                        .get(owner_id)
                        .map(|a| a.faction)
                        .unwrap_or(crate::actor::Faction::Pc);
                    if act.faction == owner_team {
                        continue;
                    }
                }
                let c = Vec2::new(act.tr.pos.x, act.tr.pos.z);
                let t = if len2 <= 1e-12 {
                    0.0
                } else {
                    ((c - seg_a).dot(ab) / len2).clamp(0.0, 1.0)
                };
                let closest = seg_a + ab * t;
                let d2 = (closest - c).length_squared();
                if d2 < best_d2 {
                    best_d2 = d2;
                    best_center = closest;
                }
            }
            if best_d2 <= r2 {
                ctx.boom.push(ExplodeEvent {
                    center_xz: best_center,
                    r2,
                    src: owner,
                });
                ctx.cmd.despawns.push(pid);
            }
        }
        if std::env::var("RA_LOG_TICK").ok().as_deref() == Some("1") {
            tracing::info!(candidates = cand_ids.len(), hits = local_hits, kind = ?kind, "proj-collision");
        }
    }
    for id in to_apply_slow {
        if let Some(t) = srv.ecs.get_mut(id) {
            t.apply_slow(srv.specs.effects.mm_slow_mul, srv.specs.effects.mm_slow_s);
        }
    }
}

fn destructible_from_projectiles(srv: &mut ServerState, ctx: &mut Ctx) {
    // Collect projectile segments for this tick
    const CARVE_BUS_CAP: usize = 1024;
    let mut segs: Vec<(ActorId, glam::Vec3, glam::Vec3, f32)> = Vec::new();
    for c in srv.ecs.iter() {
        if let (Some(proj), Some(vel)) = (c.projectile.as_ref(), c.velocity.as_ref()) {
            let spec = srv.projectile_spec(proj.kind);
            if !spec.carves_destructibles {
                continue;
            }
            let p1 = c.tr.pos;
            let p0 = p1 - vel.v * ctx.dt;
            // Radius used as carve radius
            let r = spec.carve_radius_m.max(0.0);
            segs.push((c.id, p0, p1, r));
        }
    }
    if segs.is_empty() || srv.destruct_instances.is_empty() {
        return;
    }
    for (pid, p0, p1, radius) in segs {
        for inst in &srv.destruct_instances {
            let min = glam::Vec3::from(inst.world_min);
            let max = glam::Vec3::from(inst.world_max);
            if let Some(t) = crate::ecs::geom::segment_aabb_enter_t(p0, p1, min, max) {
                let hit = p0 + (p1 - p0) * t.max(0.0);
                if ctx.carves.len() >= CARVE_BUS_CAP {
                    metrics::counter!("destruct.carves_dropped").increment(1);
                    continue;
                }
                ctx.carves.push(ecs_core::components::CarveRequest {
                    did: inst.did,
                    center_m: hit.as_dvec3(),
                    radius_m: radius as f64,
                    seed: srv.destruct_registry.cfg.seed,
                    impact_id: pid.0,
                });
            }
        }
    }
}

fn destructible_from_explosions(srv: &mut ServerState, ctx: &mut Ctx) {
    if ctx.boom.is_empty() || srv.destruct_instances.is_empty() {
        return;
    }
    const CARVE_BUS_CAP: usize = 1024;
    const MAX_CARVE_DISTANCE_M: f32 = 30.0;
    for e in &ctx.boom {
        for inst in &srv.destruct_instances {
            let min = glam::Vec3::from(inst.world_min);
            let max = glam::Vec3::from(inst.world_max);
            // Planar XZ AoE; pick a reasonable Y within AABB so carving happens inside volume
            let center = glam::vec3(e.center_xz.x, (min.y + max.y) * 0.5, e.center_xz.y);
            let closest = glam::vec3(
                center.x.clamp(min.x, max.x),
                center.y.clamp(min.y, max.y),
                center.z.clamp(min.z, max.z),
            );
            let dx = closest.x - center.x;
            let dz = closest.z - center.z;
            let planar_d2 = dx * dx + dz * dz;
            if planar_d2 > e.r2 {
                continue; // too far in XZ plane to affect this proxy
            }
            // Hard distance guard to prevent far-away carves (e.g., NPC across arena)
            if planar_d2.sqrt() > MAX_CARVE_DISTANCE_M {
                continue;
            }
            // Surface pick: require a voxel ray hit when approaching from the explosion center.
            // Use the registered proxy to transform WS->OS and raycast into the voxel grid.
            let did = crate::destructible::state::DestructibleId(inst.did);
            let mut allow = false;
            if let Some(proxy) = srv.destruct_registry.proxies.get(&did) {
                let os_c = (proxy.object_from_world * center.extend(1.0)).truncate();
                // clamp to grid AABB in object space to get a direction pointing inward
                let gmin = proxy.grid.origin_m();
                let dims = proxy.grid.dims();
                let vm = proxy.grid.voxel_m().0;
                let gmax = gmin
                    + glam::DVec3::new(dims.x as f64 * vm, dims.y as f64 * vm, dims.z as f64 * vm);
                let c_os = glam::dvec3(os_c.x as f64, os_c.y as f64, os_c.z as f64);
                let clamped = glam::dvec3(
                    c_os.x.clamp(gmin.x, gmax.x),
                    c_os.y.clamp(gmin.y, gmax.y),
                    c_os.z.clamp(gmin.z, gmax.z),
                );
                let dir = (clamped - c_os).normalize_or_zero();
                if dir.length_squared() > 0.0 {
                    let max_dist =
                        core_units::Length::meters(((clamped - c_os).length() + (2.0 * vm)) as f64);
                    if let Some(_hit) =
                        crate::destructible::raycast_voxels(&proxy.grid, c_os, dir, max_dist)
                    {
                        allow = true;
                    }
                }
            }
            if !allow {
                continue; // no surface hit, skip carving this proxy for this explosion
            }
            if ctx.carves.len() >= CARVE_BUS_CAP {
                metrics::counter!("destruct.carves_dropped").increment(1);
                continue;
            }
            ctx.carves.push(ecs_core::components::CarveRequest {
                did: inst.did,
                center_m: closest.as_dvec3(),
                radius_m: (e.r2.max(0.0)).sqrt() as f64,
                seed: srv.destruct_registry.cfg.seed,
                impact_id: e.src.map(|a| a.0).unwrap_or(0),
            });
        }
    }
}

pub fn destructible_from_explosions_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    destructible_from_explosions(srv, ctx);
}

pub fn system_names_for_test() -> Vec<&'static str> {
    vec![
        "input_apply_intents",
        "cooldown_and_mana_tick",
        "ai_caster_cast_and_face",
        "cast_system",
        "ingest_projectile_spawns",
        "spatial.rebuild",
        "effects_tick",
        "ai_move_hostiles",
        "separate_undead",
        "melee_apply_when_contact",
        "homing_acquire_targets",
        "homing_update",
        "projectile_integrate_ecs",
        "projectile_collision_ecS",
        "destructible_from_projectiles",
        "destructible_from_explosions",
        "destructible_apply_carves",
        "destructible_remesh_budgeted",
        "destructible_refresh_colliders",
        "aoe_apply_explosions",
        "faction_flip_on_pc_hits_wizards",
        "apply_damage_to_ecs",
        "cleanup",
    ]
}

fn aoe_apply_explosions(srv: &mut ServerState, ctx: &mut Ctx) {
    for e in ctx.boom.drain(..) {
        let snapshot: Vec<_> = srv
            .ecs
            .iter()
            .map(|a| (a.id, a.tr.pos, a.hp.alive()))
            .collect();
        let mut burning_ids = Vec::new();
        for (aid, pos, alive) in &snapshot {
            if !*alive {
                continue;
            }
            let dx = pos.x - e.center_xz.x;
            let dz = pos.z - e.center_xz.y;
            // Expand AoE by a small pad and the target's collision radius (min 0.30m)
            let r_eff2 = if let Some(a) = srv.ecs.get(*aid) {
                let r_capsule = a.tr.radius.max(0.30);
                let r_eff = e.r2.max(0.0).sqrt() + 0.25 + r_capsule;
                r_eff * r_eff
            } else {
                e.r2
            };
            if dx * dx + dz * dz <= r_eff2 {
                // Hostility override for PC→Wizard AoE in demo parity
                let owner_team = e
                    .src
                    .and_then(|id| srv.ecs.get(id).map(|a| a.faction))
                    .unwrap_or(Faction::Pc);
                let target_team = srv
                    .ecs
                    .get(*aid)
                    .map(|a| a.faction)
                    .unwrap_or(Faction::Undead);
                let hostile = srv.factions.effective_hostile(owner_team, target_team)
                    || (owner_team == Faction::Pc && target_team == Faction::Wizards);
                if hostile {
                    ctx.dmg.push(DamageEvent {
                        src: e.src,
                        dst: *aid,
                        amount: projectile_damage_aoe(srv),
                    });
                }
                burning_ids.push(*aid);
            }
        }
        for id in burning_ids {
            if let Some(t) = srv.ecs.get_mut(id) {
                t.apply_burning(
                    srv.specs.effects.fireball_burn_dps,
                    srv.specs.effects.fireball_burn_s,
                    e.src,
                );
            }
        }
    }
}

fn faction_flip_on_pc_hits_wizards(srv: &mut ServerState, ctx: &mut Ctx) {
    for d in &ctx.dmg {
        if let Some(src) = d.src
            && let (Some(sa), Some(v)) = (srv.ecs.get(src), srv.ecs.get(d.dst))
            && sa.faction == Faction::Pc
            && v.faction == Faction::Wizards
        {
            srv.factions.pc_vs_wizards_hostile = true;
        }
    }
}

fn apply_damage_to_ecs(srv: &mut ServerState, ctx: &mut Ctx) {
    for d in ctx.dmg.drain(..) {
        if let Some(a) = srv.ecs.get_mut(d.dst) {
            let pre = a.hp.hp;
            a.hp.hp = (a.hp.hp - d.amount).max(0);
            if pre > 0 && a.hp.hp == 0 {
                ctx.deaths.push(DeathEvent {
                    id: a.id,
                    killer: d.src,
                });
                a.despawn_after = Some(crate::ecs::world::DespawnAfter { seconds: 2.0 });
            }
        }
    }
}

fn cleanup(srv: &mut ServerState, _ctx: &mut Ctx) {
    // Despawn entities whose timers reached 0. If an entity is dead but has no
    // timer (should be rare), despawn it immediately to avoid leaks. We avoid
    // calling `remove_dead()` here so bodies can linger until their timer elapses.
    let mut to_despawn = Vec::new();
    for c in srv.ecs.iter() {
        if let Some(d) = c.despawn_after {
            if d.seconds <= 0.0 {
                to_despawn.push(c.id);
            }
            continue;
        }
        if !c.hp.alive() {
            to_despawn.push(c.id);
        }
    }
    if !to_despawn.is_empty() {
        let mut cmd = crate::ecs::world::CmdBuf {
            spawns: Vec::new(),
            despawns: to_despawn,
        };
        srv.ecs.apply_cmds(&mut cmd);
    }
}

#[inline]
fn projectile_damage(srv: &ServerState, kind: crate::ProjKind) -> i32 {
    srv.projectile_spec(kind).damage
}

#[inline]
fn projectile_damage_aoe(srv: &ServerState) -> i32 {
    srv.projectile_spec(crate::ProjKind::Fireball).damage
}

// segment intersection helper is in ecs::geom

// ----------------------------------------------------------------------------
// Spatial grid (2D XZ uniform grid) for broad-phase queries
// ----------------------------------------------------------------------------

use std::collections::HashMap;

#[derive(Default)]
pub struct SpatialGrid {
    cell: f32,
    buckets: HashMap<(i32, i32), Vec<ActorId>>,
}

impl SpatialGrid {
    pub fn rebuild(&mut self, srv: &ServerState) {
        self.cell = 4.0; // meters per cell
        self.buckets.clear();
        for a in srv.ecs.iter() {
            let key = self.key(a.tr.pos.x, a.tr.pos.z);
            self.buckets.entry(key).or_default().push(a.id);
        }
    }
    fn key(&self, x: f32, z: f32) -> (i32, i32) {
        let cx = (x / self.cell).floor() as i32;
        let cz = (z / self.cell).floor() as i32;
        (cx, cz)
    }
    #[allow(dead_code)]
    pub fn query_circle(&self, center: Vec2, r: f32) -> impl Iterator<Item = ActorId> + '_ {
        let cr = (r / self.cell).ceil() as i32;
        let (cx, cz) = (
            (center.x / self.cell).floor() as i32,
            (center.y / self.cell).floor() as i32,
        );
        let mut out: Vec<ActorId> = Vec::new();
        for dx in -cr..=cr {
            for dz in -cr..=cr {
                if let Some(v) = self.buckets.get(&(cx + dx, cz + dz)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_by_key(|id| id.0);
        out.into_iter()
    }

    /// Return candidate actor ids by traversing grid cells along the segment (2D DDA) and
    /// including neighboring cells within `pad` meters. Precise collision is done by the caller.
    pub fn query_segment(&self, a: Vec2, b: Vec2, pad: f32) -> Vec<ActorId> {
        use std::collections::HashSet;
        let cell = self.cell.max(0.001);
        let rpad = (pad / cell).ceil() as i32;
        let mut visited: HashSet<(i32, i32)> = HashSet::new();
        let mut add_bucket = |cx: i32, cz: i32| {
            visited.insert((cx, cz));
        };
        // Convert to cell space
        let a_cell = Vec2::new(a.x / cell, a.y / cell);
        let b_cell = Vec2::new(b.x / cell, b.y / cell);
        let mut cx = a_cell.x.floor() as i32;
        let mut cz = a_cell.y.floor() as i32;
        let tx = if b_cell.x > a_cell.x { 1 } else { -1 };
        let tz = if b_cell.y > a_cell.y { 1 } else { -1 };
        let dx = (b_cell.x - a_cell.x).abs();
        let dz = (b_cell.y - a_cell.y).abs();
        let mut t_max_x = if dx.abs() < 1e-6 {
            f32::INFINITY
        } else {
            let next_v = if tx > 0 { (cx + 1) as f32 } else { cx as f32 };
            ((next_v - a_cell.x) / dx).abs()
        };
        let mut t_max_z = if dz.abs() < 1e-6 {
            f32::INFINITY
        } else {
            let next_v = if tz > 0 { (cz + 1) as f32 } else { cz as f32 };
            ((next_v - a_cell.y) / dz).abs()
        };
        let t_delta_x = if dx.abs() < 1e-6 {
            f32::INFINITY
        } else {
            (1.0 / dx).abs()
        };
        let t_delta_z = if dz.abs() < 1e-6 {
            f32::INFINITY
        } else {
            (1.0 / dz).abs()
        };
        // Steps: number of cells along dominant axis
        let steps = (dx.max(dz)).ceil() as i32 + 2;
        for _ in 0..steps {
            // include cell and neighbors within rpad
            for nx in (cx - rpad)..=(cx + rpad) {
                for nz in (cz - rpad)..=(cz + rpad) {
                    add_bucket(nx, nz);
                }
            }
            if t_max_x < t_max_z {
                cx += tx;
                t_max_x += t_delta_x;
            } else {
                cz += tz;
                t_max_z += t_delta_z;
            }
        }
        let mut out: Vec<ActorId> = Vec::new();
        for (cx, cz) in visited {
            if let Some(v) = self.buckets.get(&(cx, cz)) {
                out.extend_from_slice(v);
            }
        }
        out.sort_by_key(|id| id.0);
        out.dedup_by_key(|id| id.0);
        out
    }
}
fn homing_acquire_targets(srv: &mut ServerState, ctx: &mut Ctx) {
    // Build quick maps to avoid borrow conflicts
    use std::collections::HashMap;
    let mut alive: HashMap<ActorId, (Vec3, crate::actor::Faction)> = HashMap::new();
    for a in srv.ecs.iter() {
        if a.hp.alive() {
            alive.insert(a.id, (a.tr.pos, a.faction));
        }
    }

    // Iterate projectiles and reacquire if needed
    // Collect ids first to avoid borrowing issues
    let mut proj_ids = Vec::new();
    for c in srv.ecs.iter() {
        if c.projectile.is_some() && c.homing.is_some() {
            proj_ids.push(c.id);
        }
    }

    for pid in proj_ids {
        let (p_pos, owner_team, homing) = if let Some(c) = srv.ecs.get(pid) {
            let team = c
                .owner
                .and_then(|o| srv.ecs.get(o.id).map(|a| a.faction))
                .unwrap_or(crate::actor::Faction::Pc);
            (c.tr.pos, team, c.homing)
        } else {
            continue;
        };
        let Some(hm) = homing else {
            continue;
        };
        if !hm.reacquire {
            continue;
        }

        let need_reacquire = match alive.get(&hm.target) {
            None => true,
            Some((tpos, _)) => {
                let dx = tpos.x - p_pos.x;
                let dz = tpos.z - p_pos.z;
                let d2 = dx * dx + dz * dz;
                d2 > hm.max_range_m * hm.max_range_m
            }
        };
        if !need_reacquire {
            continue;
        }

        // Query spatial grid within range and pick nearest hostile
        let center = glam::Vec2::new(p_pos.x, p_pos.z);
        let mut best: Option<(f32, ActorId)> = None;
        for aid in ctx.spatial.query_circle(center, hm.max_range_m) {
            if let Some((apos, ateam)) = alive.get(&aid) {
                if !srv.factions.effective_hostile(owner_team, *ateam) {
                    continue;
                }
                let dx = apos.x - p_pos.x;
                let dz = apos.z - p_pos.z;
                let d2 = dx * dx + dz * dz;
                if best
                    .map(|(bd2, bid)| (d2, aid.0) < (bd2, bid.0))
                    .unwrap_or(true)
                {
                    best = Some((d2, aid));
                }
            }
        }
        if let Some((_, pick)) = best
            && let Some(c) = srv.ecs.get_mut(pid)
            && let Some(h) = c.homing.as_mut()
        {
            h.target = pick;
        }
    }
}

// ------------------------------
// Test helpers (no-op wrappers)
// ------------------------------
pub fn destructible_from_projectiles_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    destructible_from_projectiles(srv, ctx);
}
pub fn ingest_projectile_spawns_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    ingest_projectile_spawns(srv, ctx);
}
pub fn projectile_integrate_ecs_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    projectile_integrate_ecs(srv, ctx);
}
pub fn projectile_collision_ecs_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    projectile_collision_ecs(srv, ctx);
}
pub fn aoe_apply_explosions_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    aoe_apply_explosions(srv, ctx);
}
pub fn apply_damage_to_ecs_for_test(srv: &mut ServerState, ctx: &mut Ctx) {
    apply_damage_to_ecs(srv, ctx);
}
