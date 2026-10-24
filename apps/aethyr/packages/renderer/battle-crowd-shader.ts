import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { MMORPG_FOG_WGSL } from "./mmorpg-fog.ts";
import { BATTLE_COMBAT_WGSL } from "./battle-combat-shader.ts";
import { BATTLE_GRID_WGSL } from "./battle-grid.ts";
import { BATTLE_SQUAD_WGSL } from "./battle-squad-shader.ts";

export const BATTLE_COMMON_WGSL = BATTLE_GRID_WGSL + /* wgsl */ `
struct Actor { pose:vec4f, motion:vec4f, clocks:vec4f, info:vec4u }
struct Frame {
 viewProjection:mat4x4f, eyeTime:vec4f, battle:vec4f, counts:vec4u,
 ground:vec4f, tuning:vec4f, sun:vec4f, sunColor:vec4f, ambient:vec4f, planes:array<vec4f,6>, participantPose:vec4f,participantAim:vec4f,participant:vec4u,
}
fn hash(value:u32)->u32 { var x=value; x=(x^(x>>16u))*0x7feb352du; x=(x^(x>>15u))*0x846ca68bu; return x^(x>>16u); }
fn random(value:u32)->f32 { return f32(hash(value)&0x00ffffffu)/16777216.0; }
fn safeNormal(v:vec2f)->vec2f { let n=dot(v,v); return v*inverseSqrt(max(n,1e-12)); }
`;

/** Each step reads immutable actors and delayed damage, then writes separate next buffers. */
export const BATTLE_SIMULATION_WGSL =
  BATTLE_COMMON_WGSL +
  /* wgsl */ `
struct Cell { count:atomic<u32>, x:atomic<i32>, z:atomic<i32>,members:array<u32,64> }
struct Damage {amount:u32,local:u32}
struct DamageOut {amount:atomic<u32>,local:atomic<u32>}
@group(0)@binding(0)var<storage,read>source:array<Actor>;
@group(0)@binding(1)var<storage,read_write>result:array<Actor>;
@group(0)@binding(2)var<storage,read_write>cells:array<Cell>;
@group(0)@binding(4)var<storage,read>damageIn:array<Damage>;
@group(0)@binding(5)var<storage,read_write>damageOut:array<DamageOut>;
@group(0)@binding(6)var<storage,read_write>stats:array<atomic<u32>>;
@group(0)@binding(7)var<storage,read>heights:array<f32>;
@group(0)@binding(8)var<uniform>frame:Frame;
` +
  BATTLE_COMBAT_WGSL +
  BATTLE_SQUAD_WGSL +
  /* wgsl */ `
var<workgroup> scatterStats:array<atomic<u32>,3>;
var<workgroup> simulationStats:array<atomic<u32>,64>;
fn maxHealth(kind:u32)->f32 { switch kind {case 0u:{return 200.0;}case 1u:{return 100.0;}case 2u:{return 50.0;}default:{return 1000.0;}} }
fn initialHealth(kind:u32)->f32 { return select(maxHealth(kind),20.0,kind==2u); }
fn reach(kind:u32)->f32 { if kind==1u{return 6.666667;} if kind==3u{return 2.5;}return 1.666667; }
fn attackTime(kind:u32)->f32 { if kind==0u{return .35;}if kind==3u{return 1.5;}return 1.0; }
fn attackDuration(kind:u32)->f32 { switch kind {case 0u:{return 1.5333333;}case 1u:{return 3.3;}case 2u:{return 3.8666667;}default:{return 2.7;}} }
fn cooldown(kind:u32)->f32 { switch kind {case 0u:{return 1.0;}case 1u:{return 1.5;}case 2u:{return 3.0;}default:{return 2.6;}} }
fn damage(kind:u32)->u32 { switch kind {case 0u:{return 300u;}case 1u:{return 20u;}case 2u:{return 5u;}default:{return 40u;}} }
fn groundHeight(position:vec2f)->f32 {
 let resolution=u32(frame.ground.w);let local=position-frame.ground.xy;
 var q=clamp(local/frame.ground.z+vec2(.5),vec2(0),vec2(1))*f32(resolution-1u);
 if frame.counts.w==1u{let magnitude=abs(local);let scaled=select(magnitude/2.0,vec2(64.0)+(magnitude-vec2(128.0))/24.0,magnitude>vec2(128.0));q=clamp(vec2(128.0)+sign(local)*scaled,vec2(0),vec2(256));}
 let a=vec2u(floor(q));let b=min(a+vec2u(1),vec2u(resolution-1u));let f=fract(q);
 let topLeft=heights[a.y*resolution+a.x];let topRight=heights[a.y*resolution+b.x];let bottomLeft=heights[b.y*resolution+a.x];let bottomRight=heights[b.y*resolution+b.x];
 if f.x+f.y<=1.0{return topLeft+(topRight-topLeft)*f.x+(bottomLeft-topLeft)*f.y;}
 return bottomRight+(bottomLeft-bottomRight)*(1.0-f.x)+(topRight-bottomRight)*(1.0-f.y);
}
fn cellPosition(position:vec2f)->vec2i {return battleGridPosition(position,frame.battle.xy);}
fn cellIndex(position:vec2i)->u32 {return battleGridIndex(position);}
fn cellHasTeam(cell:u32,team:u32)->bool{return (atomicLoad(&cells[cell].x)&i32(1u<<team))!=0;}
fn withinDistanceSquared(squared:f32,limit:f32)->bool{
 // Conservative rejection leaves the original square-root comparisons and stable ties intact.
 let padded=limit+max(.00001,limit*.000001);return squared<=padded*padded;
}
fn spawn(id:u32)->vec2f {
 let columns=u32(ceil(sqrt(f32(frame.counts.x))));let rows=(frame.counts.x+columns-1u)/columns;
 let spacing=2.0*frame.battle.z/f32(columns);let jitter=vec2(random(id^frame.counts.z),random(id^frame.counts.z^0xb123u))-.5;
 let slot=vec2i(i32(id%columns),i32(id/columns));var shift=vec2f(0);
 if source[id].info.x!=3u{
  let offsets=array<vec2i,4>(vec2i(-1,0),vec2i(1,0),vec2i(0,-1),vec2i(0,1));
  for(var i=0u;i<4u;i++){let q=slot+offsets[i];let candidate=u32(max(q.y,0))*columns+u32(max(q.x,0));
   if all(q>=vec2i(0))&&q.x<i32(columns)&&candidate<frame.counts.x&&source[candidate].info.x==3u{shift-=vec2f(offsets[i])*battleLargeNeighborOffset;}
  }
 }
 return frame.battle.xy+vec2((f32(slot.x)+.5-f32(columns)*.5)*spacing,(f32(slot.y)+.5-f32(rows)*.5)*spacing)+jitter*spacing*battleFormationJitter+shift;
}
fn turnToward(yaw:f32,goal:vec2f,dt:f32)->f32 {
 let forward=vec2(sin(yaw),-cos(yaw));let direction=safeNormal(goal);
 let alignment=clamp(dot(forward,direction),-1.0,1.0);
 if alignment>.999999{return yaw;}
 let cross=forward.x*direction.y-forward.y*direction.x;
 let angle=min(acos(alignment),4.0*dt);
 // Exact axis goals and antipodal headings must not pass signed zero through fast atan2.
 let positive=cross>0.0||(abs(cross)<.00001&&alignment<0.0);
 return yaw+select(-angle,angle,positive);
}
fn clearSpawn(location:vec2f,kind:u32,id:u32)->bool {
 let center=cellPosition(location);let radius=bodyRadius(kind);
 for(var z=-1;z<=1;z++){for(var x=-1;x<=1;x++){
  let q=center+vec2i(x,z);if any(q<vec2i(0))||any(q>=vec2i(battleGridDimension)){continue;}
  let c=cellIndex(q);let count=min(atomicLoad(&cells[c].count),64u);
  for(var n=0u;n<count;n++){let otherId=cells[c].members[n];if otherId==id{continue;}let other=source[otherId];
   if other.motion.z>0.0&&distance(location,other.pose.xz)<radius+bodyRadius(other.info.x)+.1{return false;}
  }
 }}return true;
}
@compute @workgroup_size(128)fn clear(@builtin(global_invocation_id)inv:vec3u) {
 let id=inv.x;
 if id<battleGridCells {atomicStore(&cells[id].count,0u);atomicStore(&cells[id].x,0);atomicStore(&cells[id].z,0);}
 if id<frame.counts.x {atomicStore(&damageOut[id].amount,0u);atomicStore(&damageOut[id].local,0u);}
 if id<64u && (id<4u||id==8u||id==12u||id==13u||id==14u||id==15u||id==25u||id==26u||id==27u||id==32u||id==33u||id==35u){atomicStore(&stats[id],0u);}
 if id==0u {atomicAdd(&stats[11],1u);}
}
@compute @workgroup_size(128)fn scatter(@builtin(global_invocation_id)inv:vec3u,@builtin(local_invocation_index)lane:u32) {
 if lane<3u{atomicStore(&scatterStats[lane],0u);}workgroupBarrier();
 let id=inv.x;
 if id<frame.counts.x {
  let a=source[id];
  if a.motion.z>0.0{
   let position=a.pose.xz;let cell=cellIndex(cellPosition(position));let slot=atomicAdd(&cells[cell].count,1u);
   atomicOr(&cells[cell].x,i32(1u<<a.info.y));
   atomicMax(&scatterStats[0],slot+1u);if slot==0u{atomicAdd(&scatterStats[1],1u);}
   if slot<64u{cells[cell].members[slot]=id;}else{atomicAdd(&scatterStats[2],1u);}
  }
 }
 workgroupBarrier();
 if lane==0u{atomicMax(&stats[27],atomicLoad(&scatterStats[0]));}
 if lane==1u{let value=atomicLoad(&scatterStats[1]);if value>0u{atomicAdd(&stats[25],value);}}
 if lane==2u{let value=atomicLoad(&scatterStats[2]);if value>0u{atomicAdd(&stats[8],value);atomicAdd(&stats[54],value);}}
}
@compute @workgroup_size(128)fn simulate(@builtin(global_invocation_id)inv:vec3u,@builtin(local_invocation_index)lane:u32) {
 if lane<64u{atomicStore(&simulationStats[lane],0u);}workgroupBarrier();
 let id=inv.x;
 if id<frame.counts.x {
 var a=source[id];let dt=frame.battle.w;let kind=a.info.x;let previousVelocity=a.motion.xy;let previousState=a.info.w;
 a.motion=vec4(0,0,a.motion.z,a.motion.w);a.clocks.x=max(0.0,a.clocks.x-dt);
 if a.motion.z>0.0 {
  let incoming=damageIn[id].amount;a.motion.z=max(0.0,a.motion.z-f32(incoming));a.clocks.y=max(0.0,a.clocks.y-dt);
  if incoming>0u{a.clocks.y=.18;}
  if a.motion.z<=0.0{a.info.w=3u;a.motion.w=0.0;a.clocks.y=0.0;atomicAdd(&simulationStats[6],1u);atomicAdd(&simulationStats[22u+a.info.y],1u);if damageIn[id].local>0u{atomicAdd(&stats[44],1u);}}
 }
 if a.motion.z<=0.0 {
  a.clocks.y+=dt;a.motion.w=a.clocks.y;
  let location=spawn(id);
  if a.clocks.y>=frame.tuning.z&&clearSpawn(location,kind,id) {
   a.pose=vec4(location.x,groundHeight(location),location.y,a.pose.w);
   a.motion=vec4(0,0,initialHealth(kind),0);a.clocks=vec4(0);a.info.z=0xffffffffu;a.info.w=0u;atomicAdd(&simulationStats[7],1u);
  }
 }else{
  let ranged=rangedRole(id);if ranged{atomicAdd(&simulationStats[32],1u);}else{atomicAdd(&simulationStats[33],1u);}
  let position=a.pose.xz;let squad=combat.squads[squadId(id,a.info.y)];var enemyId=a.info.z;var distance=1e9;var toEnemy=vec2(0.0);
  if enemyId==participantId{if participantAvailable(a.info.y){toEnemy=frame.participantPose.xz-position;distance=length(toEnemy);}else{enemyId=0xffffffffu;}}
  else if enemyId<frame.counts.x {let enemy=source[enemyId];if enemy.motion.z>0.0&&enemy.info.y!=a.info.y{toEnemy=enemy.pose.xz-position;distance=length(toEnemy);}else{enemyId=0xffffffffu;}}
  let cell=cellPosition(position);var correction=vec2(0.0);
  var constraints:array<vec4f,16>;var followingSpeed:array<f32,16>;var constraintCount=0u;
  let retainedDistance=distance;var candidateDistance=distance;var candidateId=enemyId;var candidateDirection=toEnemy;
  var threatDistance=1e9;var threatDirection=vec2f(0);var threatId=0xffffffffu;
  let search=(enemyId>=frame.counts.x&&enemyId!=participantId) || ((id+u32(atomicLoad(&stats[11])))%12u==0u && a.info.w!=2u);
  let extent=select(1,4,kind==1u&&search&&!ranged);var visits=0u;
  for(var z=-extent;z<=extent;z++){for(var x=-extent;x<=extent;x++){
   let coordinate=cell+vec2i(x,z);if any(coordinate<vec2i(0))||any(coordinate>=vec2i(battleGridDimension)){continue;}
   let c=cellIndex(coordinate);let nearby=abs(x)<=1&&abs(z)<=1;
   if !nearby{if !search||!cellHasTeam(c,1u-a.info.y){continue;}}
   let population=atomicLoad(&cells[c].count);if population==0u{continue;}
   let tested=min(population,64u);visits+=tested;
   for(var n=0u;n<tested;n++){
    let candidate=cells[c].members[n];if candidate==id{continue;}let other=source[candidate];if other.motion.z<=0.0{continue;}
    let delta=position-other.pose.xz;let squared=dot(delta,delta);var d=-1.0;
    if ranged&&nearby&&other.info.y!=a.info.y&&withinDistanceSquared(squared,threatDistance){
     d=length(delta);if d<threatDistance||(d==threatDistance&&candidate<threatId){threatDistance=d;threatDirection=-delta;threatId=candidate;}
    }
    if nearby&&withinDistanceSquared(squared,bodyRadius(kind)+bodyRadius(other.info.x)+.6){
     if d<0.0{d=length(delta);}let gap=d-bodyRadius(kind)-bodyRadius(other.info.x)-.1;
     if gap<.5{
     var away=safeNormal(delta);
     if d<.0001{let angle=random(min(id,candidate)^(max(id,candidate)*0x517cc1b7u))*6.2831853;away=vec2(cos(angle),sin(angle))*select(-1.0,1.0,id<candidate);}
     // Keep the deepest contacts, sorted by gap and stable ID instead of scatter order.
     var slot=0u;for(;slot<constraintCount;slot++){let c=constraints[slot];if gap<c.z||(gap==c.z&&candidate<u32(c.w)){break;}}
     if slot<16u{let end=min(constraintCount,15u);for(var n=end;n>slot;n--){constraints[n]=constraints[n-1u];followingSpeed[n]=followingSpeed[n-1u];}constraints[slot]=vec4(away,gap,f32(candidate));followingSpeed[slot]=max(0.0,-dot(other.motion.xy,away));constraintCount=min(16u,constraintCount+1u);}
     }
    }
    if search&&other.info.y!=a.info.y&&withinDistanceSquared(squared,candidateDistance){
     if d<0.0{d=length(delta);}if d<candidateDistance||(d==candidateDistance&&candidate<candidateId){candidateDistance=d;candidateDirection=-delta;candidateId=candidate;}
    }
   }
  }}
  // A live target keeps ownership until a replacement is substantially nearer.
  if (enemyId>=frame.counts.x&&enemyId!=participantId)||candidateDistance<retainedDistance*.75{enemyId=candidateId;distance=candidateDistance;toEnemy=candidateDirection;}
  if ranged&&search&&enemyId>=frame.counts.x&&enemyId!=participantId{
   let columns=u32(ceil(sqrt(f32(frame.counts.x))));let row=i32(id/columns);let column=i32(select((columns+1u)/2u,(columns-1u)/2u,a.info.y==1u));let step=select(1,-1,a.info.y==1u);
   // A few adjacent frontline slots keep a dead rank from hiding every living target behind it.
   for(var dz=-1;dz<=1;dz++){for(var depth=0;depth<3;depth++){
    let q=vec2i(column+depth*step,row+dz);if any(q<vec2i(0))||q.x>=i32(columns){continue;}let candidate=u32(q.y)*columns+u32(q.x);if candidate>=frame.counts.x{continue;}
    let other=source[candidate];let delta=other.pose.xz-position;let d=length(delta);
    if other.motion.z>0.0&&other.info.y!=a.info.y&&(d<distance||(d==distance&&candidate<enemyId)){enemyId=candidate;toEnemy=delta;distance=d;}
   }}
  }
  if enemyId>=frame.counts.x&&enemyId!=participantId&&squad.stamp.x>0u&&squad.center.w>=4.0{
   if squad.stamp.z==participantId&&participantAvailable(a.info.y){enemyId=participantId;toEnemy=frame.participantPose.xz-position;distance=length(toEnemy);}
   else if squad.stamp.z<frame.counts.x{let other=source[squad.stamp.z];if other.motion.z>0.0&&other.info.y!=a.info.y{enemyId=squad.stamp.z;toEnemy=other.pose.xz-position;distance=length(toEnemy);}}
  }
  if participantAvailable(a.info.y){let delta=frame.participantPose.xz-position;let d=length(delta);if d<distance*.75&&d<select(12.0,72.0,ranged){enemyId=participantId;distance=d;toEnemy=delta;}if d<threatDistance{threatDistance=d;threatDirection=delta;}}
  var attackReach=select(reach(kind),72.0,ranged);
  if !ranged&&enemyId<frame.counts.x{attackReach+=max(0.0,bodyRadius(source[enemyId].info.x)-.64);}
  a.info.z=enemyId;if visits>0u{atomicAdd(&simulationStats[26],visits);}
  var goal=toEnemy;if enemyId>=frame.counts.x&&enemyId!=participantId{goal=vec2(select(1.0,-1.0,a.info.y==1u)*frame.battle.z*.75+frame.battle.x-position.x,0.0);}
  let hasTarget=enemyId<frame.counts.x||enemyId==participantId;
  let squadActive=squad.stamp.x>0u&&squad.center.w>=4.0;var formationGoal=position;var squadDirection=vec2f(0);
  if squadActive&&squad.goal.z>0.0{
   var squadForward=vec2(select(1.0,-1.0,a.info.y==1u),0);if squad.stamp.z==participantId&&participantAvailable(a.info.y){squadForward=safeNormal(frame.participantPose.xz-squad.center.xy);}else if squad.stamp.z<frame.counts.x{squadForward=safeNormal(source[squad.stamp.z].pose.xz-squad.center.xy);}
   formationGoal=clamp(squad.goal.xy+squadFormationOffset(id,a.info.y,squadForward),frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));squadDirection=safeNormal(formationGoal-position);
  }
  let closeCombat=hasTarget&&distance<=max(4.0,reach(kind)*1.2);var withdrawing=squadActive&&squad.goal.z==2.0;
  if withdrawing{let escape=clamp(position+squadDirection*.5,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));if length(escape-position)<.2{withdrawing=false;}}
  let orderedTravel=squadActive&&!closeCombat&&length(formationGoal-position)>3.0&&(squad.goal.z==1.0||(!ranged&&squad.goal.z==3.0));
  if orderedTravel&&!ranged{goal=formationGoal-position;}
  let preferred=min(48.0+8.0*random(id^frame.counts.z^0x6c8e9cf5u),max(4.5,frame.battle.z*.6));
  let inner=preferred-min(8.0,preferred*.2);let outer=preferred+min(4.0,preferred*.12);let pressureRange=min(8.0,preferred*.25);
  let pressure=select(toEnemy,threatDirection,threatDistance<pressureRange&&distance>=pressureRange);
  let retreatDirection=-safeNormal(pressure);var retreat=ranged&&hasTarget&&(distance<inner||threatDistance<pressureRange);
  if retreat{
   let escape=clamp(position+retreatDirection*.5,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));
   if length(escape-position)<.2{retreat=false;}
  }
  if dot(goal,goal)>.0001{a.pose.w=turnToward(a.pose.w,goal,dt);}
  let facing=vec2(sin(a.pose.w),-cos(a.pose.w));
  if a.info.w==2u {
   let before=a.motion.w;a.motion.w+=dt;
   if a.clocks.w<.5&&before<attackTime(kind)&&a.motion.w>=attackTime(kind){
    a.clocks.w=1.0;
    if (enemyId<frame.counts.x||enemyId==participantId)&&distance<=attackReach*1.08{
     let facing=vec2(sin(a.pose.w),-cos(a.pose.w));if dot(facing,safeNormal(toEnemy))>.9{
      if ranged{
       var destination=frame.participantPose.xyz+vec3(0,.8,0);if enemyId<frame.counts.x{
        let victim=source[enemyId];let flight=clamp(distance/42.0,.3,1.8);var lead=victim.motion.xy*flight;lead*=min(1.0,6.0/max(length(lead),.0001));
        let future=clamp(victim.pose.xz+lead,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));destination=vec3(future.x,groundHeight(future)+.6,future.y);
       }
       launchProjectile(a.pose.xyz+vec3(0,1.0,0),destination,a.info.y,damage(kind),1.7,0u,enemyId);
      }else if enemyId==participantId{
       if atomicLoad(&combat.data[8])==0u{atomicAdd(&combat.data[2],damage(kind));atomicAdd(&stats[43],1u);atomicAdd(&simulationStats[5],1u);atomicAdd(&simulationStats[21],damage(kind));}
      }else{damageActor(enemyId,damage(kind),false);atomicAdd(&simulationStats[5],1u);atomicAdd(&simulationStats[21],damage(kind));}
     }
    }
   }
   if a.motion.w>=attackDuration(kind){a.info.w=0u;a.motion.w=0.0;}
  }else if hasTarget&&distance<=select(attackReach*.95,outer,ranged)&&!retreat&&((!orderedTravel&&!withdrawing)||closeCombat)&&a.clocks.x<=0.0&&dot(facing,safeNormal(toEnemy))>.92 {
   a.info.w=2u;a.motion.w=0.0;a.clocks.w=0.0;a.clocks.x=cooldown(kind);
   atomicAdd(&simulationStats[4],1u);
  }else{
   let speed=select(1.666667,3.4,kind<2u);
   var amount=speed;
   if enemyId<frame.counts.x||enemyId==participantId{amount=min(speed,max(0.0,distance-attackReach*.88)*3.0);}
   var desired=facing*amount*max(0.0,dot(facing,safeNormal(goal)));
   if withdrawing{desired=squadDirection*speed;}
   else if orderedTravel{desired=squadDirection*speed;}
   else if ranged&&hasTarget{
    // Preserve target-facing windups; walking between them can move backward out of pressure.
    desired=vec2f(0);
    if retreat{
     let shortage=max(inner-distance,pressureRange-threatDistance);
     desired=retreatDirection*min(speed,max(0.0,shortage)*1.5);
    }else if distance>outer{desired=facing*min(speed,(distance-outer)*1.5)*max(0.0,dot(facing,safeNormal(toEnemy)));}
   }else if !ranged&&(!hasTarget||distance>attackReach*1.15)&&dot(goal,goal)>.0001{
    let forward=safeNormal(goal);let side=select(-1.0,1.0,(hash(id^frame.counts.z^0x12e7a9b5u)&1u)!=0u);let lane=vec2(-forward.y,forward.x)*side;
    var blocked=false;for(var n=0u;n<constraintCount;n++){
     let contact=constraints[n];let other=source[u32(contact.w)];
     if other.info.y==a.info.y&&contact.z<.25&&dot(contact.xy,forward)<-.65&&followingSpeed[n]<.2{blocked=true;}
    }
    // Continue the same lateral escape until clear; moving convoys never enter this branch.
    let entering=length(previousVelocity)<.2;let holdLane=dot(previousVelocity,lane)>.1&&abs(dot(previousVelocity,forward))<.2;
    if blocked&&(entering||holdLane){desired=lane*speed*.55;}
   }
   let change=desired-previousVelocity;let acceleration=8.0*dt;
   a.motion=vec4(previousVelocity+change*min(1.0,acceleration/max(length(change),.0001)),a.motion.zw);
  }
  var velocity=a.motion.xy;
  // A receding neighbor permits shared motion; standing/head-on contacts retain half-gap stopping.
  // The .1m body margin covers a leader stopping within one fixed step (<.077m relative travel).
  for(var iteration=0u;iteration<3u;iteration++){for(var n=0u;n<constraintCount;n++){
   let contact=constraints[n];let limit=max(0.0,contact.z)*.45/max(dt,.0001)+followingSpeed[n];let inward=dot(velocity,contact.xy);
   if inward < -limit{velocity+=contact.xy*(-limit-inward);}
  }}
  for(var n=0u;n<constraintCount;n++){let contact=constraints[n];correction+=contact.xy*max(0.0,-contact.z)*.5;}
  correction*=1.0-exp(-10.0*dt);
  if frame.participant.y==1u&&atomicLoad(&combat.data[1])>0u{
   let delta=position-frame.participantPose.xz;let distance=length(delta);
   if distance<bodyRadius(kind)+1.0{
    var away=safeNormal(delta);if distance<.0001{let angle=random(id^frame.counts.z)*6.2831853;away=vec2(cos(angle),sin(angle));}
    correction+=away*max(0.0,bodyRadius(kind)+.4-distance)*.5*(1.0-exp(-10.0*dt));
    let limit=max(0.0,distance-bodyRadius(kind)-.4)/max(dt,.0001);let inward=dot(velocity,away);if inward < -limit{velocity+=away*(-limit-inward);}
   }
  }
  let speed=select(1.666667,3.4,kind<2u);velocity*=min(1.0,speed/max(length(velocity),.0001));
  let push=correction*min(1.0,.6*dt/max(length(correction),.0001));
  let next=clamp(position+velocity*dt+push,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));
  a.motion=vec4((next-position)/max(dt,.00001),a.motion.zw);a.pose=vec4(next.x,groundHeight(next),next.y,a.pose.w);
  if a.info.w!=2u{
   let moving=length(velocity)>select(.15,.06,previousState==1u);
   if moving{a.info.w=1u;a.clocks.z+=dt*min(1.2,length(velocity)/speed);}else{a.info.w=0u;}
   a.motion.w=select(a.clocks.z,0.0,a.info.w==0u);
  }
 }
 if a.motion.z>0.0{atomicAdd(&simulationStats[a.info.y],1u);atomicAdd(&simulationStats[15],u32(a.motion.z));}else{atomicAdd(&simulationStats[2u+a.info.y],1u);}
 if length(a.motion.xy)>.05{atomicAdd(&simulationStats[13],1u);}if a.info.w==2u{atomicAdd(&simulationStats[14],1u);}
 if any(a.pose!=a.pose)||any(a.motion!=a.motion){atomicAdd(&simulationStats[12],1u);}
 result[id]=a;
 }
 workgroupBarrier();
 if lane<64u{let value=atomicLoad(&simulationStats[lane]);if value>0u{atomicAdd(&stats[lane],value);}}
}
`;

export const BATTLE_COMPACTION_WGSL =
  BATTLE_COMMON_WGSL +
  /* wgsl */ `
struct Draw {indexCount:u32,instanceCount:atomic<u32>,firstIndex:u32,baseVertex:i32,firstInstance:u32}
@group(0)@binding(0)var<storage,read>actors:array<Actor>;
@group(0)@binding(1)var<storage,read_write>visible:array<u32>;
@group(0)@binding(2)var<storage,read_write>draws:array<Draw>;
@group(0)@binding(3)var<storage,read_write>stats:array<atomic<u32>>;
@group(0)@binding(4)var<uniform>frame:Frame;
struct Models {data:array<vec4f,4>}
@group(0)@binding(5)var<uniform>models:Models;
var<workgroup> binCounts:array<atomic<u32>,12>;
var<workgroup> binOffsets:array<u32,12>;
var<workgroup> culledCount:atomic<u32>;
@compute @workgroup_size(32)fn clearDraws(@builtin(global_invocation_id)inv:vec3u){
 let id=inv.x;if id<12u{atomicStore(&draws[id].instanceCount,0u);}
 if id==9u||id==10u||id==16u||id==24u||(id>=17u&&id<=20u){atomicStore(&stats[id],0u);}
}
@compute @workgroup_size(128)fn compact(@builtin(global_invocation_id)inv:vec3u,@builtin(local_invocation_index)lane:u32){
 if lane<12u{atomicStore(&binCounts[lane],0u);}if lane==0u{atomicStore(&culledCount,0u);}workgroupBarrier();
 let id=inv.x;var survives=false;var batch=0u;var rank=0u;
 if id<frame.counts.x{
  let a=actors[id];let distance=length(a.pose.xyz-frame.eyeTime.xyz);
  var outside=distance>frame.tuning.y+4.5;let center=vec4(a.pose.xyz+vec3(0,1.3,0),1.0);
  for(var plane=0u;plane<6u;plane++){outside=outside||dot(frame.planes[plane],center)<-4.5;}
  if outside{atomicAdd(&culledCount,1u);}else{
   let lod=select(0u,1u,distance>frame.tuning.x);batch=a.info.x*2u+lod;
   let projectedHeight=models.data[a.info.x].x*frame.tuning.w/max((frame.viewProjection*center).w,.01);
   if models.data[a.info.x].y>.5&&projectedHeight<frame.sun.w{batch=8u+a.info.x;}
   rank=atomicAdd(&binCounts[batch],1u);survives=true;
  }
 }
 workgroupBarrier();
 if lane<12u{
  let count=atomicLoad(&binCounts[lane]);binOffsets[lane]=0u;
  if count>0u{
   binOffsets[lane]=atomicAdd(&draws[lane].instanceCount,count);
   let lod=select(lane%2u,1u,lane>=8u);let kind=select(lane/2u,lane-8u,lane>=8u);
   atomicAdd(&stats[9u+lod],count);atomicAdd(&stats[17u+kind],count);atomicAdd(&stats[24],count*(draws[lane].indexCount/3u));
  }
 }
 if lane==0u{let count=atomicLoad(&culledCount);if count>0u{atomicAdd(&stats[16],count);}}
 workgroupBarrier();
 if survives{visible[batch*frame.counts.y+binOffsets[batch]+rank]=id;}
}
`;

export const BATTLE_RENDER_WGSL =
  BATTLE_COMMON_WGSL +
  /* wgsl */ `
${MMORPG_FOG_WGSL}
struct Draw {data:vec4u}
struct Class {clips:array<vec4f,4>,dimensions:vec4u,settings:vec4f,spriteClips:array<vec4f,4>,spriteSize:vec4u,spriteBounds:vec4f}
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>actors:array<Actor>;
@group(0)@binding(2)var<storage,read>visible:array<u32>;
@group(0)@binding(3)var<uniform>draw:Draw;
@group(1)@binding(0)var vatPosition:texture_2d<f32>;
@group(1)@binding(1)var vatNormal:texture_2d<f32>;
@group(1)@binding(2)var materialTexture:texture_2d_array<f32>;
@group(1)@binding(3)var materialSampler:sampler;
@group(1)@binding(4)var<uniform>kind:Class;
@group(1)@binding(5)var spriteColor:texture_2d_array<f32>;
@group(1)@binding(6)var spriteNormal:texture_2d_array<f32>;
override impostorSampleCount:u32=4u;
struct Input {@location(0)uv:vec2f,@location(1)vertex:u32,@location(2)material:u32}
struct Output {@builtin(position)clip:vec4f,@location(0)normal:vec3f,@location(1)uv:vec2f,@location(2)world:vec3f,@location(3)@interpolate(flat)material:u32,@location(4)@interpolate(flat)team:u32,@location(5)@interpolate(flat)hurt:f32}
@vertex fn vertexMain(input:Input,@builtin(instance_index)instance:u32)->Output {
 let a=actors[visible[draw.data.x+instance]];let state=min(a.info.w,3u);let animation=kind.clips[state];
 let clock=select(min(a.motion.w,animation.x),a.motion.w%animation.x,animation.w>.5);
 let progress=clamp(clock/animation.x,0.0,1.0)*max(animation.z-1.0,0.0);let first=u32(animation.y)+u32(floor(progress));let second=min(first+1u,u32(animation.y+animation.z-1.0));let blend=fract(progress);
 let p=mix(textureLoad(vatPosition,vec2i(i32(input.vertex),i32(first)),0).xyz,textureLoad(vatPosition,vec2i(i32(input.vertex),i32(second)),0).xyz,blend);
 let n=mix(textureLoad(vatNormal,vec2i(i32(input.vertex),i32(first)),0).xyz,textureLoad(vatNormal,vec2i(i32(input.vertex),i32(second)),0).xyz,blend);
 let yaw=3.141592654-a.pose.w;let c=cos(yaw);let s=sin(yaw);let world=a.pose.xyz+vec3(c*p.x+s*p.z,p.y,-s*p.x+c*p.z);
 var output:Output;output.clip=frame.viewProjection*vec4(world,1.0);output.normal=normalize(vec3(c*n.x+s*n.z,n.y,-s*n.x+c*n.z));output.uv=input.uv;output.world=world;output.material=input.material;output.team=a.info.y;output.hurt=select(0.0,a.clocks.y,a.motion.z>0.0);return output;
}
@fragment fn fragmentMain(input:Output,@builtin(front_facing)front:bool)->@location(0)vec4f {
 let texel=textureSample(materialTexture,materialSampler,input.uv,i32(input.material));
 let normal=normalize(select(-input.normal,input.normal,front));let light=max(dot(normal,normalize(frame.sun.xyz)),0.0);
 let faction=select(vec3(.3,.55,1.0),vec3(1.0,.27,.16),input.team==1u);
 var color=mix(texel.rgb,texel.rgb*faction,.12)*(frame.ambient.rgb+frame.sunColor.rgb*light);
 color=mix(color,vec3(2.0,.1,.04),clamp(input.hurt*2.0,0.0,.45));
 let scale=clamp(frame.ambient.w,1.0,100.0);let fog=mmoFog(frame.eyeTime.y/scale,(input.world-frame.eyeTime.xyz)/scale);color=mix(color,mmoFogColor,fog);
 return vec4(color*${SCENE_LINEAR_SCALE},1.0);
}
struct SpriteOutput {
 @builtin(position)clip:vec4f,@location(0)uv:vec2f,@location(1)world:vec3f,
 @location(2)@interpolate(flat)tile:vec2f,@location(3)@interpolate(flat)layer:u32,
 @location(4)@interpolate(flat)rotation:vec2f,@location(5)@interpolate(flat)team:u32,@location(6)@interpolate(flat)hurt:f32,
}
@vertex fn vertexImpostor(@builtin(vertex_index)vertex:u32,@builtin(instance_index)instance:u32)->SpriteOutput{
 let a=actors[visible[draw.data.x+instance]];let corners=array<vec2f,4>(vec2(0,0),vec2(1,0),vec2(0,1),vec2(1,1));let uv=corners[vertex];
 let toward=frame.eyeTime.xz-a.pose.xz;
 let right=normalize(vec3(frame.viewProjection[0][0],frame.viewProjection[1][0],frame.viewProjection[2][0]));
 let up=normalize(vec3(frame.viewProjection[0][1],frame.viewProjection[1][1],frame.viewProjection[2][1]));
 let horizontal=(uv.x*2.0-1.0)*kind.spriteBounds.x;
 let world=a.pose.xyz+vec3(0,kind.spriteBounds.y,0)+right*horizontal+up*((1.0-uv.y)*(kind.spriteBounds.z-kind.spriteBounds.y));
 let azimuth=acos(clamp(toward.y*inverseSqrt(max(dot(toward,toward),1e-8)),-1.0,1.0))*select(1.0,-1.0,toward.x<0.0);
 let rotation=3.141592654-a.pose.w;let theta=azimuth-rotation;
 let layer=u32(floor(fract(theta/6.2831853+1.0)*f32(kind.spriteSize.z)+.5))%kind.spriteSize.z;
 let animation=kind.spriteClips[min(a.info.w,3u)];let clock=select(min(a.motion.w,animation.x),a.motion.w%animation.x,animation.w>.5);
 let frameIndex=u32(animation.y)+u32(round(clamp(clock/animation.x,0.0,1.0)*max(animation.z-1.0,0.0)));
 let columns=kind.spriteSize.x/kind.spriteSize.w;
 var output:SpriteOutput;output.clip=frame.viewProjection*vec4(world,1);output.uv=uv;output.world=world;
 output.tile=vec2f(vec2u(frameIndex%columns,frameIndex/columns))*f32(kind.spriteSize.w);output.layer=layer;
 output.rotation=vec2(cos(rotation),sin(rotation));output.team=a.info.y;output.hurt=select(0.0,a.clocks.y,a.motion.z>0.0);return output;
}
@fragment fn fragmentImpostor(input:SpriteOutput)->@location(0)vec4f{
 let tileSize=f32(kind.spriteSize.w);let footprint=min(vec2(4),max(vec2(1),fwidth(input.uv)*tileSize));
 let offsets=array<vec2f,4>(vec2(-.25,-.25),vec2(.25,-.25),vec2(-.25,.25),vec2(.25,.25));
 var albedo=vec4f(0);var encodedNormal=vec4f(0);
 for(var sample=0u;sample<4u;sample++){
  let pixel=clamp(input.uv*tileSize+offsets[sample]*footprint,vec2(.5),vec2(tileSize-.5));
  let coordinate=(input.tile+pixel)/vec2f(kind.spriteSize.xy);
  albedo+=textureSampleLevel(spriteColor,materialSampler,coordinate,i32(input.layer),0.0)*.25;
  encodedNormal+=textureSampleLevel(spriteNormal,materialSampler,coordinate,i32(input.layer),0.0)*.25;
 }
 if albedo.a<select(.5,.01,impostorSampleCount>1u){discard;}
 let decoded=encodedNormal.xyz/max(encodedNormal.a,.001)*2.0-1.0;let local=decoded*inverseSqrt(max(dot(decoded,decoded),1e-8));
 let normal=vec3(input.rotation.x*local.x+input.rotation.y*local.z,local.y,-input.rotation.y*local.x+input.rotation.x*local.z);
 let faction=select(vec3(.3,.55,1),vec3(1,.27,.16),input.team==1u);
 let texel=albedo.rgb/max(albedo.a,.001);let light=max(dot(normal,normalize(frame.sun.xyz)),0.0);
 var color=mix(texel,texel*faction,.12)*(frame.ambient.rgb+frame.sunColor.rgb*light);
 color=mix(color,vec3(2,.1,.04),clamp(input.hurt*2.0,0.0,.45));
 let scale=clamp(frame.ambient.w,1.0,100.0);let fog=mmoFog(frame.eyeTime.y/scale,(input.world-frame.eyeTime.xyz)/scale);color=mix(color,mmoFogColor,fog);
 return vec4(color*${SCENE_LINEAR_SCALE},select(1.0,albedo.a,impostorSampleCount>1u));
}
`;
