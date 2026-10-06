//! Shared battle clients use the production duplex worker and control fences.
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Duration,
};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        wire::{Reply, State},
        worker::{self, Input, Observer, Update},
    },
};
#[derive(Clone, serde::Serialize)]
struct ProducerObservation {
    elapsed_ms: u128,
    actor: u64,
    epoch: u64,
    profile: verse_world::movement::Profile,
    confirmed_step: u64,
    snapshot_world_step: u64,
    verified_credit: Option<u64>,
    cursor: Option<u64>,
    outstanding: usize,
    pending: usize,
    input_depth: usize,
}
#[derive(Clone, serde::Serialize)]
struct FrameTrace {
    elapsed_ms: u128,
    phase: &'static str,
    actor: u64,
    epoch: u64,
    sequence: u64,
    start: u64,
    end: u64,
    authority_tick: u64,
    control_epoch: Option<u64>,
    credit_step: Option<u64>,
    pending_requests: usize,
    queued_inputs: usize,
}
#[derive(serde::Serialize)]
struct ProducerTransition {
    before: VecDeque<ProducerObservation>,
    after: ProducerObservation,
    frames: VecDeque<FrameTrace>,
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
// Verified credit wakes the interval producer without granting elapsed wall time.
fn credit_wakes_frames(
    previous: Option<(verse_engine::core::LifeId, u64, u64)>,
    latest: Option<(verse_engine::core::LifeId, u64, u64)>,
    movement: Option<verse_world::movement::Baseline>,
) -> bool {
    let (Some((life, epoch, step)), Some(baseline)) = (latest, movement) else {
        return false;
    };
    baseline.profile == verse_world::movement::Profile::Frames
        && (baseline.life, baseline.epoch) == (life, epoch)
        && previous.is_none_or(|(old_life, old_epoch, old_step)| {
            (old_life, old_epoch) != (life, epoch) || step > old_step
        })
}
pub async fn player(
    client: Client,
    index: usize,
    end: tokio::time::Instant,
    movement_frames: bool,
    mixed: bool,
) -> Result<serde_json::Value, String> {
    let instance = client.instance();
    let mut credit = client
        .control()
        .map(|c| (c.life.into(), c.epoch, c.credit_step));
    let (send, inputs, updates, mut receive) = worker::channels();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let observer = Observer::default();
    let task = tokio::spawn(worker::run_profiled(
        client,
        Cursor::new(instance),
        worker::NATIVE_CADENCE,
        inputs,
        updates,
        stopped,
        observer.clone(),
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
    let mut operation = 0u64;
    let mut next_operation = tokio::time::Instant::now();
    let mut bound_frames = 0u64;
    let mut battle_framed = 0u64;
    let mut confirmed = None;
    let mut confirmed_steps = 0u64;
    let mut operations = BTreeMap::<String, u64>::new();
    let mut turnaround = Vec::new();
    let mut omitted_turnaround = 0u64;
    let mut by_kind = BTreeMap::<String, Vec<f64>>::new();
    let mut omitted_by_kind = BTreeMap::<String, u64>::new();
    let mut peak_pending = 0usize;
    let mut peak_inputs = 0usize;
    let mut omitted_observer = 0u64;
    let mut snapshot_age = Vec::new();
    let mut omitted_age = 0u64;
    let mut refused = 0u64;
    let mut refusal_trace = Vec::new();
    let mut snapshots = 0;
    let mut snapshot_bytes = 0u64;
    let mut movement = 0;
    let mut pressure = 0;
    let mut next_cast = tokio::time::Instant::now() + Duration::from_millis(index as u64 * 75);
    let mut slot = 0;
    let mut respawned = None;
    let mut next_respawn = tokio::time::Instant::now();
    let mut min_hp = i32::MAX;
    let mut max_actors = 0;
    let mut max_players = 0;
    let mut max_live_hostiles = 0;
    let mut battle_samples = 0u64;
    let mut battle_live_total = 0u64;
    let mut battle_live_min = usize::MAX;
    let mut producer_first = Vec::new();
    let mut producer_recent = VecDeque::new();
    let mut producer_observations = 0u64;
    let mut producer_context = None;
    let mut producer_transitions = Vec::new();
    let mut omitted_producer_transitions = 0u64;
    let mut frame_trace = VecDeque::new();
    let mut omitted_frame_trace = 0u64;
    let began = tokio::time::Instant::now();
    let result=async {
  loop {
   tokio::select! {
    _=tokio::time::sleep_until(end)=>break,
    _=clock.tick()=>{
     let observations=observer.drain();
     omitted_observer+=observations.omitted;
     omitted_frame_trace+=observations.omitted_frames;
     for frame in observations.frames {
      if frame_trace.len()==128 {frame_trace.pop_front();}
      frame_trace.push_back(FrameTrace {elapsed_ms:frame.at.saturating_duration_since(began.into_std()).as_millis(),phase:frame.phase,actor:frame.actor,epoch:frame.epoch,sequence:frame.sequence,start:frame.start,end:frame.end,authority_tick:frame.authority_tick,control_epoch:frame.control_epoch,credit_step:frame.credit_step,pending_requests:frame.pending_requests,queued_inputs:frame.queued_inputs});
     }
     for observation in observations.samples {
      if observation.accepted_snapshot {snapshot_bytes+=observation.response_bytes as u64;}
      if !sample(&mut turnaround,observation.turnaround_ms) {omitted_turnaround+=1;}
      if !sample(by_kind.entry(observation.kind.into()).or_default(), observation.turnaround_ms) { *omitted_by_kind.entry(observation.kind.into()).or_default() += 1; }
      peak_pending=peak_pending.max(observation.pending_requests);
      peak_inputs=peak_inputs.max(observation.queued_inputs);
     }
     if let Some(at)=observations.snapshot_verified_at {
      if !sample(&mut snapshot_age,at.elapsed().as_secs_f64()*1000.) {omitted_age+=1;}
     }
     let Some(state)=state.as_ref() else {continue};
     let Some(hud)=state.hud.as_ref() else {return Err("Load principal has no player HUD".into())};
     min_hp=min_hp.min(hud.resources.hp);
     if hud.resources.hp<=0 {
      if mixed && tokio::time::Instant::now()>=next_respawn && respawned!=Some(hud.life) && outstanding.is_empty() && pending.is_empty() && send.try_send(Input::Respawn).is_ok() {
       respawned=Some(hud.life);
       pending.push_back((tokio::time::Instant::now(),Some("operation/respawn".into()),None,None));
      }
      continue;
     }
     if state.presentation.time<20. {continue;}
     // The designated frontline recipe exposes the initial life to ordinary
     // combat defeat. It rejoins the same mixed recipe after a real respawn.
     let frontline_initial_life=mixed && index==19 && hud.life.generation==0;
     if mixed && !frontline_initial_life && pending.is_empty() && outstanding.is_empty() && tokio::time::Instant::now()>=next_operation {
      let (input,label)=match operation {0=>(Input::EquipGear(verse_world::service::equipment::Slot::Head,501),"equipment"),1=>(Input::ClaimQuest(1),"quest_claim"),_=>(Input::UseItem(502),"item_use")};
      if send.try_send(input).is_ok() {pending.push_back((tokio::time::Instant::now(),Some(format!("operation/{label}")),None,None));operation+=1;next_operation=tokio::time::Instant::now()+Duration::from_secs(6);}
     }
     // A nonmovement operation shares this wake with movement, as in the native session.
     let seconds=began.elapsed().as_secs_f64();
     let phase=((seconds+index as f64*0.2).rem_euclid(8.)).floor() as u32;
     let mut axes=match phase {0=>[0.,1.],1=>[1.,0.],2=>[0.,-1.],3=>[-1.,0.],_=>[0.,0.]};
     let mut yaw=std::f32::consts::PI;
     if frontline_initial_life {
      let owned=state.presentation.actors.iter().find(|p|verse_engine::core::LifeId::from(p.life)==hud.life).map(|p|p.actor.position).unwrap_or_default();
      if let Some(target)=state.presentation.actors.iter().filter(|p|p.health>0 && p.visible && !p.actor.friendly && p.actor.nameplate && p.actor.model!="adventurer").min_by(|a,b|a.actor.position.distance_squared(owned).total_cmp(&b.actor.position.distance_squared(owned))) {
       let direction=target.actor.position-owned;
       axes=[0.,1.];yaw=(-direction.x).atan2(-direction.z);
      }
     }
     let mut intent=Intent::Move {axes,yaw};
     if tokio::time::Instant::now()>=next_cast && hud.casting.is_none() {
      let order=[Ability::Shield,Ability::Fireball,Ability::Web,Ability::Grease,Ability::Light,Ability::Thunderwave,Ability::MistyStep,Ability::Bow,Ability::FireBolt,Ability::MagicMissile];
      let selected=order[slot%order.len()];slot+=1;next_cast=tokio::time::Instant::now()+Duration::from_secs(2);
      let ability=if frontline_initial_life {Ability::FireBolt} else {selected};
      if hud.slots.iter().any(|s|s.ability==ability && s.ready) {
       let owned=state.presentation.actors.iter().find(|p|verse_engine::core::LifeId::from(p.life)==hud.life).map(|p|p.actor.position).unwrap_or_default();
       let target=state.presentation.actors.iter().filter(|p|p.health>0 && p.visible && !p.actor.friendly && p.actor.nameplate && p.actor.model!="adventurer").min_by(|a,b|a.actor.position.distance_squared(owned).total_cmp(&b.actor.position.distance_squared(owned)));
       let aim=target.map(|p|{let mut direction=p.actor.position-owned;direction.y=0.;direction.normalize_or_zero().to_array()}).unwrap_or([0.,0.,1.]);
       intent=Intent::Cast {ability,target:target.map(|p|p.life.into()),aim};
      }
     }
     if movement_frames {
      if let Some(baseline)=state.movement {
       let observation=ProducerObservation {elapsed_ms:began.elapsed().as_millis(),actor:baseline.life.actor,epoch:baseline.epoch,profile:baseline.profile,confirmed_step:baseline.physics_step,snapshot_world_step:baseline.world_step,verified_credit:credit.filter(|(life,e,_)|(*life,*e)==(baseline.life,baseline.epoch)).map(|(_,_,step)|step),cursor:frame_cursor.filter(|(life,e,_)|(*life,*e)==(baseline.life,baseline.epoch)).map(|(_,_,step)|step),outstanding:outstanding.len(),pending:pending.len(),input_depth:send.max_capacity()-send.capacity()};
       let context=(baseline.life,baseline.epoch);
       if producer_context.is_some_and(|old|old!=context) {
        if producer_transitions.len()<8 {producer_transitions.push(ProducerTransition {before:producer_recent.clone(),after:observation.clone(),frames:frame_trace.clone()});}
        else {omitted_producer_transitions=omitted_producer_transitions.saturating_add(1);}
       }
       producer_context=Some(context);
       producer_observations=producer_observations.saturating_add(1);
       if producer_first.len()<128 {producer_first.push(observation.clone());}
       if producer_recent.len()==128 {producer_recent.pop_front();}
       producer_recent.push_back(observation);
      }
     }
     // Flush available movement before a cast can wait for a fresh control header.
     // Keep the cast even when this wake has no additional interval credit.
     let movement_first=movement_frames && matches!(intent,Intent::Cast {..}) && state.movement.is_some_and(|baseline|baseline.profile==verse_world::movement::Profile::Frames);
     let mut interval_work=0;
     for emission in 0..if movement_frames {2} else {1} {
     let intent=if emission==usize::from(movement_first) {intent.clone()} else {Intent::Move {axes,yaw}};
     token=token.checked_add(1).ok_or("Load input identities exhausted")?;
     let moving=matches!(intent,Intent::Move {..});
     let mut remaining_frame_credit=false;
     let input=if movement_frames && moving {
      let Some(baseline)=state.movement else {break};
      let context=(baseline.life,baseline.epoch);
      if credit.is_some_and(|(life,epoch,_)|(life,epoch)!=context) {break;}
      if baseline.profile!=verse_world::movement::Profile::Frames {
       frame_cursor=None;
       if baseline.character.support.is_some() && baseline.held.axes(baseline.physics_step)==[0.;2] && frame_entry!=Some(context) && outstanding.is_empty() && pending.is_empty() {
        match send.try_send(Input::BeginMovementFrames {life:baseline.life,epoch:baseline.epoch}) {
         Ok(())=>{frame_entry=Some(context);pending.push_back((tokio::time::Instant::now(),None,Some(context),None));},
         Err(tokio::sync::mpsc::error::TrySendError::Full(_))=>pressure+=1,
         Err(_)=>return Err("Load worker input closed".into()),
        }
       }
       break;
      }
      let start=match frame_cursor {Some((life,old_epoch,start)) if (life,old_epoch)==context=>start,_=>baseline.physics_step};
      let world_step=credit.filter(|(life,epoch,_)|(*life,*epoch)==context).map_or(baseline.world_step,|(_,_,step)|step.max(baseline.world_step));
      let limit=world_step.checked_add(u64::from(verse_world::movement::frames::MAX_STEPS)).ok_or("Load movement credit exhausted")?;
      let steps=limit.saturating_sub(start).min(u64::from(verse_world::movement::frames::MAX_STEPS)).min(u64::from(verse_world::movement::frames::MAX_STEPS-interval_work)) as u32;
      if steps==0 {if movement_first {continue;} else {break;}}
      remaining_frame_credit=start+u64::from(steps)<limit;
      let frame=verse_world::movement::frames::Frame {life:baseline.life,epoch:baseline.epoch,sequence:0,tick:0,start,steps,
       segments:vec![verse_world::movement::frames::Segment {offset:0,axes,yaw,until:start+verse_world::movement::HELD_STEPS,jump:false}]};
      frame.validate_payload()?;
      Input::MovementFrame {token,frame}
     } else {Input::TrackedCommand {token,life:hud.life,epoch,intent}};
     let proposed_end=match &input {Input::MovementFrame {frame,..}=>Some(((frame.life,frame.epoch,frame.end()?),frame.steps)),_=>None};
     match send.try_send(input) {
      Ok(())=>{outstanding.insert(token);if let Some((cursor,steps))=proposed_end {frame_cursor=Some(cursor);interval_work+=steps;
       // Continue bounded intervals immediately while verified credit remains.
       if remaining_frame_credit {clock.reset_immediately();}} if moving {movement+=1;}},
      Err(tokio::sync::mpsc::error::TrySendError::Full(_))=>{pressure+=1;break;},
      Err(_)=>return Err("Load worker input closed".into()),
     }
     }
    }
    update=receive.recv()=>match update.ok_or("Load worker output closed")? {
     Update::Snapshot(response)=>{
      let latest_credit=response.control.as_ref().map(|c|(c.life.into(),c.epoch,c.credit_step));
      let movement=match &response.body {Reply::Snapshot {state}=>state.movement,_=>None};
      if credit_wakes_frames(credit,latest_credit,movement) {clock.reset_immediately();}
      credit=latest_credit;
      epoch=response.control.as_ref().ok_or("Load lost player control")?.epoch;
      let Reply::Snapshot {state:latest}=response.body else {return Err("Load snapshot refused".into())};
      if let Some(baseline)=latest.movement.filter(|b|b.profile==verse_world::movement::Profile::Frames) {
       observed_frame_snapshots+=1;if latest.presentation.time>=20. {battle_framed+=1;}
       if let Some((life,epoch,step))=confirmed {if life==baseline.life && epoch==baseline.epoch {confirmed_steps+=baseline.physics_step.saturating_sub(step);}}
       confirmed=Some((baseline.life,baseline.epoch,baseline.physics_step));
      } else {confirmed=None;}
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
      Ok(_)=>{bound_frames+=1;pending.push_back((tokio::time::Instant::now(),None,None,Some(token)));},
      Err(message)=>{outstanding.remove(&token);refused+=1;if refusal_trace.len()<32 {refusal_trace.push(serde_json::json!({"stage":"frame_binding","message":message}));}},
     },
     Update::Outcome(response)=>{
      let latest_credit=response.control.as_ref().map(|c|(c.life.into(),c.epoch,c.credit_step));
      if credit_wakes_frames(credit,latest_credit,state.as_ref().and_then(|s|s.movement)) {clock.reset_immediately();}
      credit=latest_credit;
      if let Some(control)=response.control.as_ref() {epoch=control.epoch;}
      if let Some((started,ability,entry,token))=pending.pop_front() {
       if let Some(token)=token {outstanding.remove(&token);}
       if entry.is_some() && frame_entry==entry && matches!(&response.body,Reply::Refused {..}) {frame_entry=None;}
       if !sample(&mut latency,started.elapsed().as_secs_f64()*1000.) {omitted_latency+=1;}
       if ability.as_deref()==Some("operation/respawn") && matches!(&response.body,Reply::Refused {code,message} if code=="command" && matches!(message.as_str(),"Character teleport endpoint is obstructed" | "Teleport destination is occupied")) {respawned=None;next_respawn=tokio::time::Instant::now()+Duration::from_secs(1);}
       match response.body {Reply::Accepted|Reply::GearEquipped {..}|Reply::ItemUsed {..}|Reply::QuestClaimed {..}=>{if let Some(ability)=ability {
        if let Some(label)=ability.strip_prefix("operation/") {*operations.entry(label.into()).or_default()+=1;}
        else {*casts.entry(ability).or_default()+=1;}
       }},Reply::Refused {message,code,..}=>{refused+=1;if refusal_trace.len()<32 {refusal_trace.push(serde_json::json!({"stage":"outcome","message":message,"code":code,"tick":response.tick,"control":response.control}));}},_=>{}}
      }
     }
     Update::MovementSuperseded {token,..}=>{outstanding.remove(&token);},
     auxiliary @ (Update::Events {..}|Update::Inventory(_))=>{
      let control=match &auxiliary {Update::Events {control,..}=>control.as_ref(),Update::Inventory(response)=>response.control.as_ref(),_=>unreachable!()};
      let latest_credit=control.map(|c|(c.life.into(),c.epoch,c.credit_step));
      if credit_wakes_frames(credit,latest_credit,state.as_ref().and_then(|s|s.movement)) {clock.reset_immediately();}
      credit=latest_credit;
      if let Some(control)=control {epoch=control.epoch;}
     },
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
    let by_kind = by_kind
        .into_iter()
        .map(|(kind, values)| (kind, summary(values)))
        .collect::<BTreeMap<_, _>>();
    Ok(
        serde_json::json!({"player":index,"status":if failure_stage.is_some() {"failed"} else {"complete"},"failure_stage":failure_stage,"observation_error":observation_error,"worker_error":worker_error,"omitted_refusal_samples":refused.saturating_sub(refusal_trace.len() as u64),"refusal_trace":refusal_trace,"omitted_latency_samples":omitted_latency,"snapshots":snapshots,"snapshot_bytes":snapshot_bytes,"snapshot_byte_measurement":"verified_wire_payload_excluding_framing","maximum_actors":max_actors,"maximum_players":max_players,"maximum_live_hostiles":max_live_hostiles,"battle_occupancy":{"samples":battle_samples,"minimum_live_hostiles":(battle_samples>0).then_some(battle_live_min),"mean_live_hostiles":(battle_samples>0).then(||battle_live_total as f64/battle_samples as f64)},"movement_profile":if movement_frames {"authority_credit_intervals"} else {"legacy_commands"},"observed_frame_snapshots":observed_frame_snapshots,"movement_inputs":movement,"frame_timing":{"recent":frame_trace,"window_capacity":128,"omitted":omitted_frame_trace},"producer_timing":{"observations":producer_observations,"first":producer_first,"recent":producer_recent,"window_capacity":128,"transitions":producer_transitions,"transition_capacity":8,"omitted_transitions":omitted_producer_transitions},"bound_frames":bound_frames,"battle_framed_snapshots":battle_framed,"confirmed_interval_steps":confirmed_steps,"input_pressure":pressure,"unacknowledged_bound_operations":pending.len(),"unbound_inputs":outstanding.len(),"refusals":refused,"accepted_casts":casts,"accepted_operations":operations,"request_turnaround_ms":summary(turnaround),"request_turnaround_by_kind_ms":by_kind,"omitted_request_samples_by_kind":omitted_by_kind,"pending_request_peak":peak_pending,"queued_input_peak":peak_inputs,"snapshot_verified_age_ms":summary(snapshot_age),"omitted_request_samples":omitted_turnaround,"omitted_age_samples":omitted_age,"omitted_observer_samples":omitted_observer,"minimum_hp":(min_hp!=i32::MAX).then_some(min_hp),"binding_to_outcome_ms":summary(latency)}),
    )
}
