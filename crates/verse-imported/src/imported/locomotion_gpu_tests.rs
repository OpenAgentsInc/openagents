//! Offscreen acceptance for the final body and equipment palette.
use glam::{DVec3, Mat4, Vec3};
use verse_engine::{
    core::LifeId,
    locomotion::{Aim, Control},
    motion::State,
    presentation::{Instance, Mount, MountPose, View},
    render_world::RenderWorld,
};

#[test]
#[ignore = "Requires an explicitly selected native graphics adapter"]
fn terrain_aim_and_equipment_use_the_same_rendered_palette() {
    let output = std::env::var_os("VERSE_LOCOMOTION_GPU_EVIDENCE")
        .expect("Explicit scratch evidence directory");
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir_all(&output).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut pack = super::original::generate(dir.path()).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/verse/characters/quaternius");
    super::characters::install(&mut pack, dir.path(), &root, "male-peasant").unwrap();
    let atlas = super::original::atlas().unwrap();
    let mut renderer = super::Renderer::new(pack, dir.path(), 640, 360, &atlas, &[]).unwrap();
    let mut scene = physics::queries::Scene::default();
    let mut triangles = Vec::new();
    for (left, right, step) in [(-4., 0., false), (0., 4., true)] {
        let at = |x: f64, z: f64| DVec3::new(x, if step { 0.16 } else { -z * 0.05 }, z);
        let [a, b, c, d] = [at(left, -4.), at(right, -4.), at(right, 4.), at(left, 4.)];
        triangles.extend([
            physics::queries::Triangle([a, c, b]),
            physics::queries::Triangle([a, d, c]),
        ]);
    }
    scene
        .insert(physics::queries::MeshCollider {
            key: physics::queries::ColliderKey {
                life: physics::queries::Life {
                    instance: 1,
                    entity: 900,
                    generation: 0,
                },
                shape: 0,
            },
            layers: u32::MAX,
            usage: physics::queries::Usage::Blocking,
            mesh: physics::queries::Mesh::compile(triangles).unwrap(),
        })
        .unwrap();
    let support = verse_world::animation_support::Queries::new(&scene);
    let eye = Vec3::new(0., 2.5, -6.);
    let view = View {
        eye,
        view_proj: Mat4::perspective_rh(45f32.to_radians(), 640. / 360., 0.1, 100.)
            * Mat4::look_at_rh(eye, Vec3::Y, Vec3::Y),
    };
    let mut lighting = verse_engine::lighting::Lighting {
        ambient: Vec3::splat(0.75),
        density: 0.,
        shadowed: 0,
        ..Default::default()
    };
    let names = ["universal-male-peasant", "universal-female-ranger"];
    let controls = [0, 1].map(|i| Control {
        life: LifeId {
            instance: 1,
            actor: 14 + i,
            generation: 0,
        },
        aim: Aim {
            yaw: 0.2,
            pitch: 0.1,
        },
    });
    let mut contacts = 0;
    let mut maximum_socket_error: f32 = 0.;
    let mut diagnostics = Vec::new();
    for tick in 0..18 {
        lighting.time = tick as f32 / 60.;
        let mut instances = Vec::new();
        for (i, name) in names.iter().enumerate() {
            instances.push(Instance {
                mount: None,
                actor: Some(controls[i].life),
                model: (*name).into(),
                transform: Mat4::from_translation(Vec3::new(
                    if i == 0 { -1.2 } else { 1.2 },
                    0.,
                    -tick as f32 * 0.03,
                )) * verse_content::basis(),
                animation: State::Walk.into(),
                time: lighting.time,
                animation_epoch: Some(1),
                emission: Vec3::ONE,
            });
        }
        for (i, name) in names.iter().enumerate() {
            instances.push(Instance {
                mount: Some(Mount {
                    parent: controls[i].life,
                    parent_model: (*name).into(),
                    socket: 6,
                    local: Mat4::IDENTITY,
                    pose: MountPose::Socket,
                }),
                actor: None,
                model: "gear-wand".into(),
                transform: Mat4::IDENTITY,
                animation: 0.into(),
                time: 0.,
                animation_epoch: None,
                emission: Vec3::ONE,
            });
        }
        let world = RenderWorld::extract(renderer.catalog(), view, &instances, &[], &lighting)
            .unwrap()
            .with_animation(&support, &controls)
            .unwrap();
        let pixels = renderer.draw_world(&world).unwrap();
        for (i, name) in names.iter().enumerate() {
            let body = renderer.evaluated_pose(i).unwrap();
            let gear = renderer.evaluated_pose(i + 2).unwrap();
            let model = &renderer.pack().models[*name];
            let bones = body.1[..model.bones.len()]
                .iter()
                .map(Mat4::from_cols_array_2d)
                .collect::<Vec<_>>();
            let sockets = verse_engine::sockets::Sockets::admit(model).unwrap();
            let expected = sockets
                .frame(
                    verse_engine::sockets::Palette::admit(model, &bones).unwrap(),
                    Mat4::from_cols_array_2d(&body.0),
                    6,
                    Mat4::IDENTITY,
                )
                .unwrap();
            maximum_socket_error = maximum_socket_error.max(
                expected
                    .to_cols_array()
                    .into_iter()
                    .zip(Mat4::from_cols_array_2d(&gear.0).to_cols_array())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f32::max),
            );
        }
        for diagnostic in &renderer.animation_diagnostics {
            let adjustment = diagnostic.adjustment.as_ref().unwrap();
            assert_eq!(adjustment.inputs.aim.yaw, 0.2);
            contacts += adjustment.planted.into_iter().filter(|p| *p).count();
            assert!(
                adjustment
                    .foot_error
                    .into_iter()
                    .all(|e| e < 0.005 || adjustment.reach_clamps > 0)
            );
        }
        assert_eq!(renderer.last_timings.pose_samples, 2);
        diagnostics.push(serde_json::json!({"tick":tick,"poses":renderer.animation_diagnostics,"timings":renderer.last_timings}));
        if tick == 17 {
            assert!(
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] > 40 || p[1] > 40 || p[2] > 40)
                    .count()
                    > 100
            );
            let mut encoder = png::Encoder::new(
                std::fs::File::create(output.join("terrain-aim-equipment.png")).unwrap(),
                640,
                360,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }
    assert!(contacts > 8, "{contacts} planted contacts");
    assert!(maximum_socket_error < 0.00001, "{maximum_socket_error}");
    std::fs::write(output.join("renderer.json"),serde_json::to_vec_pretty(&serde_json::json!({"schema":"verse.locomotion.renderer.v1","adapter":renderer.adapter_name,"device":renderer.device_profile,"models":names,"contacts":contacts,"maximum_socket_error":maximum_socket_error,"frames":diagnostics})).unwrap()).unwrap();
}
