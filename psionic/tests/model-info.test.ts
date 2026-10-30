import {test,expect} from 'bun:test'
import {spawnSync} from 'node:child_process'
const binary=process.env.PSIONIC_MODEL_INFO_BIN
if(!binary)throw Error('set PSIONIC_MODEL_INFO_BIN to the native model-info executable')
test('native manifest preserves the checked model, capacity and all four pins',()=>{
  const r=spawnSync(binary,['elephant-1'],{encoding:'utf8'})
  expect(r.status).toBe(0)
  const m=JSON.parse(r.stdout)
  expect(m.model).toBe('elephant-1')
  expect(m.origin).toBe('baselabs/elephant-1')
  expect(m.revision).toBe('b3f8a93462fb1051a9de031e94a7ff380e254072')
  expect(m.capacity).toBe(512)
  expect(m.identity).toBe('elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba')
  expect(m.files.map((f:any)=>f.name)).toEqual(['config.json','tokenizer.json','readout.json','model.safetensors'])
  for(const f of m.files){
    expect(f.sha256).toMatch(/^[0-9a-f]{64}$/)
    expect(f.url).toBe(`https://huggingface.co/${m.origin}/resolve/${m.revision}/${f.name}`)
  }
  expect(m.files.map((f:any)=>f.bytes)).toEqual([0,0,0,18819722392])
})
test('unknown models and incorrect process arguments never print a partial plan',()=>{
  for(const args of [[],['other'],['../elephant-1'],['elephant-1','extra']]){
    const r=spawnSync(binary,args,{encoding:'utf8'})
    expect(r.status).not.toBe(0)
    expect(r.stdout).toBe('')
  }
})
