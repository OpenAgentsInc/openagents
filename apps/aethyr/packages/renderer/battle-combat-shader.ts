export const BATTLE_PROJECTILE_CAPACITY = 16_384;
export const BATTLE_SIEGE_COUNT = 8;
export const BATTLE_COMBAT_HEADER_BYTES = 96;
export const BATTLE_PROJECTILE_OFFSET =
  BATTLE_COMBAT_HEADER_BYTES + BATTLE_SIEGE_COUNT * 32;
export const BATTLE_SQUAD_OFFSET =
  BATTLE_PROJECTILE_OFFSET + BATTLE_PROJECTILE_CAPACITY * (64 + 4 + 4);
export const BATTLE_COMBAT_BYTES =
  BATTLE_SQUAD_OFFSET + BATTLE_SQUAD_CAPACITY * BATTLE_SQUAD_STRIDE;

export const BATTLE_COMBAT_TYPES =
  BATTLE_SQUAD_TYPES +
  /* wgsl */ `
struct Projectile { pose:vec4f, velocity:vec4f, impact:vec4f, info:vec4u }
struct Siege {pose:vec4f,state:vec4f}
struct Combat {
 pose:vec4f,aim:vec4f,data:array<atomic<u32>,16>,siege:array<Siege,8>,
 projectiles:array<Projectile,16384>,occupied:array<atomic<u32>,16384>,visible:array<u32,16384>,
 squads:array<Squad,65536>,
}
`;

/** Segment tests return 2 when the finite segment has no intersection. */
export const BATTLE_SWEEP_WGSL = /* wgsl */ `
fn sphereSegmentHit(origin:vec3f,delta:vec3f,center:vec3f,radius:f32)->f32{
 let offset=origin-center;let c=dot(offset,offset)-radius*radius;if c<=0.0{return 0.0;}
 let a=dot(delta,delta);if a<1e-12{return 2.0;}let b=dot(offset,delta);let discriminant=b*b-a*c;if discriminant<0.0{return 2.0;}
 let t=(-b-sqrt(max(0.0,discriminant)))/a;return select(2.0,t,t>=0.0&&t<=1.0);
}
fn capsuleSegmentHit(origin:vec3f,delta:vec3f,feet:vec3f,height:f32,radius:f32)->f32{
 let low=feet+vec3(0,radius,0);let high=feet+vec3(0,max(radius,height-radius),0);
 var best=min(sphereSegmentHit(origin,delta,low,radius),sphereSegmentHit(origin,delta,high,radius));
 let offset=origin.xz-feet.xz;let c=dot(offset,offset)-radius*radius;
 if c<=0.0&&origin.y>=low.y&&origin.y<=high.y{return 0.0;}
 let a=dot(delta.xz,delta.xz);if a<1e-12{return best;}let b=dot(offset,delta.xz);let discriminant=b*b-a*c;
 if discriminant>=0.0{
  let root=sqrt(max(0.0,discriminant));
  for(var i=0u;i<2u;i++){let t=(-b+select(-root,root,i==1u))/a;let y=origin.y+delta.y*t;if t>=0.0&&t<=1.0&&y>=low.y&&y<=high.y{best=min(best,t);}}
 }return best;
}
fn triangleSegmentHit(origin:vec3f,delta:vec3f,a:vec3f,b:vec3f,c:vec3f)->f32{
 let edge1=b-a;let edge2=c-a;let p=cross(delta,edge2);let determinant=dot(edge1,p);if abs(determinant)<1e-9{return 2.0;}
 let inverse=1.0/determinant;let offset=origin-a;let u=dot(offset,p)*inverse;if u < -.00001||u>1.00001{return 2.0;}
 let q=cross(offset,edge1);let v=dot(delta,q)*inverse;if v < -.00001||u+v>1.00001{return 2.0;}
 let t=dot(edge2,q)*inverse;return select(2.0,t,t>=0.0&&t<=1.0);
}
`;

/** Extra combat uses the same bounded grid and delayed actor-damage buffers. */
export const BATTLE_COMBAT_WGSL =
  BATTLE_COMBAT_TYPES +
  BATTLE_SWEEP_WGSL +
  /* wgsl */ `
@group(0)@binding(3)var<storage,read_write>combat:Combat;
const participantId=0xfffffffeu;
fn rangedRole(id:u32)->bool {
 let columns=u32(ceil(sqrt(f32(frame.counts.x))));
 let fraction=abs((f32(id%columns)+.5)/f32(columns)-.5)*2.0;
 return (id%4u)==0u&&fraction>.35;
}
fn participantAvailable(team:u32)->bool{return frame.participant.y==1u&&atomicLoad(&combat.data[1])>0u&&frame.participant.x!=team;}
fn damageActor(id:u32,amount:u32,local:bool){
 atomicAdd(&damageOut[id].amount,amount);if local{atomicAdd(&damageOut[id].local,amount);}
}
fn launchProjectile(origin:vec3f,destination:vec3f,team:u32,amount:u32,radius:f32,kind:u32,targetId:u32)->bool {
 let distance=select(length(destination.xz-origin.xz),length(destination-origin),kind==2u);
 let flight=clamp(distance/select(42.0,33.0,kind==1u),.3,3.0);
 let gravity=select(18.0,0.0,kind==2u);let velocity=(destination-origin)/flight+vec3(0.0,.5*gravity*flight,0.0);
 for(var attempt=0u;attempt<8u;attempt++){
  let slot=atomicAdd(&combat.data[0],1u)%16384u;
  if atomicCompareExchangeWeak(&combat.occupied[slot],0u,1u).exchanged{
   combat.projectiles[slot]=Projectile(vec4(origin,0.0),vec4(velocity,flight),vec4(destination,radius),vec4u(team,amount,kind,targetId));
   if kind==1u{atomicAdd(&stats[39],1u);}else{atomicAdd(&stats[34],1u);}return true;
  }
 }
 atomicAdd(&stats[38],1u);return false;
}
struct SiegeTarget {id:u32,point:vec3f}
fn siegeTarget(s:Siege,id:u32)->SiegeTarget {
 let team=u32(s.state.z);let count=frame.counts.x;
 let offset=hash(id^atomicLoad(&stats[11]))%count;
 var best=0xffffffffu;var bestScore=-1e9;var bestDistance=1e9;
 // Stratified candidates and bounded blast queries keep siege planning independent of army size.
 for(var sample=0u;sample<24u;sample++){
  let candidate=(offset+u32(f32(sample)*f32(count)/24.0))%count;
  let actor=source[candidate];if actor.motion.z<=0.0||actor.info.y==team{continue;}
  let center=cellPosition(actor.pose.xz);var score=0.0;
  for(var z=-2;z<=2;z++){for(var x=-2;x<=2;x++){
   let q=center+vec2i(x,z);if any(q<vec2i(0))||any(q>=vec2i(battleGridDimension)){continue;}
   let c=cellIndex(q);let n=min(atomicLoad(&cells[c].count),64u);
   for(var i=0u;i<n;i++){
    let other=source[cells[c].members[i]];
    if other.motion.z<=0.0||distance(other.pose.xz,actor.pose.xz)>=8.0{continue;}
    score+=select(select(1.0,3.0,other.info.w==2u),-.5,other.info.y==team);
   }
  }}
  let d=distance(actor.pose.xz,s.pose.xz);
  if score>bestScore||(score==bestScore&&(d<bestDistance||(d==bestDistance&&candidate<best))){best=candidate;bestScore=score;bestDistance=d;}
 }
 if best==0xffffffffu{return SiegeTarget(best,vec3f(0));}
 let actor=source[best];let flight=clamp(distance(actor.pose.xz,s.pose.xz)/33.0,.3,3.0);
 let lead=actor.motion.xy*flight;let boundedLead=lead*min(1.0,6.0/max(length(lead),.001));
 let point=clamp(actor.pose.xz+boundedLead,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));
 return SiegeTarget(best,vec3(point.x,groundHeight(point)+.6,point.y));
}
// Reserve the complete cast before publishing acceptance; failed batches release every slot.
fn scheduleMeteorShower()->bool {
 let origin=frame.participantPose.xyz+vec3(0,1.0,0);let aimed=frame.participantAim.xyz;
 let aimedDistance=distance(origin,aimed);if (u32(frame.participantAim.w)&2u)==0u||!(aimedDistance>=.01&&aimedDistance<=80.001){return false;}
 var slots:array<u32,6>;var reserved=0u;
 for(var meteor=0u;meteor<6u;meteor++){
  var acquired=false;
  for(var attempt=0u;attempt<8u;attempt++){
   let slot=atomicAdd(&combat.data[0],1u)%16384u;
   if atomicCompareExchangeWeak(&combat.occupied[slot],0u,1u).exchanged{slots[meteor]=slot;reserved++;acquired=true;break;}
  }
  if !acquired{for(var j=0u;j<reserved;j++){atomicStore(&combat.occupied[slots[j]],0u);}atomicAdd(&stats[38],1u);return false;}
 }
 var seed=hash(frame.counts.z^frame.participant.z^atomicLoad(&stats[11]));var roll=0u;
 for(var die=0u;die<40u;die++){seed=hash(seed+die+0x517cc1b7u);roll+=1u+seed%6u;}
 let epoch=atomicLoad(&combat.data[11]);
 for(var meteor=0u;meteor<6u;meteor++){
  let angle=random(seed^(meteor*0x9e3779b9u))*6.2831853;let radius=sqrt(random(seed^(meteor*0x85ebca6bu)^0xb123u))*6.0;
  let point=aimed.xz+vec2(cos(angle),sin(angle))*radius;let destination=vec3(point.x,groundHeight(point),point.y);let launchOrigin=destination+vec3(-12,80,-8);
  let delay=150u+u32(round(f32(meteor)*.13*60.0));
  combat.projectiles[slots[meteor]]=Projectile(vec4(launchOrigin,-f32(delay+1u)/60.0),vec4(destination-launchOrigin,1.0),vec4(destination,4.0),vec4u(frame.participant.x,roll,3u,epoch));
 }
 atomicAdd(&stats[55],1u);return true;
}
fn applyBlast(point:vec3f,radius:f32,team:u32,amount:u32,kind:u32){
 let center=cellPosition(point.xz);let extent=i32(ceil(radius/battleCellSize));var hits=0u;
 for(var z=-extent;z<=extent;z++){for(var x=-extent;x<=extent;x++){
  let q=center+vec2i(x,z);if any(q<vec2i(0))||any(q>=vec2i(battleGridDimension)){continue;}
  let c=cellIndex(q);let n=min(atomicLoad(&cells[c].count),64u);
  for(var i=0u;i<n;i++){
   let id=cells[c].members[i];let a=source[id];if a.motion.z<=0.0||a.info.y==team{continue;}
   let d=distance(a.pose.xyz+vec3(0,.6,0),point);
   if d<radius{let value=select(max(1u,u32(f32(amount)*(1.0-d/radius))),amount,kind==3u);damageActor(id,value,kind==2u||kind==3u);hits++;atomicAdd(&stats[21],value);if kind==2u||kind==3u{atomicAdd(&stats[47],value);}}
  }
 }}
 if participantAvailable(team)&&atomicLoad(&combat.data[8])==0u&&distance(frame.participantPose.xyz+vec3(0,.8,0),point)<radius{
  atomicAdd(&combat.data[2],amount);atomicAdd(&stats[43],1u);atomicAdd(&stats[21],amount);hits++;
 }
 if hits>0u{atomicAdd(&stats[5],hits);atomicAdd(&stats[37],hits);if kind==1u{atomicAdd(&stats[40],hits);}if kind==3u{atomicAdd(&stats[59],hits);}}
}
@compute @workgroup_size(8)fn participants(@builtin(global_invocation_id)inv:vec3u){
 let id=inv.x;let dt=frame.battle.w;
 if id==0u{
  combat.pose=frame.participantPose;combat.aim=frame.participantAim;
  let incoming=atomicExchange(&combat.data[2],0u);var hp=atomicLoad(&combat.data[1]);
  let joined=frame.participant.y==1u;
  if joined&&atomicLoad(&combat.data[7])==0u{atomicStore(&combat.data[8],120u);}atomicStore(&combat.data[7],u32(joined));
  let protection=atomicLoad(&combat.data[8]);if protection>0u{atomicStore(&combat.data[8],protection-1u);}
  if joined&&protection==0u{hp=hp-min(hp,incoming);atomicStore(&combat.data[1],hp);}
  var cooldown=atomicLoad(&combat.data[9]);if cooldown>0u{cooldown--;atomicStore(&combat.data[9],cooldown);}
  if joined&&hp>0u&&frame.participant.z>atomicLoad(&combat.data[12]){
   atomicStore(&combat.data[12],frame.participant.z);
   if cooldown==0u&&(frame.participant.w!=2u||scheduleMeteorShower()){atomicStore(&combat.data[3],frame.participant.z);atomicStore(&combat.data[4],frame.participant.w);atomicStore(&combat.data[5],0u);atomicStore(&combat.data[6],0u);atomicStore(&combat.data[9],select(92u,252u,frame.participant.w==2u));atomicAdd(&stats[46],1u);}
  }
  let ticks=atomicAdd(&combat.data[5],1u)+1u;let kind=atomicLoad(&combat.data[4]);let impact=select(21u,42u,kind==1u);
  if joined&&hp>0u&&kind!=2u&&atomicLoad(&combat.data[6])==0u&&ticks>=impact&&atomicLoad(&combat.data[9])>0u{
   atomicStore(&combat.data[6],1u);
   if kind==1u{
    let origin=frame.participantPose.xyz+vec3(0,1.0,0);let delta=frame.participantAim.xyz-origin;
    let destination=origin+delta*min(1.0,80.0/max(length(delta),.001));
    launchProjectile(origin,destination,frame.participant.x,300u,3.5,2u,participantId);
   }else{
    let p=frame.participantPose.xyz;let c=cellPosition(p.xz);let facing=vec2(sin(frame.participantPose.w),-cos(frame.participantPose.w));
    for(var z=-1;z<=1;z++){for(var x=-1;x<=1;x++){
     let q=c+vec2i(x,z);if any(q<vec2i(0))||any(q>=vec2i(battleGridDimension)){continue;}
     let cell=cellIndex(q);let n=min(atomicLoad(&cells[cell].count),64u);
     for(var i=0u;i<n;i++){let destination=cells[cell].members[i];let a=source[destination];let delta=a.pose.xz-p.xz;let d=length(delta);
      if a.motion.z>0.0&&a.info.y!=frame.participant.x&&d<1.8+max(0.0,bodyRadius(a.info.x)-.64)&&abs(a.pose.y-p.y)<2.0&&dot(facing,safeNormal(delta))>.5{damageActor(destination,300u,true);atomicAdd(&stats[47],300u);atomicAdd(&stats[49],1u);atomicAdd(&stats[5],1u);atomicAdd(&stats[21],300u);}
     }
    }}
   }
  }
  atomicStore(&stats[57],select(0u,atomicLoad(&combat.data[9]),kind==2u));
  atomicStore(&stats[58],0u);
  atomicStore(&stats[53],atomicLoad(&combat.data[12]));atomicStore(&stats[52],atomicLoad(&combat.data[11]));atomicStore(&stats[41],hp);atomicStore(&stats[42],u32(hp==0u));atomicStore(&stats[45],atomicLoad(&combat.data[3]));atomicStore(&stats[51],atomicLoad(&combat.data[8]));
 }
 if id<8u{
  var s=combat.siege[id];s.state.x=max(0.0,s.state.x-dt);s.state.y+=dt;
  if s.state.x<=0.0&&frame.counts.x>0u{
   let choice=siegeTarget(s,id);s.state.x=.5;
   if choice.id!=0xffffffffu{
    let delta=choice.point-s.pose.xyz;s.pose.w=atan2(delta.x,delta.z);
    if launchProjectile(s.pose.xyz+vec3(0,3.0,0),choice.point,u32(s.state.z),180u,8.0,1u,choice.id){s.state.x=6.0;s.state.y=0.0;}
   }
  }combat.siege[id]=s;
 }
}
fn heightGridPoint(position:vec2f)->vec2f{
 let resolution=u32(frame.ground.w);let local=position-frame.ground.xy;
 if frame.counts.w==1u{let magnitude=abs(local);let scaled=select(magnitude/2.0,vec2(64.0)+(magnitude-vec2(128.0))/24.0,magnitude>vec2(128.0));return clamp(vec2(128.0)+sign(local)*scaled,vec2(0),vec2(256));}
 return clamp(local/frame.ground.z+vec2(.5),vec2(0),vec2(1))*f32(resolution-1u);
}
fn heightWorldPoint(index:vec2u)->vec3f{
 let n=u32(frame.ground.w);var horizontal=(vec2f(index)/f32(n-1u)-vec2(.5))*frame.ground.z+frame.ground.xy;
 if frame.counts.w==1u{let local=vec2f(index)-vec2(128.0);let magnitude=abs(local);horizontal=frame.ground.xy+sign(local)*select(magnitude*2.0,vec2(128.0)+(magnitude-vec2(64.0))*24.0,magnitude>vec2(64.0));}
 return vec3(horizontal.x,heights[index.y*n+index.x]+.12,horizontal.y);
}
fn groundSegmentHit(origin:vec3f,delta:vec3f)->f32{
 if origin.y<=groundHeight(origin.xz)+.12{return 0.0;}
 let n=u32(frame.ground.w);let a=heightGridPoint(origin.xz);let b=heightGridPoint(origin.xz+delta.xz);
 let low=min(vec2u(floor(min(a,b))),vec2u(n-2u));let high=min(vec2u(floor(max(a,b))),vec2u(n-2u));var best=2.0;
 // A local bolt travels at most .7m/tick. Even the smallest supported uniform
 // field (16m/1024 cells) needs at most 46 cells/axis; native terrain needs two.
 for(var z=0u;z<48u;z++){let iz=low.y+z;if iz>high.y{break;}
  for(var x=0u;x<48u;x++){let ix=low.x+x;if ix>high.x{break;}
   let p=vec2u(ix,iz);let p00=heightWorldPoint(p);let p10=heightWorldPoint(p+vec2u(1,0));let p01=heightWorldPoint(p+vec2u(0,1));let p11=heightWorldPoint(p+vec2u(1,1));
   best=min(best,min(triangleSegmentHit(origin,delta,p00,p10,p01),triangleSegmentHit(origin,delta,p11,p01,p10)));
  }
 }
 // Outside the finite field, height sampling clamps to its edge rather than
 // leaving a collision hole. Refine that edge-height crossing separately.
 if best>1.0&&(origin+delta).y<=groundHeight(origin.xz+delta.xz)+.12{
  var lo=0.0;var hi=1.0;for(var i=0u;i<10u;i++){let middle=(lo+hi)*.5;let p=origin+delta*middle;if p.y<=groundHeight(p.xz)+.12{hi=middle;}else{lo=middle;}}best=hi;
 }return best;
}
fn localProjectileHit(origin:vec3f,delta:vec3f,team:u32)->f32{
 var best=groundSegmentHit(origin,delta);
 let low=cellPosition(min(origin.xz,origin.xz+delta.xz)-vec2(battleMaxBodyRadius));let high=cellPosition(max(origin.xz,origin.xz+delta.xz)+vec2(battleMaxBodyRadius));
 // <=.7m swept motion plus twice the largest 1.33m body radius spans at most 3x3 four-metre cells.
 for(var z=0;z<3;z++){let iz=low.y+z;if iz>high.y{break;}for(var x=0;x<3;x++){
  let ix=low.x+x;if ix>high.x{break;}let cell=cellIndex(vec2i(ix,iz));let count=min(atomicLoad(&cells[cell].count),64u);
  for(var i=0u;i<count;i++){let a=source[cells[cell].members[i]];if a.motion.z<=0.0||a.info.y==team{continue;}
   var height=select(1.8,3.3,a.info.x==3u);if a.info.x==2u{height=1.3;}
   best=min(best,capsuleSegmentHit(origin,delta,a.pose.xyz,height,bodyRadius(a.info.x)));
  }
 }}return best;
}
@compute @workgroup_size(128)fn projectiles(@builtin(global_invocation_id)inv:vec3u){
 let id=inv.x;if id>=16384u||atomicLoad(&combat.occupied[id])==0u{return;}
 var p=combat.projectiles[id];let dt=frame.battle.w;let before=p.pose.xyz;
 if p.info.z==4u{
  p.pose.w+=dt;if p.pose.w>=.4{atomicStore(&combat.occupied[id],0u);}else{combat.projectiles[id]=p;atomicAdd(&stats[35],1u);atomicAdd(&stats[58],1u);}return;
 }
 if p.info.z==3u{
  if p.pose.w<0.0{
   if frame.participant.y==0u||atomicLoad(&combat.data[1])==0u||p.info.w!=atomicLoad(&combat.data[11]){atomicStore(&combat.occupied[id],0u);return;}
   p.pose.w+=dt;
   if p.pose.w>=-.0001{p.pose.w=0.0;atomicAdd(&stats[34],1u);}
   combat.projectiles[id]=p;atomicAdd(&stats[35],1u);atomicAdd(&stats[58],1u);return;
  }
  p.pose.w=min(1.0,p.pose.w+dt);p.pose=vec4(p.impact.xyz+p.velocity.xyz*(p.pose.w-1.0),p.pose.w);
  if p.pose.w>=.9999{
   p.pose=vec4(p.impact.xyz,0.0);
   let point=p.impact.xz;let dx=(groundHeight(point+vec2(.1,0))-groundHeight(point-vec2(.1,0)))*5.0;let dz=(groundHeight(point+vec2(0,.1))-groundHeight(point-vec2(0,.1)))*5.0;
   p.velocity=vec4(normalize(vec3(-dx,1,-dz)),p.velocity.w);applyBlast(p.impact.xyz,4.0,p.info.x,p.info.y,3u);p.info.z=4u;
   atomicAdd(&stats[36],1u);atomicAdd(&stats[56],1u);
   atomicStore(&combat.data[13],bitcast<u32>(p.impact.x));atomicStore(&combat.data[14],bitcast<u32>(p.impact.y));atomicStore(&combat.data[15],bitcast<u32>(p.impact.z));atomicStore(&combat.data[10],atomicLoad(&stats[11])+1u);
  }
  combat.projectiles[id]=p;atomicAdd(&stats[35],1u);atomicAdd(&stats[58],1u);return;
 }

 let local=p.info.z==2u;let acceleration=select(vec3(0,-9.0*dt*dt,0),vec3f(0),local);
 let previousAge=p.pose.w;p.pose=vec4(before+p.velocity.xyz*dt+acceleration,p.pose.w+dt);p.velocity.y-=select(18.0,0.0,local)*dt;
 if local{
  let fraction=clamp((p.velocity.w-previousAge)/dt,0.0,1.0);let delta=(p.pose.xyz-before)*min(1.0,fraction);let t=localProjectileHit(before,delta,p.info.x);
  if t<=1.0||p.pose.w>=p.velocity.w{
   let point=before+delta*min(t,1.0);applyBlast(point,p.impact.w,p.info.x,p.info.y,p.info.z);
   atomicStore(&combat.data[13],bitcast<u32>(point.x));atomicStore(&combat.data[14],bitcast<u32>(point.y));atomicStore(&combat.data[15],bitcast<u32>(point.z));atomicStore(&combat.data[10],atomicLoad(&stats[11])+1u);
   p.pose=vec4(point,p.pose.w);combat.projectiles[id]=p;atomicAdd(&stats[36],1u);atomicStore(&combat.occupied[id],0u);
  }else{combat.projectiles[id]=p;atomicAdd(&stats[35],1u);}return;
 }
 var hit=false;let destination=p.info.w;
 if destination<frame.counts.x{
  let a=source[destination];let v=p.pose.xyz-before;let center=a.pose.xyz+vec3(0,.6,0);let t=clamp(dot(center-before,v)/max(dot(v,v),.00001),0.0,1.0);
  hit=a.motion.z>0.0&&a.info.y!=p.info.x&&distance(before+v*t,center)<.65;
 }
 let ground=groundHeight(p.pose.xz);hit=hit||p.pose.y<=ground+.12||p.pose.w>=p.velocity.w;
 if hit{applyBlast(vec3(p.pose.x,max(p.pose.y,ground+.25),p.pose.z),p.impact.w,p.info.x,p.info.y,p.info.z);atomicAdd(&stats[36],1u);atomicStore(&combat.occupied[id],0u);}
 else{combat.projectiles[id]=p;atomicAdd(&stats[35],1u);}
}
`;
import {
  BATTLE_SQUAD_CAPACITY,
  BATTLE_SQUAD_STRIDE,
  BATTLE_SQUAD_TYPES,
} from "./battle-squad-shader.ts";
