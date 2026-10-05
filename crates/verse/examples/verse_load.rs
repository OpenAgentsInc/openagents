//! Bounded authenticated movement and combat load using the native replication worker.
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        wire::{Reply, State},
        worker::{self, Input, Update},
    },
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    address: SocketAddr,
    server_name: String,
    instance: u64,
    trust_der: PathBuf,
    pack: PathBuf,
    scene: PathBuf,
    dir: PathBuf,
    keys: Vec<PathBuf>,
    seconds: u32,
    output: PathBuf,
    #[serde(default)]
    movement_frames: bool,
}
fn bounded(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "Cannot open remote configuration input")?;
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect remote configuration input")?
        .is_file()
    {
        return Err("Remote configuration input must be a regular file".into());
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read remote configuration input")?;
    if bytes.len() > limit {
        return Err("Remote configuration input exceeds its size bound".into());
    }
    Ok(bytes)
}

fn sample(values: &mut Vec<f64>, value: f64) -> bool {
    if values.len() < 8192 {
        values.push(value);
        true
    } else {
        false
    }
}
fn summary(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    let p95 =
        (!values.is_empty()).then(|| values[((values.len() - 1) as f64 * 0.95).ceil() as usize]);
    serde_json::json!({"samples":values.len(),"p95":p95,"max":values.last()})
}
async fn player(
    client: Client,
    index: usize,
    end: tokio::time::Instant,
    movement_frames: bool,
) -> Result<serde_json::Value, String> {
    let instance = client.instance();
    let (send, inputs, updates, mut receive) = worker::channels();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(worker::run(
        client,
        Cursor::new(instance),
        worker::NATIVE_CADENCE,
        inputs,
        updates,
        stopped,
    ));
    let mut clock = tokio::time::interval(Duration::from_millis(33));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut state: Option<State> = None;
    let mut token = 0u64;
    let mut epoch = 0;
    let mut frame_cursor = None;
    let mut frame_entry = None;
    let mut observed_frame_snapshots = 0u64;
    let mut pending = VecDeque::new();
    let mut outstanding = BTreeSet::new();
    let mut latency = Vec::new();
    let mut omitted_latency = 0u64;
    let mut casts = BTreeMap::<String, u64>::new();
    let mut refused = 0;
    let mut refusal_trace = Vec::new();
    let mut snapshots = 0;
    let mut snapshot_bytes = 0u64;
    let mut movement = 0;
    let mut pressure = 0;
    let mut next_cast = tokio::time::Instant::now() + Duration::from_millis(index as u64 * 75);
    let mut slot = 0;
    let mut respawned = None;
    let mut min_hp = i32::MAX;
    let mut max_actors = 0;
    let mut max_players = 0;
    let mut max_live_hostiles = 0;
    let mut battle_samples = 0u64;
    let mut battle_live_total = 0u64;
    let mut battle_live_min = usize::MAX;
    let began = tokio::time::Instant::now();
    let result=async {
  loop {
   tokio::select! {
    _=tokio::time::sleep_until(end)=>break,
    _=clock.tick()=>{
     let Some(state)=state.as_ref() else {continue};
     let Some(hud)=state.hud.as_ref() else {return Err("Load principal has no player HUD".into())};
     min_hp=min_hp.min(hud.resources.hp);
     if hud.resources.hp<=0 {
      if respawned!=Some(hud.life) && send.try_send(Input::Respawn).is_ok() {respawned=Some(hud.life);}
      continue;
     }
     if state.presentation.time<20. {continue;}
     let seconds=began.elapsed().as_secs_f64();
     let phase=((seconds+index as f64*0.2).rem_euclid(8.)).floor() as u32;
     let axes=match phase {0=>[0.,1.],1=>[1.,0.],2=>[0.,-1.],3=>[-1.,0.],_=>[0.,0.]};
     let mut intent=Intent::Move {axes,yaw:std::f32::consts::PI};
     if tokio::time::Instant::now()>=next_cast && hud.casting.is_none() {
      let order=[Ability::Shield,Ability::Fireball,Ability::Web,Ability::Grease,Ability::Light,Ability::Thunderwave,Ability::MistyStep,Ability::Bow,Ability::FireBolt,Ability::MagicMissile];
      let ability=order[slot%order.len()];slot+=1;next_cast=tokio::time::Instant::now()+Duration::from_secs(2);
      if hud.slots.iter().any(|s|s.ability==ability && s.ready) {
       let owned=state.presentation.actors.iter().find(|p|verse_engine::core::LifeId::from(p.life)==hud.life).map(|p|p.actor.position).unwrap_or_default();
       let target=state.presentation.actors.iter().filter(|p|p.health>0 && p.visible && !p.actor.friendly && p.actor.nameplate && p.actor.model!="adventurer").min_by(|a,b|a.actor.position.distance_squared(owned).total_cmp(&b.actor.position.distance_squared(owned)));
       let aim=target.map(|p|{let mut direction=p.actor.position-owned;direction.y=0.;direction.normalize_or_zero().to_array()}).unwrap_or([0.,0.,1.]);
       intent=Intent::Cast {ability,target:target.map(|p|p.life.into()),aim};
      }
     }
     token=token.checked_add(1).ok_or("Load input identities exhausted")?;
     let moving=matches!(intent,Intent::Move {..});
     let input=if movement_frames && moving {
      let Some(baseline)=state.movement else {continue};
      let context=(baseline.life,baseline.epoch);
      if baseline.profile!=verse_world::movement::Profile::Frames {
       frame_cursor=None;
       if baseline.character.support.is_some() && baseline.held.axes(baseline.physics_step)==[0.;2] && frame_entry!=Some(context) && outstanding.is_empty() && pending.is_empty() {
        match send.try_send(Input::BeginMovementFrames {life:baseline.life,epoch:baseline.epoch}) {
         Ok(())=>{frame_entry=Some(context);pending.push_back((tokio::time::Instant::now(),None,Some(context),None));},
         Err(tokio::sync::mpsc::error::TrySendError::Full(_))=>pressure+=1,
         Err(_)=>return Err("Load worker input closed".into()),
        }
       }
       continue;
      }
      let start=match frame_cursor {Some((life,old_epoch,start)) if (life,old_epoch)==context=>start,_=>baseline.physics_step};
      let limit=baseline.world_step.checked_add(u64::from(verse_world::movement::frames::MAX_STEPS)).ok_or("Load movement credit exhausted")?;
      let steps=limit.saturating_sub(start).min(u64::from(verse_world::movement::frames::MAX_STEPS)) as u32;
      if steps<4 {continue;}
      let frame=verse_world::movement::frames::Frame {life:baseline.life,epoch:baseline.epoch,sequence:0,tick:0,start,steps,
       segments:vec![verse_world::movement::frames::Segment {offset:0,axes,yaw:std::f32::consts::PI,until:start+verse_world::movement::HELD_STEPS,jump:false}]};
      frame.validate_payload()?;
      Input::MovementFrame {token,frame}
     } else {Input::TrackedCommand {token,life:hud.life,epoch,intent}};
     let proposed_end=match &input {Input::MovementFrame {frame,..}=>Some((frame.life,frame.epoch,frame.end()?)),_=>None};
     match send.try_send(input) {
      Ok(())=>{outstanding.insert(token);if let Some(cursor)=proposed_end {frame_cursor=Some(cursor);} if moving {movement+=1;}},
      Err(tokio::sync::mpsc::error::TrySendError::Full(_))=>pressure+=1,
      Err(_)=>return Err("Load worker input closed".into()),
     }
    }
    update=receive.recv()=>match update.ok_or("Load worker output closed")? {
     Update::Snapshot(response)=>{
      epoch=response.control.as_ref().ok_or("Load lost player control")?.epoch;
      snapshot_bytes+=serde_json::to_vec(&response).map_err(|_|"Cannot size load snapshot")?.len() as u64;
      let Reply::Snapshot {state:latest}=response.body else {return Err("Load snapshot refused".into())};
      if latest.movement.is_some_and(|b|b.profile==verse_world::movement::Profile::Frames) {observed_frame_snapshots+=1;}
      snapshots+=1;max_actors=max_actors.max(latest.presentation.actors.len());
      max_players=max_players.max(latest.presentation.actors.iter().filter(|p|p.actor.model=="adventurer").count());
      max_live_hostiles=max_live_hostiles.max(latest.presentation.actors.iter().filter(|p|p.health>0 && !p.actor.friendly && p.actor.model!="adventurer").count());
      if latest.presentation.time>=20. {
       let live=latest.presentation.actors.iter().filter(|p|p.health>0 && !p.actor.friendly && p.actor.model!="adventurer").count();
       battle_samples+=1;battle_live_total+=live as u64;battle_live_min=battle_live_min.min(live);
      }
      state=Some(latest);
     }
     Update::CommandBound {token,binding}=>match binding {
      Ok(command)=>{let ability=match command.intent {Intent::Cast {ability,..}=>Some(ability.label().to_string()),_=>None};pending.push_back((tokio::time::Instant::now(),ability,None,Some(token)));},
      Err(message)=>{outstanding.remove(&token);refused+=1;if refusal_trace.len()<32 {refusal_trace.push(serde_json::json!({"stage":"command_binding","message":message}));}},
     },
     Update::FrameBound {token,binding}=>match binding {
      Ok(_)=>pending.push_back((tokio::time::Instant::now(),None,None,Some(token))),
      Err(message)=>{outstanding.remove(&token);refused+=1;if refusal_trace.len()<32 {refusal_trace.push(serde_json::json!({"stage":"frame_binding","message":message}));}},
     },
     Update::Outcome(response)=>{
      if let Some((started,ability,entry,token))=pending.pop_front() {
       if let Some(token)=token {outstanding.remove(&token);}
       if entry.is_some() && frame_entry==entry && matches!(&response.body,Reply::Refused {..}) {frame_entry=None;}
       if !sample(&mut latency,started.elapsed().as_secs_f64()*1000.) {omitted_latency+=1;}
       match response.body {Reply::Accepted=>{if let Some(ability)=ability {*casts.entry(ability).or_default()+=1;}},Reply::Refused {message,code,..}=>{refused+=1;if refusal_trace.len()<32 {refusal_trace.push(serde_json::json!({"stage":"outcome","message":message,"code":code,"tick":response.tick,"control":response.control}));}},_=>{}}
      }
     }
     Update::MovementSuperseded {token,..}=>{outstanding.remove(&token);},
     Update::Events {..}|Update::Inventory(_)=>{},
    }
   }
  }
  Ok::<(),String>(())
 }.await;
    let _ = stop.send(());
    let worker_result = task.await;
    let observation_error = result
        .as_ref()
        .err()
        .map(|e| e.chars().take(2048).collect::<String>());
    let worker_error = match &worker_result {
        Err(e) => Some(e.to_string()),
        Ok(Err(e)) => Some(e.clone()),
        Ok(Ok(())) => None,
    }
    .map(|e| e.chars().take(2048).collect::<String>());
    let failure_stage = if result.is_err() {
        Some("observation")
    } else {
        match worker_result {
            Err(_) => Some("worker_task"),
            Ok(Err(_)) => Some("worker_transport"),
            Ok(Ok(())) => None,
        }
    };
    Ok(
        serde_json::json!({"player":index,"status":if failure_stage.is_some() {"failed"} else {"complete"},"failure_stage":failure_stage,"observation_error":observation_error,"worker_error":worker_error,"refusal_trace":refusal_trace,"omitted_latency_samples":omitted_latency,"snapshots":snapshots,"snapshot_bytes":snapshot_bytes,"maximum_actors":max_actors,"maximum_players":max_players,"maximum_live_hostiles":max_live_hostiles,"battle_occupancy":{"samples":battle_samples,"minimum_live_hostiles":(battle_samples>0).then_some(battle_live_min),"mean_live_hostiles":(battle_samples>0).then(||battle_live_total as f64/battle_samples as f64)},"movement_profile":if movement_frames {"authority_credit_intervals"} else {"legacy_commands"},"observed_frame_snapshots":observed_frame_snapshots,"movement_inputs":movement,"input_pressure":pressure,"refusals":refused,"accepted_casts":casts,"minimum_hp":(min_hp!=i32::MAX).then_some(min_hp),"binding_to_outcome_ms":summary(latency)}),
    )
}
async fn run(config: Config) -> Result<(), String> {
    if !(1..=20).contains(&config.keys.len()) || !(1..=120).contains(&config.seconds) {
        return Err("Load count or duration exceeds bounds".into());
    }
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let content = verse::imported::remote_content::identity(&pack, &scene, &config.dir)?;
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(bounded(
            &config.trust_der,
            1024 * 1024,
        )?))
        .map_err(|_| "Invalid load trust certificate")?;
    let tls = Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|_| "Invalid load TLS versions")?
        .with_root_certificates(roots)
        .with_no_client_auth(),
    );
    let name = rustls::pki_types::ServerName::try_from(config.server_name)
        .map_err(|_| "Invalid load server name")?;
    let mut clients = Vec::new();
    for path in config.keys {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&path)
                .map_err(|_| "Cannot inspect load key")?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Load signing key must have owner-only permissions".into());
            }
        }
        let bytes = bounded(&path, 256)?;
        let key: secp256k1::SecretKey = std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid load key")?
            .trim()
            .parse()
            .map_err(|_| "Invalid load key")?;
        let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &key);
        clients.push(
            Client::connect_with_content(
                config.address,
                name.clone(),
                tls.clone(),
                config.instance,
                Some(content),
                &key,
            )
            .await?,
        );
    }
    println!("Load ready: {} authenticated players", clients.len());
    let end = tokio::time::Instant::now() + Duration::from_secs(config.seconds as u64);
    let mut tasks = tokio::task::JoinSet::new();
    for (index, client) in clients.into_iter().enumerate() {
        tasks.spawn(player(client, index, end, config.movement_frames));
    }
    let mut rows = Vec::new();
    while let Some(result) = tasks.join_next().await {
        rows.push(match result {
            Ok(Ok(row)) => row,
            Ok(Err(_)) => {
                serde_json::json!({"player":null,"status":"failed","failure_stage":"player"})
            }
            Err(_) => {
                serde_json::json!({"player":null,"status":"failed","failure_stage":"player_task"})
            }
        });
    }
    rows.sort_by_key(|r| r["player"].as_u64());
    let failed = rows.iter().any(|row| row["status"] != "complete");
    let receipt = serde_json::json!({"schema":"verse.multiplayer.load.v3","movement_frames_requested":config.movement_frames,"status":if failed {"failed"} else {"complete"},"seconds":config.seconds,"players":rows,"limits":["Headless authenticated clients measure transport and authority load, not rendering.","Headless intervals use confirmed server time plus the existing bounded authority lookahead, without native prediction or rendering; the receipt declares the requested movement mode.","Binding-to-outcome includes server processing and client delivery, not isolated RTT.","Timing retains at most 8192 samples per player."]});
    std::fs::write(
        config.output,
        serde_json::to_vec_pretty(&receipt).map_err(|_| "Cannot encode load receipt")?,
    )
    .map_err(|_| "Cannot write load receipt")?;
    if failed {
        return Err("Load failed; partial player measurements retained in the receipt".into());
    }
    Ok(())
}
fn main() {
    let result = (|| {
        let mut args = std::env::args_os().skip(1);
        let path = args.next().ok_or("Usage: verse_load CONFIG.json")?;
        if args.next().is_some() {
            return Err("Usage: verse_load CONFIG.json".into());
        }
        let config: Config =
            serde_json::from_slice(&bounded(std::path::Path::new(&path), 64 * 1024)?)
                .map_err(|_| "Invalid load configuration")?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot create load runtime")?
            .block_on(run(config))
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
