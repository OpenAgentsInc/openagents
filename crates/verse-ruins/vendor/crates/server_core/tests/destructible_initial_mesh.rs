#![allow(clippy::unwrap_used)]

#[test]
fn initial_destructible_meshes_after_insert() {
    let mut srv = server_core::ServerState::new();
    server_core::scene_build::add_demo_ruins_destructible(&mut srv);

    // Run one apply+remesh step to process initial dirty chunks
    let mut ctx = server_core::ecs::schedule::Ctx::default();
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);

    let deltas = srv.drain_destruct_mesh_deltas();
    assert!(
        !deltas.is_empty(),
        "expected initial mesh deltas after inserting destructible"
    );
}
