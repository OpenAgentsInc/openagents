//! Robot Lab inspection capture (`docs/lab/README.md`, slice 1).
//! Usage: robot_lab_capture OUTPUT.png [--urdf FILE.urdf] [JOINT=RADIANS ...]
//! Add --dataset FOLDER --mapping FILE --episode N --frame N for recorded poses.
//!
//! Loads the pinned SO-101 URDF by default, sets the named joints
//! (the rest stay at home), refuses values outside the joints' limits,
//! stands the robot on a 0.75 m table beside a 5 cm red cube, and renders a
//! 1280×800 frame with the shared renderer. It prints each link's position.
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use verse::mesh::Mesh;
use verse::pbr::{Key, Neon};
use verse::render::View;
use verse::robot_lab::{self, TABLE_HEIGHT};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("Usage: robot_lab_capture OUTPUT.png [--urdf FILE.urdf] [JOINT=RADIANS ...]")?,
    );
    let mut urdf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../robot-description/assets/so101/so101_new_calib.urdf");
    let mut joints = Vec::new();
    let mut dataset = None;
    let mut mapping = None;
    let mut episode = 0_u64;
    let mut frame = 0_usize;
    let mut lab_store = None;
    let mut project = None;
    let mut saved_episode = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--lab-store" => {
                lab_store = Some(PathBuf::from(
                    args.next().ok_or("--lab-store needs a folder")?,
                ));
                continue;
            }
            "--project" => {
                project = Some(args.next().ok_or("--project needs a name")?);
                continue;
            }
            "--saved-episode" => {
                saved_episode = Some(
                    args.next()
                        .ok_or("--saved-episode needs DOCUMENT@VERSION")?,
                );
                continue;
            }
            "--dataset" => {
                dataset = Some(PathBuf::from(
                    args.next().ok_or("--dataset needs a folder")?,
                ));
                continue;
            }
            "--mapping" => {
                mapping = Some(PathBuf::from(args.next().ok_or("--mapping needs a file")?));
                continue;
            }
            "--episode" => {
                episode = args
                    .next()
                    .ok_or("--episode needs a number")?
                    .parse()
                    .map_err(|_| "Choose a nonnegative episode number")?;
                continue;
            }
            "--frame" => {
                frame = args
                    .next()
                    .ok_or("--frame needs a number")?
                    .parse()
                    .map_err(|_| "Choose a nonnegative frame number")?;
                continue;
            }
            _ => {}
        }
        if arg == "--urdf" {
            urdf = PathBuf::from(args.next().ok_or("--urdf needs a file")?);
            continue;
        }
        let (name, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("{arg} is not JOINT=RADIANS"))?;
        let value: f64 = value
            .parse()
            .map_err(|_| format!("{value} is not a number of radians"))?;
        joints.push((name.to_string(), value));
    }
    let robot = robot_description::load(&urdf).map_err(|e| e.to_string())?;
    if saved_episode.is_some() && (dataset.is_some() || mapping.is_some() || !joints.is_empty()) {
        return Err("Choose a saved attempt, recorded demonstration, or joint edits".into());
    }
    match (dataset, mapping) {
        (Some(dataset), Some(mapping)) => {
            if !joints.is_empty() {
                return Err("Choose a recorded pose or joint edits for this capture".into());
            }
            let hash = robot_description::pin::sha256_hex(
                &std::fs::read(&urdf).map_err(|e| e.to_string())?,
            );
            joints = recorded_joints(&dataset, &mapping, &hash, episode, frame)?;
        }
        (None, None) => {}
        _ => return Err("Recorded poses need both --dataset and --mapping".into()),
    }
    let named: Vec<(&str, f64)> = joints.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    let (pose, scene) = if let Some(reference) = saved_episode {
        let (scene, step) = saved_frame(
            lab_store
                .as_deref()
                .ok_or("Choose --lab-store for this saved attempt")?,
            project
                .as_deref()
                .ok_or("Choose --project for this saved attempt")?,
            &reference,
            &urdf,
            frame,
        )?;
        let pose = robot
            .movable()
            .map(|j| {
                step.joints
                    .get(&j.name)
                    .copied()
                    .ok_or("This attempt is missing a robot joint")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let drawn = robot_lab::document_scene(&robot, &scene, Some(&step))?;
        (pose, drawn)
    } else {
        if lab_store.is_some() || project.is_some() {
            return Err("Choose --saved-episode with this lab folder".into());
        }
        let pose = robot.named_pose(&named).map_err(|e| e.to_string())?;
        let drawn = robot_lab::scene(&robot, &pose).map_err(|e| e.to_string())?;
        (pose, drawn)
    };
    eprintln!("{} at {}", robot.name(), urdf.display());
    for (joint, value) in robot.movable().zip(&pose) {
        let limit = joint.limit.map_or(String::new(), |l| {
            format!(" in [{:.3}, {:.3}]", l.lower, l.upper)
        });
        eprintln!("  joint {} = {value:.3}{limit}", joint.name);
    }
    for (link, at) in robot.links().iter().zip(&scene.links) {
        eprintln!("  link {} at {:.3?} m", link.name, at.to_array());
    }

    let mut lit = scene.props;
    lit.extend(scene.robot);
    let world = Mesh {
        lit,
        ..Mesh::default()
    };
    let center = Vec3::new(0.1, TABLE_HEIGHT + 0.15, 0.0);
    let mut neon = Neon::neutral(0.0);
    neon.key = Some(Key {
        dir: Vec3::new(0.45, 0.8, 0.4).normalize(),
        illuminance: 4_200.0,
        angular_radius: 0.035,
        rim_dir: Vec3::new(-0.5, 0.4, -0.75).normalize(),
        rim_illuminance: 1_800.0,
        rim_angular_radius: 0.12,
        sky: 420.0,
        ground: 60.0,
        ev100: 10.0,
        shadow_center: center,
        shadow_half: 1.5,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    let dynamic = Mesh {
        neon: Some(neon),
        ..Mesh::default()
    };
    let (width, height) = (1280, 800);
    let eye = center + Vec3::new(0.55, 0.35, 1.05);
    let view = View {
        view_proj: Mat4::perspective_rh(0.75, width as f32 / height as f32, 0.05, 50.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
        eye,
    };
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &world,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

fn saved_frame(
    root: &Path,
    project: &str,
    reference: &str,
    urdf: &Path,
    frame: usize,
) -> Result<(::robot_lab::document::Scene, ::robot_lab::document::Step), String> {
    use ::robot_lab::{Document, Reference, Store};
    let (document, version) = reference
        .split_once('@')
        .ok_or("Use DOCUMENT@VERSION for a saved attempt")?;
    let store = Store::open(root, project).map_err(|e| e.to_string())?;
    let value = store
        .get(&Reference {
            document: document.into(),
            version: version.into(),
        })
        .map_err(|e| e.to_string())?;
    store
        .validate_dependencies(&value.content)
        .map_err(|e| e.to_string())?;
    let Document::Episode(episode) = value.content else {
        return Err("Choose a saved attempt".into());
    };
    let Document::Experiment(experiment) = store
        .get(&episode.experiment)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("This attempt is missing its experiment".into());
    };
    let Document::Scene(scene) = store
        .get(&experiment.scene)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("This attempt is missing its scene".into());
    };
    if scene.robots.len() != 1 {
        return Err("Open an attempt with one robot".into());
    }
    let Document::Robot(robot) = store
        .get(&scene.robots[0].robot)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("This attempt is missing its robot".into());
    };
    let description = robot
        .source
        .files
        .get(&robot.description)
        .ok_or("This attempt is missing its robot description")?;
    if robot_description::pin::sha256_hex(&std::fs::read(urdf).map_err(|e| e.to_string())?)
        != description.sha256
    {
        return Err("Choose the robot description saved with this attempt".into());
    }
    let root = urdf.parent().ok_or("Choose the saved robot folder")?;
    for (name, asset) in &robot.source.files {
        let path = root.join(name);
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.len() != asset.bytes
            || metadata.len() > robot_description::MAX_MESH_BYTES
        {
            return Err("Choose the unchanged robot files saved with this attempt".into());
        }
        if robot_description::pin::sha256_hex(&std::fs::read(path).map_err(|e| e.to_string())?)
            != asset.sha256
        {
            return Err("Choose the unchanged robot files saved with this attempt".into());
        }
    }
    let step = episode
        .steps
        .get(frame)
        .ok_or("Choose a frame in this saved attempt")?
        .clone();
    Ok((scene, step))
}

fn recorded_joints(
    dataset: &Path,
    mapping: &Path,
    robot_hash: &str,
    episode: u64,
    frame: usize,
) -> Result<Vec<(String, f64)>, String> {
    let metadata = std::fs::metadata(mapping).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 65_536 {
        return Err("Choose a joint mapping smaller than 64 KB".into());
    }
    let mapping: robot_data::mapping::Mapping =
        serde_json::from_slice(&std::fs::read(mapping).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if mapping.robot_version != robot_hash {
        return Err("Choose a mapping for this robot description".into());
    }
    let dataset = robot_data::dataset::Dataset::open(dataset).map_err(|e| e.to_string())?;
    let episode = dataset.episode(episode).map_err(|e| e.to_string())?;
    mapping
        .validate(&episode.robot_type, &episode.state_features)
        .map_err(|e| e.to_string())?;
    let frame = episode
        .frames
        .get(frame)
        .ok_or("Choose a frame in this episode")?;
    mapping
        .convert(&episode.state_features, &frame.state)
        .map(|joints| joints.into_iter().collect())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_pose_uses_the_named_frame_and_exact_robot_mapping() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let data = root.join("../robot-data/tests/fixtures/v3.0");
        let mapping = root.join("../robot-data/tests/fixtures/so101-view-mapping.json");
        let urdf = root.join("../robot-description/assets/so101/so101_new_calib.urdf");
        let hash = robot_description::pin::sha256_hex(&std::fs::read(&urdf).unwrap());
        let robot = robot_description::load(&urdf).unwrap();
        let joints = recorded_joints(&data, &mapping, &hash, 1, 3).unwrap();
        let values: Vec<_> = joints
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        let pose = robot.named_pose(&values).unwrap();
        let scene = robot_lab::scene(&robot, &pose).unwrap();
        assert_eq!(joints.len(), 6);
        assert!(scene.links.iter().all(|point| point.is_finite()));
        assert!(!scene.robot.is_empty());
        assert!(recorded_joints(&data, &mapping, &"0".repeat(64), 1, 3).is_err());
        assert!(recorded_joints(&data, &mapping, &hash, 1, 4).is_err());
    }
}
