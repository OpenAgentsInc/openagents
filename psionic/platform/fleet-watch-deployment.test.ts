import {test,expect} from 'bun:test'
import {readFileSync,mkdtempSync,writeFileSync,chmodSync,rmSync,existsSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'

const here=import.meta.dir,script=join(here,'pylon-fleet-watch.sh')
const image='us-central1-docker.pkg.dev/openagentsgemini/openagents/pylon-fleet-watch@sha256:'+'a'.repeat(64)
const now=Math.floor(Date.now()/1000),end=now+21600,key='b'.repeat(64)
const runtime='pylon-watch-runtime@openagentsgemini.iam.gserviceaccount.com'
const scheduler='pylon-watch-scheduler@openagentsgemini.iam.gserviceaccount.com'
const marker='OpenAgents bounded Pylon fleet watch'

function fixture(scenario:Record<string,string>={}){
 const work=mkdtempSync(join(tmpdir(),'fleet-watch-deployment-')),calls=join(work,'calls')
 writeFileSync(join(work,'gcloud'),`#!/usr/bin/env bun
import {appendFileSync,existsSync,readFileSync,writeFileSync} from 'node:fs'
const a=process.argv.slice(2),s=a.join(' '),e=process.env
appendFileSync(e.CALLS,JSON.stringify(a)+'\\n')
const out=(v)=>{console.log(typeof v==='string'?v:JSON.stringify(v));process.exit(0)}
const statusPath=e.CALLS+'.state'
let state=existsSync(statusPath)?JSON.parse(readFileSync(statusPath,'utf8')):{scheduler:e.STATE??'PAUSED',created:false,ran:false}
const save=()=>writeFileSync(statusPath,JSON.stringify(state))
if(s.includes('iam service-accounts describe'))process.exit(1)
if(s.includes('iam roles describe')){if(e.BROAD)out('compute.instances.get;compute.instances.stop');process.exit(1)}
if(s.includes('run jobs describe')){
 if(e.NEW==='1'&&!state.created)process.exit(1)
 out({metadata:{labels:{'openagents-component':e.FOREIGN?'someone-else':'pylon-fleet-watch'}},spec:{template:{spec:{template:{spec:{serviceAccountName:'${runtime}',containers:[{env:[{name:'PYLON_WATCH_DRY_RUN',value:e.DRY??'1'}]}]}}}}}})
}
if(s.includes('run jobs deploy')){state.created=true;save();out('deployed')}
if(s.includes('scheduler jobs describe')){if(e.NEW==='1'&&!state.scheduled)process.exit(1);out(s.includes('value(state)')?state.scheduler:'${marker}')}
if(s.includes('scheduler jobs create')){state.scheduled=true;save();out('created')}
if(s.includes('scheduler jobs pause')){state.scheduler='PAUSED';save();out('paused')}
if(s.includes('scheduler jobs resume')){state.scheduler='ENABLED';save();out('enabled')}
if(s.includes('scheduler jobs run')){state.ran=true;save();out('dispatched')}
if(s.includes('executions list'))out(state.ran?[{metadata:{name:'pylon-fleet-watch-fixture'},status:{completionTime:'2026-10-30T20:30:00Z'}}]:[])
if(s.includes('executions describe'))out({metadata:{name:'pylon-fleet-watch-fixture',annotations:{'run.googleapis.com/creator':e.BAD_CREATOR?'stranger':'${scheduler}'}},status:{completionTime:'2026-10-30T20:30:00Z',succeededCount:e.FAILED?'0':'1'}})
out('ok')
`)
 chmodSync(join(work,'gcloud'),0o755)
 writeFileSync(join(work,'sleep'),'#!/usr/bin/env bash\nexit 0\n');chmodSync(join(work,'sleep'),0o755)
 return {work,calls,run:(args:string[])=>Bun.spawnSync(['bash',script,...args],{env:{...process.env,PATH:work+':'+process.env.PATH,CALLS:calls,PYLON_WATCH_RECEIPT:join(work,'receipt.json'),...scenario}}),read:()=>existsSync(calls)?readFileSync(calls,'utf8').trim().split('\n').map(x=>JSON.parse(x) as string[]):[],done:()=>rmSync(work,{recursive:true,force:true})}
}

test('role definitions expose only read, TDX start and non-override job dispatch',()=>{
 for(const [name,permissions] of [['read',['compute.instances.get']],['start',['compute.instances.start']],['invoke',['run.jobs.run']]] as const)
  expect(Bun.YAML.parse(readFileSync(join(here,`fleet-watch-${name}-role.yaml`),'utf8')).includedPermissions).toEqual(permissions)
 const source=readFileSync(script,'utf8')
 for(const banned of ['projects add-iam-policy-binding','roles/run.admin','roles/compute.instanceAdmin','keys create','compute instances stop','compute instances create'])expect(source).not.toContain(banned)
})

test('deployment scopes every grant to the two existing VMs, two fleet services and two jobs',()=>{
 const f=fixture({NEW:'1'})
 try{
  const r=f.run(['deploy',image,String(now),String(end),key]);expect(r.exitCode).toBe(0)
  const calls=f.read(),grants=calls.filter(a=>a.includes('add-iam-policy-binding'))
  const vm=grants.filter(a=>a.includes('instances'))
  expect(vm).toHaveLength(3)
  expect(vm.filter(a=>a.includes('psionic-rtx6000-pylon'))).toHaveLength(1)
  expect(vm.filter(a=>a.includes('oct30-att-tdx-clef'))).toHaveLength(2)
  expect(vm.find(a=>a.includes('psionic-rtx6000-pylon'))!.join(' ')).toContain('pylonFleetWatchReadOnly')
  expect(grants.filter(a=>a.includes('services')).map(a=>a[a.indexOf('add-iam-policy-binding')+1]).sort()).toEqual(['oa-mesh-elephant','oa-mesh-mammoth'])
  expect(grants.filter(a=>a.includes('jobs')).map(a=>a[a.indexOf('add-iam-policy-binding')+1]).sort()).toEqual(['pylon-fleet-watch','pylon-spot-recovery'])
  expect(calls.find(a=>a.includes('deploy'))!.join(' ')).toContain('PYLON_WATCH_DRY_RUN=1')
  expect(calls.at(-1)!.join(' ')).toContain('scheduler jobs pause pylon-fleet-watch')
  expect(calls.some(a=>a.includes('execute'))).toBe(false)
 }finally{f.done()}
})

for(const args of [
 ['deploy',image.replace(/@sha256:.+/,':latest'),String(now),String(end)],
 ['deploy',image,'oops',String(end)],['deploy',image,String(now),String(end+1)],
 ['activate',String(now),String(end),'bad-key'],['activate','1','21601',key]
])test('invalid image/window/key is refused before any cloud effect: '+args.join(' '),()=>{
 const f=fixture();try{expect(f.run(args).exitCode).not.toBe(0);expect(f.read()).toHaveLength(0)}finally{f.done()}
})

test('a broader existing custom role is refused rather than modified',()=>{
 const f=fixture({NEW:'1',BROAD:'1'});try{
  expect(f.run(['deploy',image,String(now),String(end),key]).exitCode).not.toBe(0)
  expect(f.read().some(a=>a.includes('add-iam-policy-binding'))).toBe(false)
 }finally{f.done()}
})

test('a foreign job with the same name is not redeployed',()=>{
 const f=fixture({FOREIGN:'1'});try{
  expect(f.run(['deploy',image,String(now),String(end),key]).exitCode).not.toBe(0)
  expect(f.read().some(a=>a.includes('deploy'))).toBe(false)
 }finally{f.done()}
})

test('verification refuses active mode without resuming or dispatching',()=>{
 const f=fixture({DRY:'0'});try{
  expect(f.run(['verify']).exitCode).not.toBe(0)
  expect(f.read().some(a=>a.includes('resume')||a.includes('run')&&a.includes('scheduler'))).toBe(false)
 }finally{f.done()}
})

test('verification proves the Scheduler creator and preserves initial pause',()=>{
 const f=fixture();try{
  expect(f.run(['verify']).exitCode).toBe(0)
  const calls=f.read();expect(calls.some(a=>a.includes('scheduler')&&a.includes('run'))).toBe(true)
  expect(calls.at(-1)!.join(' ')).toContain('scheduler jobs pause pylon-fleet-watch')
  const receipt=JSON.parse(readFileSync(join(f.work,'receipt.json'),'utf8'))
  expect(receipt.metadata.annotations['run.googleapis.com/creator']).toBe(scheduler)
 }finally{f.done()}
})

for(const scenario of [{FAILED:'1'},{BAD_CREATOR:'1'}])test('failed or wrongly attributed execution is not verification: '+JSON.stringify(scenario),()=>{
 const f=fixture(scenario);try{
  expect(f.run(['verify']).exitCode).not.toBe(0)
  expect(f.read().at(-1)!.join(' ')).toContain('scheduler jobs pause pylon-fleet-watch')
 }finally{f.done()}
})

test('pause makes future runs dry and observes a quiescent window',()=>{
 const f=fixture();try{
  expect(f.run(['pause']).exitCode).toBe(0)
  const calls=f.read(),dry=calls.findIndex(a=>a.some(v=>v.includes('PYLON_WATCH_DRY_RUN=1')))
  expect(dry).toBeGreaterThan(0)
  expect(calls.filter(a=>a.includes('executions')&&a.includes('list')).length).toBeGreaterThan(5)
  expect(calls.some(a=>a.includes('pylon-spot-recovery'))).toBe(false)
 }finally{f.done()}
})
