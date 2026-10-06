//! Exercises the shared native session without a window or owner state.
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use verse::{
    imported::chamber_session::{Frame, Note, Session},
    profiling::FrameProfile,
};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        view::View,
        wire::Reply,
        worker::{self, Input, Observer, Update},
    },
};

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
struct ControlTransition {
    tick: u64,
    before: verse_world::movement::Baseline,
    after: verse_world::movement::Baseline,
    frames: VecDeque<FrameTrace>,
}
pub struct Tap {
    pub frame: Frame,
    pub applied_at: Instant,
    pub projection_ms: f64,
}
pub async fn player(
    client: Client,
    index: usize,
    end: tokio::time::Instant,
    scene: Scene,
    pack: Option<Pack>,
    tap: Option<mpsc::Sender<Tap>>,
    atlas: Option<std::sync::Arc<verse::ui::Atlas>>,
) -> Result<serde_json::Value, String> {
    let instance = client.instance();
    let (input, inputs, updates, mut output) = worker::channels();
    let (project, projection) = mpsc::channel(worker::UPDATE_CAPACITY);
    let (stop, stopping) = oneshot::channel();
    let observer = Observer::default();
    let task = tokio::spawn(worker::run_profiled(
        client,
        Cursor::new(instance),
        worker::NATIVE_CADENCE,
        inputs,
        updates,
        stopping,
        observer.clone(),
    ));
    let mut session = Session::attached(View::new(instance, 10., 0)?, input, projection);
    session.observe();
    let mut measurements = FrameProfile::new(120);
    let mut window = FrameProfile::new(0);
    let mut windows = Vec::new();
    let mut window_frames = 0u64;
    let mut operations = BTreeMap::<String, u64>::new();
    let mut occupancy = 0u64;
    let mut min_hostiles = usize::MAX;
    let mut max_players = 0usize;
    let mut confirmed = None;
    let mut confirmed_steps = 0u64;
    let mut framed = 0u64;
    let mut battle_framed = 0u64;
    let mut movements = 0u64;
    let mut accepted_frames = 0u64;
    let mut bound_frames = 0u64;
    let mut min_hp = i32::MAX;
    let mut refusals = 0u64;
    let mut trace = Vec::new();
    let mut omitted_observer = 0u64;
    let mut correction_trace = Vec::new();
    let mut maximum_correction_detail: Option<serde_json::Value> = None;
    let mut omitted_corrections = 0u64;
    let mut omitted_taps = 0u64;
    let mut frame = 0u64;
    let mut operation = 0u64;
    let began = Instant::now();
    let mut last = began;
    let mut next_cast = began + Duration::from_secs(1) + Duration::from_millis(index as u64 * 75);
    let mut next_operation =
        began + Duration::from_secs(2) + Duration::from_millis(index as u64 * 75);
    let mut cast = 0usize;
    let mut responses = 0u64;
    let mut frontline_yaw = None;
    let mut frame_trace = VecDeque::new();
    let mut control_transitions = Vec::new();
    let mut previous_movement: Option<verse_world::movement::Baseline> = None;
    let mut omitted_frame_trace = 0u64;
    let mut omitted_control_transitions = 0u64;
    let result=async {
      let mut clock=tokio::time::interval(Duration::from_secs_f64(1./60.));clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
      while tokio::time::Instant::now()<end {
        clock.tick().await;frame+=1;window_frames+=1;
        if window_frames>1800 {windows.push(serde_json::json!({"end_seconds":began.elapsed().as_secs_f64(),"measurements":window.summary()}));window=FrameProfile::new(0);window_frames=1;}
        while let Ok(update)=output.try_recv() {
          match &update {
            Update::Snapshot(response)=>if let Reply::Snapshot {state}=&response.body {
              if let Some(after)=state.movement {
                if let Some(before)=previous_movement.filter(|b|b.profile==verse_world::movement::Profile::Frames && (b.life,b.epoch)!=(after.life,after.epoch)) {
                  if control_transitions.len()<8 {control_transitions.push(ControlTransition {tick:response.tick,before,after,frames:frame_trace.clone()});}
                  else {omitted_control_transitions+=1;}
                }
                previous_movement=Some(after);
              }
              if index==19 {
                frontline_yaw=state.hud.as_ref().filter(|h|h.life.generation==0).and_then(|h| {
                  let owned=state.presentation.actors.iter().find(|a|verse_engine::core::LifeId::from(a.life)==h.life)?.actor.position;
                  let target=state.presentation.actors.iter().filter(|a|a.health>0 && a.visible && !a.actor.friendly && a.actor.nameplate && a.actor.model!="adventurer").min_by(|a,b|a.actor.position.distance_squared(owned).total_cmp(&b.actor.position.distance_squared(owned)))?;
                  let direction=target.actor.position-owned;Some((-direction.x).atan2(-direction.z))
                });
              }
              max_players=max_players.max(state.presentation.actors.iter().filter(|p|p.actor.model=="adventurer").count());
              if state.presentation.time>=20. {occupancy+=1;min_hostiles=min_hostiles.min(state.presentation.actors.iter().filter(|p|p.health>0 && !p.actor.friendly && p.actor.model!="adventurer").count());}
              if let Some(baseline)=state.movement.filter(|b|b.profile==verse_world::movement::Profile::Frames) {
                framed+=1;if state.presentation.time>=20. {battle_framed+=1;}
                if let Some((life,epoch,step))=confirmed {if life==baseline.life && epoch==baseline.epoch {confirmed_steps+=baseline.physics_step.saturating_sub(step);}}
                confirmed=Some((baseline.life,baseline.epoch,baseline.physics_step));
              } else {confirmed=None;}
            },
            Update::FrameBound {binding:Ok(_),..}=>bound_frames+=1,
            Update::Outcome(response)=>{responses+=1;match &response.body {
              Reply::GearEquipped {..}=>*operations.entry("equipment".into()).or_default()+=1,
              Reply::QuestClaimed {..}=>*operations.entry("quest_claim".into()).or_default()+=1,
              Reply::ItemUsed {..}=>*operations.entry("item_use".into()).or_default()+=1,
              Reply::Accepted=>accepted_frames+=1,
              _=>{}
            }},
            _=>{}
          }
          project.send(update).await.map_err(|_|"Native projection closed")?;
          session.consume(&scene)?;
        }
        if task.is_finished() {return Err("Native session worker stopped before its deadline".into());}
        let now=Instant::now();let dt=now.duration_since(last).as_secs_f64().min(0.1);last=now;
        if let Some(hud)=session.hud() {min_hp=min_hp.min(hud.resources.hp);}
        if session.dead() {session.respawn();}
        let frontline_initial_life=index==19 && session.hud().is_some_and(|h|h.life.generation==0);
        if session.controlled(&scene) && session.movement_frames() && bound_frames>=10 {
          if !frontline_initial_life && now>=next_operation && session.idle() {
            let input=match operation {0=>Input::EquipGear(verse_world::service::equipment::Slot::Head,501),1=>Input::ClaimQuest(1),_=>Input::UseItem(502)};
            session.send(input);operation+=1;next_operation=now+Duration::from_secs(6);
          }
          if now>=next_cast {
            let abilities=[Ability::Shield,Ability::Fireball,Ability::Web,Ability::Grease,Ability::Light,Ability::Thunderwave,Ability::MistyStep,Ability::Bow,Ability::FireBolt,Ability::MagicMissile];
            session.target_nearest();session.cast(&scene,if frontline_initial_life {Ability::FireBolt} else {abilities[cast%abilities.len()]});cast+=1;next_cast=now+Duration::from_secs(2);
          }
        }
        let phase=((began.elapsed().as_secs_f64()+index as f64*0.2).rem_euclid(8.)).floor() as u32;
        let axes=match phase {0=>[0.,1.],1=>[1.,0.],2=>[0.,-1.],3=>[-1.,0.],_=>[0.,0.]};
        if frontline_initial_life {if let Some(yaw)=frontline_yaw {session.yaw=yaw;}}
        session.steer(&scene,if frontline_initial_life {[0.,1.]} else {axes},dt as f32,session.movement_frames())?;
        if session.controlled(&scene) {movements+=1;}
        let prediction_delay=session.prediction_delay_steps() as f64 * 1000. / 120.;
        measurements.record(frame,"input_to_prediction_clock_delay_ms",prediction_delay);
        window.record(frame,"input_to_prediction_clock_delay_ms",prediction_delay);
        for note in session.take_notes() {match note {
          Note::Correction {distance,discontinuity:false,detail}=>{
            measurements.record(frame,"prediction_correction_meters",distance);window.record(frame,"prediction_correction_meters",distance);
            if maximum_correction_detail.as_ref().is_none_or(|record|record["distance"].as_f64().is_none_or(|previous|distance>previous)) {
              maximum_correction_detail=Some(serde_json::json!({"seconds":began.elapsed().as_secs_f64(),"distance":distance,"detail":detail.clone()}));
            }
            if distance>0.25 {if correction_trace.len()<32 {correction_trace.push(serde_json::json!({"seconds":began.elapsed().as_secs_f64(),"distance":distance,"detail":detail}));} else {omitted_corrections+=1;}}
          },
          Note::Reset(reason)=>{measurements.record(frame,"prediction_reset",1.);if trace.len()<32 {trace.push(serde_json::json!({"reset":reason,"seconds":began.elapsed().as_secs_f64()}));}},
          Note::Refusal(detail)=>{refusals+=1;if trace.len()<32 {trace.push(detail);}},
          _=>{}
        }}
        let observations=observer.drain();omitted_observer+=observations.omitted;
        omitted_frame_trace+=observations.omitted_frames;
        for frame in observations.frames {
          if frame_trace.len()==128 {frame_trace.pop_front();}
          frame_trace.push_back(FrameTrace {elapsed_ms:frame.at.saturating_duration_since(began).as_millis(),phase:frame.phase,actor:frame.actor,epoch:frame.epoch,sequence:frame.sequence,start:frame.start,end:frame.end,authority_tick:frame.authority_tick,control_epoch:frame.control_epoch,credit_step:frame.credit_step,pending_requests:frame.pending_requests,queued_inputs:frame.queued_inputs});
        }
        for sample in observations.samples {
          measurements.record(frame,"request_turnaround_ms",sample.turnaround_ms);window.record(frame,"request_turnaround_ms",sample.turnaround_ms);
          measurements.record(frame,"pending_requests",sample.pending_requests as f64);window.record(frame,"pending_requests",sample.pending_requests as f64);
          measurements.record(frame,"queued_inputs",sample.queued_inputs as f64);window.record(frame,"queued_inputs",sample.queued_inputs as f64);
        }
        if let Some(at)=observations.snapshot_verified_at {measurements.record(frame,"snapshot_verified_age_ms",at.elapsed().as_secs_f64()*1000.);}
        if let (Some(tap),Some(atlas),Some(pack))=(&tap,&atlas,&pack) {
          let started=Instant::now();let drawn=session.frame(pack,atlas,&scene,[1280,720])?;
          measurements.record(frame,"frame_projection_cpu_ms",started.elapsed().as_secs_f64()*1000.);
          if let Some(age)=session.snapshot_age() {
            if tap.try_send(Tap {frame:drawn,applied_at:Instant::now()-age,projection_ms:started.elapsed().as_secs_f64()*1000.}).is_err() {omitted_taps+=1;}
          }
        }
      }
      Ok::<_,String>(())
    }.await;
    let _ = stop.send(());
    let worker = task.await;
    let worker_error = match worker {
        Ok(Err(e)) => Some(e),
        Err(e) => Some(e.to_string()),
        _ => None,
    };
    let error = result
        .as_ref()
        .err()
        .cloned()
        .or_else(|| worker_error.clone());
    if window_frames > 0 {
        windows.push(serde_json::json!({"end_seconds":began.elapsed().as_secs_f64(),"measurements":window.summary()}));
    }
    operations.insert("respawn".into(), session.life_changes);
    Ok(
        serde_json::json!({"player":index,"status":if error.is_none(){"complete"}else{"failed"},"error":error,"worker_error":worker_error,"snapshots":session.snapshots,"maximum_players":max_players,"battle_occupancy":{"samples":occupancy,"minimum_live_hostiles":(occupancy>0).then_some(min_hostiles)},"movement_profile":"native_session_intervals","frame_timing":{"recent":frame_trace,"window_capacity":128,"omitted":omitted_frame_trace},"control_transitions":control_transitions,"control_transition_capacity":8,"omitted_control_transitions":omitted_control_transitions,"observed_frame_snapshots":framed,"movement_inputs":movements,"battle_framed_snapshots":battle_framed,"confirmed_interval_steps":confirmed_steps,"bound_frames":bound_frames,"accepted_outcomes":accepted_frames,"outcomes":responses,"accepted_casts":session.accepted_casts,"accepted_operations":operations,"damage_events":session.damage_events,"minimum_hp":(min_hp!=i32::MAX).then_some(min_hp),"prediction_horizon_pauses":session.prediction_horizon_pauses(),"prediction_failures":session.prediction_failures(),"last_prediction_failure":session.last_prediction_failure(),"prediction_embedding_deferrals":session.prediction_embedding_deferrals(),"prediction_separating_steps":session.prediction_separating_steps(),"last_prediction_embedding":session.prediction_embedding_diagnostic(),"unacknowledged_bound_operations":session.pending(),"refusals":refusals,"refusal_trace":trace,"correction_trace":correction_trace,"maximum_correction_detail":maximum_correction_detail,"omitted_correction_details":omitted_corrections,"omitted_observer_samples":omitted_observer,"omitted_render_frames":omitted_taps,"windows":windows,"measurements":measurements.summary()}),
    )
}
