//! Run one explicit private study, keeping held-out cases unspent.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if !(9..=10).contains(&args.len()) {
        return Err("Use: STORE PROJECT SCENE@VERSION STUDY LIBRARY LIBRARY_SHA POLICY_FOLDER POLICY_SHA OUTPUT_FOLDER".into());
    }
    let store = robot_lab::Store::open(Path::new(&args[0]), &args[1])?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = robot_workbench::host::Host::open(store, &catalog, "http://127.0.0.1:8794")?;
    let (document, version) = args[2].split_once('@').ok_or("Choose the exact scene")?;
    let reference = robot_lab::Reference {
        document: document.into(),
        version: version.into(),
    };
    let config = robot_workbench::experiments::Config {
        library: robot_sim::mujoco::LibraryPin {
            path: args[4].clone().into(),
            sha256: args[5].clone(),
        },
        policy: Some(robot_workbench::experiments::Policy {
            id: args.get(9).cloned().unwrap_or_else(|| "act".into()),
            root: args[6].clone().into(),
            sha256: args[7].clone(),
        }),
    };
    let result = robot_workbench::experiments::study(
        &host,
        &reference,
        &args[3],
        &config,
        Path::new(&args[8]),
    )?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
