// Actual site packaging, with a small app fixture. No cloud upload, models,
// private keys or hidden presentations; this checks the publication boundary.
import {test,expect} from 'bun:test'
import {mkdtempSync,mkdirSync,copyFileSync,writeFileSync,readFileSync,existsSync,rmSync} from 'node:fs'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {resolve as served} from '../golden-serve'
const root=resolve(import.meta.dir,'../../..')
test('site packaging publishes exact guide text and only the allowlisted public proof JSON',()=>{
 const work=mkdtempSync(join(tmpdir(),'coax-guide-package-')),app=join(work,'app'),stage=join(work,'stage'),doc=join(app,'docs/connect-a-pylon')
 try{
  mkdirSync(doc,{recursive:true});mkdirSync(join(app,'build'));writeFileSync(join(app,'build.sh'),'#!/usr/bin/env bash\nexit 0\n',{mode:0o755})
  for(const name of ['index.html','index.md'])copyFileSync(join(root,'apps/openagents-web/docs/connect-a-pylon',name),join(doc,name))
  for(const name of ['connect-pylon.wgsl','llms.txt'])copyFileSync(join(root,'apps/openagents-web',name),join(app,name))
  copyFileSync(join(root,'apps/openagents-web/build/connect-pylon.wasm'),join(app,'build/connect-pylon.wasm'))
  const proof='{"fixture":"public packaging check"}\n';writeFileSync(join(doc,'confidential-round.json'),proof);writeFileSync(join(doc,'private.json'),'{"fixture":"must remain private"}\n')
  const run=Bun.spawnSync(['bash',join(root,'coax/web/deploy.sh'),app,'fixture'],{cwd:root,env:{...process.env,COAXIAL_STAGE_TO:stage,COAXIAL_UNLOCKS_FILE:'/dev/null',COAXIAL_UNLOCKS:''}})
  if(run.exitCode)throw Error(run.stderr.toString()+run.stdout.toString())
  const site=join(stage,'site');expect(readFileSync(join(site,'docs/connect-a-pylon/index.md'),'utf8')).toBe(readFileSync(join(root,'docs/connect-a-pylon.md'),'utf8'))
  expect(readFileSync(join(site,'docs/connect-a-pylon/confidential-round.json'),'utf8')).toBe(proof)
  expect(existsSync(join(site,'docs/connect-a-pylon/private.json'))).toBe(false)
  expect(served(site,'/docs/connect-a-pylon/confidential-round.json')?.type).toBe('application/json')
  const html=readFileSync(join(site,'docs/connect-a-pylon/index.html'),'utf8');expect(html).toMatch(/data-wasm="\/build\/connect-pylon\.[0-9a-f]{12}\.wasm"/);expect(html).toMatch(/data-wgsl="\/build\/connect-pylon\.[0-9a-f]{12}\.wgsl"/)
  expect(readFileSync(join(site,'llms-full.txt'),'utf8')).toContain(readFileSync(join(root,'docs/connect-a-pylon.md'),'utf8'))
 }finally{rmSync(work,{recursive:true,force:true})}
})
