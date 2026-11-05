// Firm reason: 1 (browser SDK: scalar projection of Coax lifecycle and frame offers).
export function makeHostLifecycle(x){
  if(x?.coaxial_host_contract_version?.()!==1||!x.coaxial_host_lifecycle_version)return null;
  if(x.coaxial_host_lifecycle_version()!==1)throw Error('unsupported Aethyr host lifecycle contract');
  return {
    reset:()=>x.coaxial_host_lifecycle_reset(),
    event:(kind,nowUs,arg=0)=>Number(x.coaxial_host_lifecycle_event(kind,BigInt(nowUs),BigInt(arg))),
    offer:(nowUs,pending)=>x.coaxial_host_frame_offer(BigInt(nowUs),BigInt(pending))?{dt:x.coaxial_host_frame_dt()}:null,
    info:()=>Array.from({length:5},(_,i)=>Number(x.coaxial_host_lifecycle_info(i)))
  };
}
