export const BATTLE_SQUAD_CAPACITY = 65_536;
export const BATTLE_SQUAD_STRIDE = 64;
export const BATTLE_SQUAD_INTERVAL = 12;
export const BATTLE_SQUAD_ORDERS = [
  "advance",
  "flank",
  "withdraw",
  "screen",
] as const;

export function battleSquadCount(count: number): number {
  if (!count) return 0;
  const columns = Math.ceil(Math.sqrt(count));
  return Math.ceil(columns / 8) * Math.ceil(Math.ceil(count / columns) / 4) * 2;
}
export function battleSquadId(id: number, count: number, team: number): number {
  const columns = Math.ceil(Math.sqrt(count));
  return (
    (Math.floor(Math.floor(id / columns) / 4) * Math.ceil(columns / 8) +
      Math.floor((id % columns) / 8)) *
      2 +
    team
  );
}

export const BATTLE_SQUAD_TYPES = /* wgsl */ `
struct Squad {center:vec4f,anchor:vec4f,goal:vec4f,stamp:vec4u}
`;

/** One commander observes a fixed formation tile; actors consume its shared order. */
export const BATTLE_SQUAD_WGSL = /* wgsl */ `
fn squadCount()->u32{
 if frame.counts.x==0u{return 0u;}let columns=u32(ceil(sqrt(f32(frame.counts.x))));let rows=(frame.counts.x+columns-1u)/columns;
 return ((columns+7u)/8u)*((rows+3u)/4u)*2u;
}
fn squadId(id:u32,team:u32)->u32{
 let columns=u32(ceil(sqrt(f32(frame.counts.x))));return (((id/columns)/4u)*((columns+7u)/8u)+(id%columns)/8u)*2u+team;
}
fn squadFormationOffset(id:u32,team:u32,forward:vec2f)->vec2f{
 let columns=u32(ceil(sqrt(f32(frame.counts.x))));let side=vec2(-forward.y,forward.x);
 return (forward*(f32((id%columns)%8u)-3.5)*2.0+side*(f32((id/columns)%4u)-1.5)*2.0)*select(1.0,-1.0,team==1u);
}
@compute @workgroup_size(128)fn commandSquads(@builtin(global_invocation_id)inv:vec3u){
 let id=inv.x;if id>=squadCount(){return;}let tick=atomicLoad(&stats[11]);if tick%12u!=1u{return;}
 let columns=u32(ceil(sqrt(f32(frame.counts.x))));let tiles=(columns+7u)/8u;let tile=id/2u;let team=id%2u;
 let first=vec2u((tile%tiles)*8u,(tile/tiles)*4u);let previous=combat.squads[id];
 var center=vec2f(0);var anchor=vec2f(0);var alive=0u;var rangedCount=0u;var assigned=0u;var health=0.0;
 for(var row=0u;row<4u;row++){for(var column=0u;column<8u;column++){
  let p=first+vec2u(column,row);let member=p.y*columns+p.x;if p.x>=columns||member>=frame.counts.x{continue;}
  let a=source[member];if a.info.y!=team{continue;}assigned++;if a.motion.z<=0.0{continue;}
  center+=a.pose.xz;alive++;health+=clamp(a.motion.z/initialHealth(a.info.x),0.0,1.0);
  if rangedRole(member){anchor+=a.pose.xz;rangedCount++;}
 }}
 if alive==0u{combat.squads[id]=Squad(vec4f(0),vec4f(0),vec4f(0),vec4u(tick,0u,0xffffffffu,team));return;}
 center/=f32(alive);anchor=select(center,anchor/max(1.0,f32(rangedCount)),rangedCount>0u);let strength=health/max(1.0,f32(assigned));
 var enemyId=0xffffffffu;var nearest=1e9;var enemyPoint=center;var enemies=0u;var allies=0u;
 for(var row=0u;row<4u;row++){for(var column=0u;column<8u;column++){
  let p=first+vec2u(column,row);let member=p.y*columns+p.x;if p.x>=columns||member>=frame.counts.x{continue;}
  let a=source[member];if a.info.y!=team||a.motion.z<=0.0{continue;}var point=vec2f(0);var valid=false;
  if a.info.z==participantId{if participantAvailable(team){point=frame.participantPose.xz;valid=true;}}
  else if a.info.z<frame.counts.x{let other=source[a.info.z];if other.motion.z>0.0&&other.info.y!=team{point=other.pose.xz;valid=true;}}
  let d=distance(center,point);if valid&&(d<nearest||(d==nearest&&a.info.z<enemyId)){enemyId=a.info.z;nearest=d;enemyPoint=point;}
 }}
 let cell=cellPosition(center);
 for(var z=-2;z<=2;z++){for(var x=-2;x<=2;x++){
  let p=cell+vec2i(x,z);if any(p<vec2i(0))||any(p>=vec2i(battleGridDimension)){continue;}let c=cellIndex(p);let n=min(atomicLoad(&cells[c].count),64u);
  for(var i=0u;i<n;i++){
   let member=cells[c].members[i];let a=source[member];let d=distance(center,a.pose.xz);if a.motion.z<=0.0||d>8.0{continue;}
   if a.info.y==team{allies++;}else{enemies++;if d<nearest||(d==nearest&&member<enemyId){enemyId=member;nearest=d;enemyPoint=a.pose.xz;}}
  }
 }}
 if participantAvailable(team){
  let point=frame.participantPose.xz;let d=distance(center,point);
  if d<=8.0{enemies++;}
  if min(d,distance(anchor,point))<=16.0&&(d<nearest||(d==nearest&&participantId<enemyId)){enemyId=participantId;nearest=d;enemyPoint=point;}
 }
 let fallback=vec2(select(1.0,-1.0,team==1u),0);var forward=safeNormal(enemyPoint-center);if enemyId==0xffffffffu{forward=fallback;}
 var kind=0u;var goal=center+forward*24.0;
 if assigned>=8u&&alive>=4u&&enemyId!=0xffffffffu{
  let weak=nearest<32.0&&(strength<.45||(f32(enemies)>f32(allies)*1.5&&strength<.75));
  let held=previous.stamp.x>0u&&previous.goal.z==2.0&&(tick-previous.stamp.y<120u||!(strength>.65&&nearest>16.0));
  if weak||held{kind=2u;goal=center-forward*24.0;}
  else if rangedCount>0u&&rangedCount<alive&&distance(anchor,enemyPoint)<16.0{kind=3u;goal=anchor+safeNormal(enemyPoint-anchor)*clamp(distance(anchor,enemyPoint)*.45,2.0,6.0);}
  else{
   let arrived=previous.stamp.x>0u&&previous.goal.z==1.0&&distance(center,previous.goal.xy)<6.0;
   let regrouping=previous.stamp.x>0u&&previous.goal.z==0.0&&tick-previous.stamp.y<120u;
   let keepFlank=previous.stamp.x>0u&&previous.goal.z==1.0&&tick-previous.stamp.y<120u;
   if !arrived&&!regrouping&&strength>.7&&nearest<48.0&&(tile%4u==0u||keepFlank){
    kind=1u;let side=vec2(-forward.y,forward.x)*select(-1.0,1.0,(hash(tile^frame.counts.z)&1u)!=0u);
    goal=enemyPoint+forward*min(8.0,frame.battle.z*.1)+side*min(18.0,max(5.0,frame.battle.z*.12));
   }
  }
 }
 goal=clamp(goal,frame.battle.xy-vec2(frame.battle.z),frame.battle.xy+vec2(frame.battle.z));
 let since=select(tick,previous.stamp.y,previous.stamp.x>0u&&u32(previous.goal.z)==kind);
 combat.squads[id]=Squad(vec4(center,strength,f32(alive)),vec4(anchor,f32(rangedCount),f32(enemies)),vec4(goal,f32(kind),nearest),vec4u(tick,since,enemyId,team));
}
`;
