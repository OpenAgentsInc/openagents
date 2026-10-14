//! Save policy settings for a named scene without starting execution.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 9 {
        return Err("Use: STORE PROJECT SCENE@VERSION DOCUMENT LIBRARY LIBRARY_SHA POLICY_FOLDER POLICY_SHA KIND".into());
    }
    let store = robot_lab::Store::open(Path::new(&args[0]), &args[1])?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = robot_workbench::host::Host::open(store, &catalog, "http://127.0.0.1:8794")?;
    let (document, version) = args[2]
        .split_once('@')
        .ok_or("Choose the exact saved scene")?;
    let scene = robot_lab::Reference {
        document: document.into(),
        version: version.into(),
    };
    let config = robot_workbench::experiments::Config {
        library: robot_sim::mujoco::LibraryPin {
            path: args[4].clone().into(),
            sha256: args[5].clone(),
        },
        policy: Some(robot_workbench::experiments::Policy {
            id: args[8].clone(),
            root: args[6].clone().into(),
            sha256: args[7].clone(),
        }),
    };
    let saved = robot_workbench::experiments::policy_experiment(
        &host,
        &scene,
        (11..=20).collect(),
        241,
        &config,
        &args[3],
    )?;
    println!("{}", serde_json::to_string(&saved)?);
    Ok(())
}
