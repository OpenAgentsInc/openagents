// Firm reason: 1 (real OfflineAudioContext/Worklet SDK with held descriptor and decode completion).
import {openPackHost} from '../pack-host.js';
import {openCoaxAudio} from '../audio-host.js';
import {openHostServices,openPcmMemory} from '../services-host.js';
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test,label){const until=performance.now()+10000;while(!test()){if(performance.now()>until)throw Error(`pack audio:${label}`);await sleep(10);}}
export async function packAudioFixture(){
 const host=await openPackHost(),services=await openHostServices(),out=[];
 for(const mode of ['decode','malformed','descriptor']){
  const owner={},pack=await host.load({url:'/audio.pack'},{owner}),source=pack.entries.find(entry=>entry.kind===5).bytes,context=new OfflineAudioContext(2,1024,48000),memory=openPcmMemory(services);
  let released=0,entered=false,releaseDecode,decodeCalls=0,borrowRejected=0;const held=new Promise(resolve=>{releaseDecode=resolve;}),decode=context.decodeAudioData.bind(context);
  context.decodeAudioData=async(...args)=>{decodeCalls++;const result=await decode(...args);entered=true;await held;return result;};
  const descriptor=()=>new Promise((resolve,reject)=>{pack.use(async()=>{let finish;const pending=new Promise(done=>{finish=done;});if(mode==='descriptor'){entered=true;await held;}resolve({opus:mode==='malformed'?source.subarray(0,10):source,release(){released++;finish();}});await pending;}).catch(error=>{borrowRejected++;reject(error);});});
  const adapter=await openCoaxAudio(context,context.destination,descriptor,{services,memory});await adapter.ready;const preparing=adapter.prepare('!fetch pack-borrow-fixture');
  if(mode==='malformed'){const result=await preparing;out.push(Number(result===-1),released,decodeCalls,memory.total());await adapter.close();await host.retire(owner);continue;}
  await wait(()=>entered,`${mode} entered`);let packClosed=false,audioClosed=false;const closingPack=host.retire(owner).then(()=>{packClosed=true;}),closingAudio=adapter.close().then(report=>{audioClosed=true;return report;});await sleep(15);
  out.push(Number(!packClosed),Number(!audioClosed),Number(source.byteLength>0));if(mode==='decode')out.push(host.info().owners,released);
  releaseDecode();const result=await preparing,report=await closingAudio;await closingPack;
  if(mode==='decode')out.push(Number(result!==-1),released,source.byteLength,memory.total(),report.nodeAfter);
  else out.push(source.byteLength,memory.total(),report.nodeAfter);
 }
 return out;
}
