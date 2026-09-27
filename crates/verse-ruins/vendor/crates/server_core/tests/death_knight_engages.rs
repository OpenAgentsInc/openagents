use glam::vec3;

#[test]
fn death_knight_moves_then_melees_or_casts() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wiz = s.spawn_wizard_npc(vec3(6.0, 0.6, 0.0));
    let dk = s.spawn_death_knight(vec3(25.0, 0.6, -15.0));
    let start = {
        let dkpos = s.ecs.get(dk).unwrap().tr.pos;
        let wzpos = s.ecs.get(wiz).unwrap().tr.pos;
        (dkpos - wzpos).length()
    };
    let hp0 = s.ecs.get(wiz).unwrap().hp.hp;
    let mut saw_proj = false;
    let mut target_hp_drop = false;
    for _ in 0..120 {
        s.step_authoritative(1.0 / 60.0);
        if s.ecs.iter().any(|e| e.projectile.is_some()) {
            saw_proj = true;
        }
        let hp = s.ecs.get(wiz).unwrap().hp.hp;
        if hp < hp0 {
            target_hp_drop = true;
        }
    }
    let end = {
        let dkpos = s.ecs.get(dk).unwrap().tr.pos;
        let wzpos = s.ecs.get(wiz).unwrap().tr.pos;
        (dkpos - wzpos).length()
    };
    assert!(
        end < start,
        "DK failed to close distance (start={start:.2}, end={end:.2})"
    );
    assert!(
        saw_proj || target_hp_drop,
        "DK neither cast nor dealt melee damage within ~2s"
    );
}
