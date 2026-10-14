//! Exercise room controls against an isolated workbench and private store.
use robot_lab::{Reference, Store};
use robot_sim::{
    jobs::{Book, Status},
    mujoco::LibraryPin,
};
use robot_workbench::{
    experiments::{self, Config},
    host::Host,
    runs::{Action, Runs},
    server::Server,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use verse::zones::{
    Intent,
    robotlab::{Room, local::Config as RoomConfig},
};
fn wait(room: &mut Room, predicate: impl Fn(&Room) -> bool) -> Result<(), String> {
    let started = Instant::now();
    while !predicate(room) && started.elapsed() < Duration::from_secs(30) {
        room.tick(0.05);
        if let Some(error) = room.snapshot().error {
            return Err(error);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !predicate(room) {
        return Err("The room control didn't finish in time".into());
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("Use: PRIVATE_ROOT LIBRARY LIBRARY_SHA".into());
    }
    let store = Store::open(Path::new(&args[0]), "room-controls")?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let runtime = tokio::runtime::Runtime::new()?;
    let listener = runtime.block_on(tokio::net::TcpListener::bind(("127.0.0.1", 0)))?;
    let base = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let mut host = Host::open(store.clone(), &catalog, &base)?;
    let config = Config {
        library: LibraryPin {
            path: args[1].clone().into(),
            sha256: args[2].clone(),
        },
        policy: None,
    };
    let imported=host.execute("robot_import",serde_json::json!({"document":"table","source":"so101","revision":"a758567c3978dfeefe282ede0500085a48fe8f78"}),1)?;
    let scene: Reference = serde_json::from_value(imported["card"]["reference"].clone())?;
    let mut pose = host.bundle(&scene)?.scene.robots[0].joints.clone();
    pose.insert("shoulder_pan".into(), 0.2);
    experiments::scripted_experiment(
        &host,
        &scene,
        vec![7],
        241,
        vec![pose],
        1,
        &config,
        "room-experiment",
    )?;
    host.runs = Some(Arc::new(Runs::new(config)));
    let server = Arc::new(Server::new(host.clone(), Path::new(&args[0])));
    let task = runtime.spawn(async move { axum::serve(listener, server.router()).await });
    let room_config = RoomConfig {
        root: args[0].clone().into(),
        project: "room-controls".into(),
        subject: Some(scene),
        control_url: Some(base),
        watch_job: None,
    };
    let mut room = Room::local(&room_config)?;
    room.intent(Intent::Grab)?;
    wait(&mut room, |r| r.snapshot().mode == "Simulation")?;
    assert_eq!(room.snapshot().frame, 0);
    let original_job = room.snapshot().job.unwrap();
    room.intent(Intent::Step)?;
    wait(&mut room, |r| r.snapshot().frame == 1)?;
    room.intent(Intent::Reset)?;
    wait(&mut room, |r| {
        r.snapshot().job.as_deref() != Some(&original_job) && r.snapshot().frame == 0
    })?;
    let reset_job = room.snapshot().job.unwrap();
    assert!(
        Book::open(store.clone())?
            .get(&original_job)?
            .episode
            .is_some()
    );
    room.intent(Intent::Release)?;
    wait(&mut room, |r| r.snapshot().job.is_none())?;
    assert_eq!(room.snapshot().mode, "Saved simulation");
    assert!(
        Book::open(store.clone())?
            .get(&reset_job)?
            .episode
            .is_some()
    );
    let mut room = Room::local(&room_config)?;
    room.intent(Intent::Grab)?;
    wait(&mut room, |r| r.snapshot().mode == "Simulation")?;
    room.intent(Intent::Pause)?;
    wait(&mut room, |r| r.snapshot().frame > 0)?;
    let running_job = room.snapshot().job.unwrap();
    drop(room);
    let started = Instant::now();
    while Book::open(store.clone())?.get(&running_job)?.status != Status::Paused
        && started.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        Book::open(store.clone())?.get(&running_job)?.status,
        Status::Paused
    );
    host.runs
        .as_ref()
        .unwrap()
        .control(&running_job, Action::Stop)?;
    task.abort();
    println!(
        "{}",
        serde_json::json!({"step":1,"reset_preserved":original_job,"stopped_attempt":reset_job,"left_interactive_paused":running_job})
    );
    Ok(())
}
