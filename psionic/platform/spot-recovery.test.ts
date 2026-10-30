import {test,expect} from 'bun:test'
import {readFileSync,mkdtempSync,writeFileSync,chmodSync,rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
const here=import.meta.dir,decider=process.env.PYLON_RECOVERY_TEST_DECIDER
if(!decider)throw Error('run psionic/platform/spot-recovery-check.sh, which builds the real Coax policy adapter')
for(const scenario of [{status:'RUNNING',dry:'0',action:'noop',start:false},{status:'RUNNING',dry:'1',action:'noop',start:false},{status:'TERMINATED',dry:'1',action:'would_start',start:false},{status:'TERMINATED',dry:'0',action:'start_requested',start:true},{status:'STOPPING',dry:'0',action:'wait',start:false},{status:'UNKNOWN',dry:'0',action:'refused',start:false}])test('effects obey checked Coax policy '+JSON.stringify(scenario),()=>{
 const work=mkdtempSync(join(tmpdir(),'pylon-recovery-test-'))
 try{
  const fake=join(work,'curl');writeFileSync(fake,"#!/usr/bin/env bash\nprintf \"%s\\n\" \"$*\" >> \"$CALLS\"\nif [[ $* == *metadata.google.internal* ]];then printf '{\"access_token\":\"fixture-only\"}\\n';elif [[ $* == */start* ]];then printf \"{}\\n\";else printf '{\"status\":\"%s\"}\\n' \"$OBSERVED\";fi\n");chmodSync(fake,0o755)
  const run=Bun.spawnSync(['bash',join(here,'spot-recovery-run.sh')],{env:{...process.env,PATH:work+':'+process.env.PATH,CALLS:join(work,'calls'),OBSERVED:scenario.status,PYLON_RECOVERY_DRY_RUN:scenario.dry,PYLON_RECOVERY_DECIDER:decider}})
  expect(run.exitCode).toBe(scenario.action==='refused'?1:0);expect(run.stdout.toString()).toContain('"action":"'+scenario.action+'"')
  const calls=readFileSync(join(work,'calls'),'utf8');expect(calls.includes('/start')).toBe(scenario.start);expect(calls).not.toContain('psionic-l4-elephant');if(scenario.start)expect(calls).toContain('--request POST')
 }finally{rmSync(work,{recursive:true,force:true})}
})
test('runtime and invoker roles contain only their exact minimum permissions',()=>{
 expect(Bun.YAML.parse(readFileSync(join(here,'spot-recovery-runtime-role.yaml'),'utf8')).includedPermissions).toEqual(['compute.instances.get','compute.instances.start'])
 expect(Bun.YAML.parse(readFileSync(join(here,'spot-recovery-invoke-role.yaml'),'utf8')).includedPermissions).toEqual(['run.jobs.run'])
 const source=readFileSync(join(here,'pylon-spot-recovery.sh'),'utf8');expect(source).not.toContain('roles/run.admin');expect(source).not.toContain('roles/compute.instanceAdmin');expect(source).not.toContain('keys create')
})
test('verify cannot resume an intentionally paused active-mode controller',()=>{
 const work=mkdtempSync(join(tmpdir(),'pylon-recovery-verify-'))
 try{
  const fake=join(work,'gcloud');writeFileSync(fake,`#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$CALLS"
if [[ $* == *"scheduler jobs describe"* ]];then printf 'PAUSED\\n';else printf '{"spec":{"template":{"spec":{"template":{"spec":{"containers":[{"env":[{"name":"PYLON_RECOVERY_DRY_RUN","value":"0"}]}]}}}}}}\\n';fi
`);chmodSync(fake,0o755)
  const run=Bun.spawnSync(['bash',join(here,'pylon-spot-recovery.sh'),'verify'],{env:{...process.env,PATH:work+':'+process.env.PATH,CALLS:join(work,'calls')}})
  expect(run.exitCode).toBe(1);expect(run.stderr.toString()).toContain('refuses to resume');expect(readFileSync(join(work,'calls'),'utf8')).not.toContain('jobs resume')
 }finally{rmSync(work,{recursive:true,force:true})}
})
test('pause makes future executions dry and requires more than one empty drain observation',()=>{
 const work=mkdtempSync(join(tmpdir(),'pylon-recovery-pause-'))
 try{
  const fake=join(work,'gcloud');writeFileSync(fake,`#!/usr/bin/env bash
printf '%s\\n' "$*" >> "$CALLS"
if [[ $* == *"executions list"* ]];then printf '[]\\n';fi
`);chmodSync(fake,0o755);writeFileSync(join(work,'sleep'),'#!/usr/bin/env bash\nexit 0\n');chmodSync(join(work,'sleep'),0o755)
  const run=Bun.spawnSync(['bash',join(here,'pylon-spot-recovery.sh'),'pause'],{env:{...process.env,PATH:work+':'+process.env.PATH,CALLS:join(work,'calls')}})
  expect(run.exitCode).toBe(0);const calls=readFileSync(join(work,'calls'),'utf8');expect(calls.indexOf('jobs pause')).toBeLessThan(calls.indexOf('PYLON_RECOVERY_DRY_RUN=1'));expect(calls.match(/executions list/g)!.length).toBeGreaterThan(5);expect(calls).not.toContain('compute instances stop')
 }finally{rmSync(work,{recursive:true,force:true})}
})
