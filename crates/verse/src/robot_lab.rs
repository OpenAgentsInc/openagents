//! Robot Lab's inspection scene (`docs/lab/README.md`, slice 1): a robot
//! description posed by forward kinematics, standing on a table beside a
//! cube, as physically lit triangles in Verse's Y-up meters.
//!
//! The description's Z-up frame crosses into Y-up once, at the robot's base
//! ([`robot_description::z_up_to_y_up`]); links are never turned again.
//! A pose outside the joints' limits is refused, not clamped.

use glam::{DMat4, DQuat, DVec3, Mat4, Vec3};
use robot_description::{Error, Geometry, Robot, z_up_to_y_up};

use crate::pbr::{LitVertex, Material};

/// Height of the table top above the floor, in meters.
pub const TABLE_HEIGHT: f32 = 0.75;
/// The table top's size along X and Z, in meters.
pub const TABLE_SIZE: [f32; 2] = [1.2, 0.75];
/// The cube's side, in meters.
pub const CUBE_SIDE: f32 = 0.05;
/// Where the cube's center sits on the table top, from the robot's base,
/// in Verse axes: in front of the robot (its +X) and a little to its side.
pub const CUBE_AT: [f32; 2] = [0.22, -0.1];

const TABLE_THICKNESS: f32 = 0.04;
const LEG: f32 = 0.05;
const SEGMENTS: usize = 24;

/// One posed frame of the inspection scene.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    /// Floor, table, and cube.
    pub props: Vec<LitVertex>,
    /// The robot's visual geometry.
    pub robot: Vec<LitVertex>,
    /// Each link's origin in Verse coordinates, indexed like the robot's links.
    pub links: Vec<Vec3>,
    /// Vertex ranges for link selection in the robot's visual mesh.
    pub link_ranges: Vec<std::ops::Range<usize>>,
    /// Where the robot's base stands on the table.
    pub base: Vec3,
    /// The cube's center.
    pub cube: Vec3,
}

/// The robot's base frame in Verse: on the table's center, Z-up made Y-up.
#[must_use]
pub fn base_frame() -> DMat4 {
    DMat4::from_translation(DVec3::new(0.0, f64::from(TABLE_HEIGHT), 0.0)) * z_up_to_y_up()
}

/// The scene with `robot` at `pose` (one value per movable joint, radians
/// or meters, within the declared limits).
pub fn scene(robot: &Robot, pose: &[f64]) -> Result<Scene, Error> {
    let frames = robot.forward(pose)?;
    let base = base_frame();
    let mut out = Scene {
        base: Vec3::new(0.0, TABLE_HEIGHT, 0.0),
        cube: Vec3::new(CUBE_AT[0], TABLE_HEIGHT + CUBE_SIDE / 2.0, CUBE_AT[1]),
        ..Scene::default()
    };
    out.links = frames
        .iter()
        .map(|frame| (base * *frame).transform_point3(DVec3::ZERO).as_vec3())
        .collect();
    for (link, frame) in robot.links().iter().zip(&frames) {
        let start = out.robot.len();
        for shape in &link.visuals {
            let place = (base * *frame * shape.origin).as_mat4();
            let rgba = shape.rgba.unwrap_or([0.8, 0.8, 0.8, 1.0]);
            let color = [rgba[0], rgba[1], rgba[2]].map(linear);
            let triangles = match &shape.geometry {
                Geometry::Box { size } => cuboid(Mat4::from_scale(size.as_vec3())),
                Geometry::Cylinder { radius, length } => cylinder(*radius as f32, *length as f32),
                Geometry::Sphere { radius } => sphere(*radius as f32),
                Geometry::Mesh { scale, mesh, .. } => {
                    let scale = scale.as_vec3();
                    mesh.triangles
                        .iter()
                        .map(|t| (*t).map(|corner| Vec3::from(corner) * scale))
                        .collect()
                }
            };
            push(
                &mut out.robot,
                place,
                &triangles,
                color,
                Material::WhitePaint,
                0.5,
            );
        }
        out.link_ranges.push(start..out.robot.len());
    }
    // The floor, a wooden table on four legs, and a red cube.
    let (floor, _, _) = Material::Stage.parameters();
    let floor_box = Mat4::from_translation(Vec3::new(0.0, -0.01, 0.0))
        * Mat4::from_scale(Vec3::new(8.0, 0.02, 8.0));
    push(
        &mut out.props,
        floor_box,
        &cuboid(Mat4::IDENTITY),
        floor,
        Material::Stage,
        0.8,
    );
    let wood = [0.42, 0.26, 0.14].map(linear);
    let [width, depth] = TABLE_SIZE;
    let top = Mat4::from_translation(Vec3::new(0.0, TABLE_HEIGHT - TABLE_THICKNESS / 2.0, 0.0))
        * Mat4::from_scale(Vec3::new(width, TABLE_THICKNESS, depth));
    push(
        &mut out.props,
        top,
        &cuboid(Mat4::IDENTITY),
        wood,
        Material::WhitePaint,
        0.7,
    );
    let leg_height = TABLE_HEIGHT - TABLE_THICKNESS;
    for (x, z) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let leg = Mat4::from_translation(Vec3::new(
            x * (width / 2.0 - LEG),
            leg_height / 2.0,
            z * (depth / 2.0 - LEG),
        )) * Mat4::from_scale(Vec3::new(LEG, leg_height, LEG));
        push(
            &mut out.props,
            leg,
            &cuboid(Mat4::IDENTITY),
            wood,
            Material::WhitePaint,
            0.7,
        );
    }
    let red = [0.85, 0.08, 0.06].map(linear);
    let cube = Mat4::from_translation(out.cube) * Mat4::from_scale(Vec3::splat(CUBE_SIDE));
    push(
        &mut out.props,
        cube,
        &cuboid(Mat4::IDENTITY),
        red,
        Material::WhitePaint,
        0.45,
    );
    Ok(out)
}

/// Draw the exact saved scene and, optionally, a physical observation. This
/// viewing path owns no simulator or controller and does not clamp observations.
pub fn document_scene(
    robot: &Robot,
    scene: &robot_lab::document::Scene,
    step: Option<&robot_lab::document::Step>,
) -> Result<Scene, String> {
    scene.validate().map_err(|e| e.to_string())?;
    if scene.robots.len() != 1 {
        return Err("Open a scene with one robot".into());
    }
    let instance = &scene.robots[0];
    let values = step.map_or(&instance.joints, |step| &step.joints);
    let pose: Vec<_> = robot
        .movable()
        .map(|joint| {
            values
                .get(&joint.name)
                .copied()
                .ok_or_else(|| format!("This pose is missing {}", joint.name))
        })
        .collect::<Result<_, _>>()?;
    let frames = if step.is_some() {
        robot.forward_observed(&pose)
    } else {
        robot.forward(&pose)
    }
    .map_err(|e| e.to_string())?;
    let base = saved_pose(&instance.pose) * z_up_to_y_up();
    let mut out = Scene {
        base: Vec3::from_array(instance.pose.position.map(|v| v as f32)),
        ..Scene::default()
    };
    for (link, frame) in robot.links().iter().zip(&frames) {
        let start = out.robot.len();
        out.links
            .push((base * *frame).transform_point3(DVec3::ZERO).as_vec3());
        for shape in &link.visuals {
            let triangles = match &shape.geometry {
                Geometry::Box { size } => cuboid(Mat4::from_scale(size.as_vec3())),
                Geometry::Cylinder { radius, length } => cylinder(*radius as f32, *length as f32),
                Geometry::Sphere { radius } => sphere(*radius as f32),
                Geometry::Mesh { scale, mesh, .. } => mesh
                    .triangles
                    .iter()
                    .map(|t| t.map(|p| Vec3::from(p) * scale.as_vec3()))
                    .collect(),
            };
            let rgba = shape.rgba.unwrap_or([0.8, 0.8, 0.8, 1.0]);
            push(
                &mut out.robot,
                (base * *frame * shape.origin).as_mat4(),
                &triangles,
                [rgba[0], rgba[1], rgba[2]].map(linear),
                Material::WhitePaint,
                0.5,
            );
        }
        out.link_ranges.push(start..out.robot.len());
    }
    for object in &scene.objects {
        let pose = step
            .and_then(|s| s.objects.get(&object.id))
            .map_or(&object.geometry.pose, |o| &o.pose);
        let triangles = match &object.geometry.shape {
            robot_lab::document::Shape::Box { size } => {
                cuboid(Mat4::from_scale(Vec3::from_array(size.map(|v| v as f32))))
            }
            robot_lab::document::Shape::Sphere { radius } => sphere(*radius as f32),
            robot_lab::document::Shape::Cylinder { radius, length } => {
                cylinder(*radius as f32, *length as f32)
                    .into_iter()
                    .map(|t| {
                        t.map(|v| {
                            Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2).transform_point3(v)
                        })
                    })
                    .collect()
            }
            robot_lab::document::Shape::Mesh { .. } => {
                return Err("Use primitive scene objects for this viewer".into());
            }
        };
        let rgba = object.geometry.rgba;
        push(
            &mut out.props,
            saved_pose(pose).as_mat4(),
            &triangles,
            [rgba[0], rgba[1], rgba[2]].map(|v| linear(v as f32)),
            Material::WhitePaint,
            0.5,
        );
        if object.id == "cube" {
            out.cube = Vec3::from_array(pose.position.map(|v| v as f32));
        }
    }
    Ok(out)
}

fn saved_pose(pose: &robot_lab::document::Pose) -> DMat4 {
    DMat4::from_rotation_translation(
        DQuat::from_array(pose.rotation),
        DVec3::from_array(pose.position),
    )
}

/// sRGB to linear light.
fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn push(
    out: &mut Vec<LitVertex>,
    place: Mat4,
    triangles: &[[Vec3; 3]],
    color: [f32; 3],
    material: Material,
    roughness: f32,
) {
    let (_, metallic, _) = material.parameters();
    for triangle in triangles {
        let world = triangle.map(|p| place.transform_point3(p));
        let Some(normal) = (world[1] - world[0])
            .cross(world[2] - world[0])
            .try_normalize()
        else {
            continue;
        };
        let tangent = (world[1] - world[0]).normalize();
        for (corner, local) in world.iter().zip(triangle) {
            out.push(LitVertex {
                pos: corner.to_array(),
                normal: normal.to_array(),
                tangent: tangent.to_array(),
                local: local.to_array(),
                color,
                params: [metallic, roughness, material.code(), 1.0],
            });
        }
    }
}

/// The unit cube centered on the origin, mapped by `scale`, wound outward.
fn cuboid(scale: Mat4) -> Vec<[Vec3; 3]> {
    let c = |x: f32, y: f32, z: f32| scale.transform_point3(Vec3::new(x, y, z) * 0.5);
    let quads = [
        // -X, +X, -Y, +Y, -Z, +Z
        [
            c(-1., -1., -1.),
            c(-1., -1., 1.),
            c(-1., 1., 1.),
            c(-1., 1., -1.),
        ],
        [
            c(1., -1., -1.),
            c(1., 1., -1.),
            c(1., 1., 1.),
            c(1., -1., 1.),
        ],
        [
            c(-1., -1., -1.),
            c(1., -1., -1.),
            c(1., -1., 1.),
            c(-1., -1., 1.),
        ],
        [
            c(-1., 1., -1.),
            c(-1., 1., 1.),
            c(1., 1., 1.),
            c(1., 1., -1.),
        ],
        [
            c(-1., -1., -1.),
            c(-1., 1., -1.),
            c(1., 1., -1.),
            c(1., -1., -1.),
        ],
        [
            c(-1., -1., 1.),
            c(1., -1., 1.),
            c(1., 1., 1.),
            c(-1., 1., 1.),
        ],
    ];
    quads
        .iter()
        .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
        .collect()
}

/// A cylinder along Z, centered on the origin.
fn cylinder(radius: f32, length: f32) -> Vec<[Vec3; 3]> {
    let half = length / 2.0;
    let rim = |i: usize, z: f32| {
        let a = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        Vec3::new(a.cos() * radius, a.sin() * radius, z)
    };
    let mut out = Vec::with_capacity(SEGMENTS * 4);
    for i in 0..SEGMENTS {
        let (a0, a1) = (rim(i, -half), rim(i + 1, -half));
        let (b0, b1) = (rim(i, half), rim(i + 1, half));
        out.push([a0, a1, b1]);
        out.push([a0, b1, b0]);
        out.push([Vec3::new(0.0, 0.0, half), b0, b1]);
        out.push([Vec3::new(0.0, 0.0, -half), a1, a0]);
    }
    out
}

/// A sphere centered on the origin.
fn sphere(radius: f32) -> Vec<[Vec3; 3]> {
    let rings = SEGMENTS / 2;
    let point = |i: usize, j: usize| {
        let theta = j as f32 / rings as f32 * std::f32::consts::PI;
        let phi = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        Vec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        ) * radius
    };
    let mut out = Vec::new();
    for j in 0..rings {
        for i in 0..SEGMENTS {
            let (a, b) = (point(i, j), point(i + 1, j));
            let (c, d) = (point(i, j + 1), point(i + 1, j + 1));
            out.push([a, c, d]);
            out.push([a, d, b]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm() -> Robot {
        robot_description::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../robot-description/tests/fixtures/arm/arm.urdf"),
        )
        .unwrap()
    }

    fn so101() -> Robot {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../robot-description/assets/so101");
        robot_description::pin::load_pinned(&path).unwrap().1
    }

    fn heights(vertices: &[LitVertex]) -> (f32, f32) {
        vertices
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v.pos[1]), hi.max(v.pos[1]))
            })
    }

    #[test]
    fn saved_scene_draws_observed_violations_and_object_poses_without_clamping() {
        use robot_lab::document::{
            Geometry, Object, ObjectState, Pose, Reference, RobotInstance, Scene as DocumentScene,
            Shape, Step,
        };
        let robot = so101();
        let joints: std::collections::BTreeMap<_, _> =
            robot.movable().map(|j| (j.name.clone(), 0.0)).collect();
        let scene = DocumentScene {
            name: "Recorded arm".into(),
            robots: vec![RobotInstance {
                id: "arm".into(),
                robot: Reference {
                    document: "arm".into(),
                    version: "a".repeat(64),
                },
                pose: Pose {
                    position: [0.0, 0.75, 0.0],
                    ..Pose::default()
                },
                joints: joints.clone(),
            }],
            objects: vec![Object {
                id: "cube".into(),
                geometry: Geometry {
                    pose: Pose::default(),
                    shape: Shape::Box { size: [0.05; 3] },
                    rgba: [1.0; 4],
                },
                mesh: None,
                collider: Some(Shape::Box { size: [0.05; 3] }),
                mass: 0.05,
                friction: 0.7,
            }],
            lights: vec![],
            cameras: vec![robot_lab::document::Camera {
                id: "front".into(),
                pose: Pose::default(),
                width: 64,
                height: 64,
                intrinsics: [50.0, 50.0, 32.0, 32.0],
                robot_link: None,
            }],
        };
        let mut step = Step {
            index: 0,
            time_seconds: 0.0,
            joints,
            actions: Default::default(),
            observation_step: None,
            objects: Default::default(),
            contacts: vec![],
            sensors: Default::default(),
            inference_ms: None,
            limit_violations: vec![],
        };
        step.joints.insert("shoulder_pan".into(), 2.0);
        step.limit_violations.push("shoulder_pan".into());
        step.objects.insert(
            "cube".into(),
            ObjectState {
                pose: Pose {
                    position: [0.3, 0.775, 0.1],
                    ..Pose::default()
                },
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                held: false,
            },
        );
        let saved = document_scene(&robot, &scene, Some(&step)).unwrap();
        assert_eq!(saved.cube, Vec3::new(0.3, 0.775, 0.1));
        assert!(!saved.robot.is_empty());
        assert!(saved.links.iter().all(|v| v.is_finite()));
        let mut invalid = scene;
        invalid.robots[0].joints = step.joints;
        assert!(
            document_scene(&robot, &invalid, None).is_err(),
            "pose edits still enforce limits"
        );
    }

    #[test]
    fn the_arm_stands_upright_on_the_table_at_meter_scale() {
        let robot = arm();
        let home = scene(&robot, &robot.home()).unwrap();
        let (low, high) = heights(&home.robot);
        // The base mesh sits on the table top; the jaw tops out 0.47 m up.
        assert!((low - TABLE_HEIGHT).abs() < 1e-4, "{low}");
        assert!((high - (TABLE_HEIGHT + 0.47)).abs() < 1e-4, "{high}");
        // The floor is below zero; the cube on the table is the props' top.
        let (floor, top) = heights(&home.props);
        assert!(floor < 0.0);
        assert!((top - (TABLE_HEIGHT + CUBE_SIDE)).abs() < 1e-4, "{top}");
        assert!((home.cube.y - CUBE_SIDE / 2.0 - TABLE_HEIGHT).abs() < 1e-6);
        // Lifting the shoulder a quarter turn reaches toward the cube (+X).
        let lifted = robot
            .named_pose(&[("shoulder_lift", std::f64::consts::FRAC_PI_2)])
            .unwrap();
        let reach = scene(&robot, &lifted).unwrap();
        let gripper = robot.link("gripper").unwrap();
        let at = reach.links[gripper];
        assert!(
            (at - Vec3::new(0.29, TABLE_HEIGHT + 0.1, 0.0)).length() < 1e-4,
            "{at}"
        );
    }

    #[test]
    fn a_pose_beyond_the_limits_is_refused() {
        let robot = arm();
        let error = robot.named_pose(&[("elbow_flex", 3.0)]).unwrap_err();
        assert!(error.to_string().contains("outside its limits"));
        let mut pose = robot.home();
        pose[2] = 3.0;
        assert!(scene(&robot, &pose).is_err());
    }

    #[test]
    fn the_pinned_so101_renders_in_the_table_scene() {
        let robot = so101();
        let frame = scene(&robot, &robot.home()).unwrap();
        assert_eq!(frame.base.y, TABLE_HEIGHT);
        assert!((frame.cube.y - CUBE_SIDE / 2.0 - TABLE_HEIGHT).abs() < 1e-6);
        assert!(!frame.robot.is_empty());
        let (low, high) = heights(&frame.robot);
        assert!(
            frame
                .robot
                .iter()
                .all(|vertex| vertex.pos.iter().all(|value| value.is_finite()))
        );
        assert!(
            low >= TABLE_HEIGHT - 0.2 && high < TABLE_HEIGHT + 1.0,
            "robot Y extent: {low}..{high}"
        );
    }
}
