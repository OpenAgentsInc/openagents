import { expect, test } from "bun:test"
import { appendFileSync, mkdirSync, mkdtempSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { readCodexHistoryCatalog, readCodexHistoryPage } from "../src/codex-history.ts"

test("valid 100 MiB / 100-child / 100k-item history stays metadata-first and page-bounded", () => {
  const sessions=mkdtempSync(path.join(tmpdir(),"oa-history-scale-")); const dir=path.join(sessions,"2026","07","10"); mkdirSync(dir,{recursive:true})
  const meta=(id:string,parent?:string)=>JSON.stringify({timestamp:"2026-07-10T00:00:00.000Z",type:"session_meta",payload:{id,source:parent?{subagent:{thread_spawn:{parent_thread_id:parent,depth:1,agent_path:`root/${id}`,agent_nickname:id,agent_role:"worker"}}}:"cli"}})+"\n"
  const root=path.join(dir,"root.jsonl"); writeFileSync(root,meta("root")); const padding="x".repeat(1050)
  for(let base=0;base<100_000;base+=1000){let chunk="";for(let i=base;i<base+1000;i++)chunk+=JSON.stringify({timestamp:"2026-07-10T00:00:01.000Z",type:"response_item",payload:{type:"message",role:"assistant",content:[{type:"output_text",text:`${i}:${padding}`}]}})+"\n";appendFileSync(root,chunk)}
  for(let i=0;i<100;i++)writeFileSync(path.join(dir,`child-${i}.jsonl`),meta(`child-${i}`,"root"))
  expect(statSync(root).size).toBeGreaterThanOrEqual(100*1024*1024)
  const catalogStarted=performance.now(); const catalog=readCodexHistoryCatalog(sessions); const catalogMs=performance.now()-catalogStarted
  expect(catalog.roots[0]?.descendantCount).toBe(100); expect(catalogMs).toBeLessThan(500)
  const pageStarted=performance.now(); const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",offset:0,limit:200})!; const pageMs=performance.now()-pageStarted
  expect(page.totalItems).toBe(100_001); expect(page.items).toHaveLength(200); expect(pageMs).toBeLessThan(15_000)
},120_000)
