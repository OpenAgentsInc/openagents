//! Offline GPU capture of the shared Gym and synthetic desktop board.
//! This opens no identity files or sockets and never requests a run.
#[cfg(feature = "capture")]
fn main() -> Result<(), String> {
    use glam::{Mat4, Vec3};
    use verse::{
        gym::Board,
        hud, render,
        runtime::WorldRuntime,
        ui::{Atlas, UiBatch},
    };
    let output = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .ok_or("pass an output directory for the three synthetic PNG files")?;
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut world = WorldRuntime::new();
    world.player = verse::controller::PlayerController::new(
        Vec3::new(54.0, 0.0, 0.0),
        std::f32::consts::FRAC_PI_2,
    );
    let atlas = Atlas::new(16.0);
    let (width, height) = (1600, 1000);
    let aspect = width as f32 / height as f32;
    let dynamic = world.dynamic_mesh();
    let eye = Vec3::new(18.0, 19.0, 28.0);
    let exterior = render::View {
        eye,
        view_proj: Mat4::perspective_rh(50.0_f32.to_radians(), aspect, 0.1, 400.0)
            * Mat4::look_at_rh(eye, Vec3::new(48.0, 2.0, 0.0), Vec3::Y),
    };
    render::capture(
        &output.join("gym-exterior.png"),
        width,
        height,
        &world.world.mesh,
        exterior,
        &dynamic,
        &UiBatch::default(),
        &atlas,
    )?;
    let view = world.view(aspect);
    render::capture(
        &output.join("gym-interior.png"),
        width,
        height,
        &world.world.mesh,
        view,
        &dynamic,
        &UiBatch::default(),
        &atlas,
    )?;
    let mut board = Board::new(
        secp256k1::SecretKey::from_byte_array([7; 32]).map_err(|e| e.to_string())?,
        true,
    );
    board.set_active(true);
    let first = board
        .view()
        .runs
        .first()
        .map(|r| r.id.clone())
        .ok_or("the synthetic board has no runs")?;
    board.select_run(&first)?;
    let state = board.view();
    let mut ui = UiBatch::default();
    let _ = hud::gym_panel(
        &mut ui,
        &atlas,
        [width as f32, height as f32],
        1.0,
        &hud::GymPanel {
            view: Some(&state),
            recipes: false,
            selected: 0,
            scroll: 0,
            notice: None,
        },
    );
    render::capture(
        &output.join("gym-board.png"),
        width,
        height,
        &world.world.mesh,
        view,
        &dynamic,
        &ui,
        &atlas,
    )
}
#[cfg(not(feature = "capture"))]
fn main() {}
