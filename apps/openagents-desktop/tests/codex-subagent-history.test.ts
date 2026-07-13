import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { decodeCodexHistoryCatalog, decodeCodexHistoryPage } from "../src/codex-history-contract.ts"
import { readCodexHistoryCatalog, readCodexHistoryPage } from "../src/codex-history.ts"

const root = () => { const value=path.join(mkdtempSync(path.join(tmpdir(), "oa-subagents-")),"sessions");mkdirSync(value,{recursive:true});return value }
const write = (root: string, id: string, parent: string | null, rows: unknown[]) => {
  const dir = path.join(root,"2026","07","10"); mkdirSync(dir,{recursive:true})
  const source = parent === null ? "cli" : { subagent: { thread_spawn: { parent_thread_id: parent, depth: 1, agent_path: `root/${id}`, agent_nickname: id, agent_role: "worker" } } }
  writeFileSync(path.join(dir,`${id}.jsonl`), [JSON.stringify({timestamp:"2026-07-10T00:00:00.000Z",type:"session_meta",payload:{id,source}}),...rows.map(row=>JSON.stringify(row))].join("\n")+"\n")
}
const msg = (role:string,text:string) => ({timestamp:"2026-07-10T00:00:01.000Z",type:"response_item",payload:{type:"message",role,content:[{type:"output_text",text}]}})

describe("openagents_desktop.codex_subagent_history.v2", () => {
  test("builds the historical descendant graph and keeps source order", () => {
    const sessions=root(); write(sessions,"root",null,[msg("user","root")]); write(sessions,"a","root",[msg("assistant","a")]); write(sessions,"b","root",[msg("assistant","b")]); write(sessions,"g","a",[msg("assistant","grandchild")])
    const catalog=readCodexHistoryCatalog(sessions)
    expect(catalog.roots).toHaveLength(1); expect(catalog.roots[0]?.descendantCount).toBe(3); expect(decodeCodexHistoryCatalog(catalog)).not.toBeNull()
    const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"g",offset:0,limit:200})!
    expect(page.rootThreadRef).toBe("root"); expect(page.agents).toHaveLength(4); expect(page.items.map(item=>item.sequence)).toEqual([0,1]); expect(page.completeness).toEqual({source:2,rendered:2,redactions:0,gaps:0,complete:true}); expect(decodeCodexHistoryPage(page)).not.toBeNull()
  })

  test("renders rich supported records, redacts credentials, and exposes unknown/corrupt gaps", () => {
    const sessions=root(); const dir=path.join(sessions,"2026","07","10"); mkdirSync(dir,{recursive:true})
    writeFileSync(path.join(dir,"root.jsonl"), [
      JSON.stringify({timestamp:"2026-07-10T00:00:00.000Z",type:"session_meta",payload:{id:"root",source:"cli"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:01.000Z",type:"response_item",payload:{type:"reasoning",summary:[{text:"bounded summary"}],encrypted_content:"never render"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:02.000Z",type:"response_item",payload:{type:"function_call",name:"exec",call_id:"c1",arguments:"Bearer abcdefghijklmnop"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:03.000Z",type:"response_item",payload:{type:"function_call_output",call_id:"c1",output:"ok"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:04.000Z",type:"event_msg",payload:{type:"sub_agent_activity",agent_thread_id:"child",kind:"started"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:04.500Z",type:"event_msg",payload:{type:"sub_agent_activity",agent_thread_id:"child",kind:"interacted"}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:05.000Z",type:"event_msg",payload:{type:"token_count",info:{total_token_usage:42}}}),
      JSON.stringify({timestamp:"2026-07-10T00:00:06.000Z",type:"future_record",payload:{type:"future_kind"}}),
      "{broken",
    ].join("\n")+"\n")
    write(sessions,"child","root",[msg("assistant","Checking the focused tests")])
    const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",limit:200})!
    expect(page.items.map(item=>item.kind)).toEqual(["session","reasoning","tool_call","tool_result","collaboration","collaboration","usage","gap","gap"])
    expect(JSON.stringify(page)).not.toContain("abcdefghijklmnop"); expect(page.items[2]?.redacted).toBe(true)
    expect(page.items[4]?.relatedAgent).toMatchObject({threadRef:"child",title:"child",status:"completed",latest:{label:"Assistant",summary:"Checking the focused tests",kind:"assistant_message"}})
    expect(page.items[5]?.relatedAgent).toBeUndefined()
    expect(decodeCodexHistoryPage(page)).not.toBeNull()
    expect(page.completeness).toEqual({source:9,rendered:7,redactions:0,gaps:2,complete:true})
  })

  test("pages without overlap or omission", () => {
    const sessions=root(); write(sessions,"root",null,Array.from({length:501},(_,i)=>msg("assistant",String(i))))
    const pages=[0,200,400].map(offset=>readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",offset,limit:200})!)
    const refs=pages.flatMap(page=>page.items.map(item=>item.itemRef)); expect(new Set(refs).size).toBe(502); expect(pages[0]?.hasPrevious).toBe(false); expect(pages[2]?.hasNext).toBe(false)
  })
  test("H5 preserves and loss-accounts the persisted compaction boundary",()=>{const sessions=root();write(sessions,"root",null,[msg("user","before"),{timestamp:"2026-07-10T00:00:02.000Z",type:"compacted",payload:{item_count:7,replacement_history:Array.from({length:7},(_,index)=>({index}))}},msg("assistant","after")]);const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",limit:50})!;expect(page.items.map(({kind,label})=>({kind,label}))).toEqual([{kind:"session",label:"Session started"},{kind:"user_message",label:"You"},{kind:"context",label:"History compacted"},{kind:"assistant_message",label:"Assistant"}]);expect(page.items[2]?.fields).toEqual([{label:"replacement items",value:"7"}]);expect(page.items[3]?.summary).toBe("after");expect(page.completeness).toEqual({source:4,rendered:4,redactions:0,gaps:0,complete:true});expect(decodeCodexHistoryPage(page)).not.toBeNull()})
  test("classifies the structured AGENTS injection as metadata rather than user chat",()=>{const sessions=root();write(sessions,"root",null,[msg("user","# AGENTS.md instructions for /workspace\n\n<INSTRUCTIONS>\n# AGENTS\nKeep the contract.\n</INSTRUCTIONS>")]);const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",limit:50})!;expect(page.items[1]).toMatchObject({kind:"metadata",label:"Agent metadata"});expect(page.items[1]?.summary).toContain("Keep the contract.");expect(decodeCodexHistoryPage(page)).not.toBeNull()})
  test("classifies the injected environment envelope as context rather than user chat",()=>{const sessions=root();const envelope="<environment_context>\n  <cwd>/workspace</cwd>\n  <shell>zsh</shell>\n  <permission_profile type=\"disabled\" />\n</environment_context>";write(sessions,"root",null,[msg("user",envelope),msg("user","U there?"),msg("user","Show the literal <environment_context> tag")]);writeFileSync(path.join(path.dirname(sessions),"session_index.jsonl"),JSON.stringify({id:"root",thread_name:"<environment_context> <cwd>/workspace</cwd>"})+"\n");const catalog=readCodexHistoryCatalog(sessions);expect(catalog.roots[0]?.title).toBe("U there?");const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",limit:50})!;expect(page.items[1]).toMatchObject({kind:"context",label:"Execution environment",summary:envelope});expect(page.items[2]).toMatchObject({kind:"user_message",label:"You",summary:"U there?"});expect(page.items[3]).toMatchObject({kind:"user_message",label:"You",summary:"Show the literal <environment_context> tag"});expect(decodeCodexHistoryPage(page)).not.toBeNull()})
  test("hides protocol injections and parses the adjacent agent handoff",()=>{const sessions=root();write(sessions,"root",null,[{timestamp:"2026-07-10T00:00:01.000Z",type:"inter_agent_communication_metadata",payload:{trigger_turn:true}},{timestamp:"2026-07-10T00:00:02.000Z",type:"response_item",payload:{type:"agent_message",author:"/root",recipient:"/root/roadmap_audit",content:[{type:"output_text",text:"Message Type: NEW_TASK\nTask name: /root/roadmap_audit\nSender: /root\nPayload:\n\nReview the roadmap."}]}},msg("user","<recommended_plugins>\nInternal plugin suggestions\n</recommended_plugins>")]);const page=readCodexHistoryPage({sessionsRoot:sessions,threadRef:"root",limit:50})!;expect(page.items[1]).toMatchObject({kind:"system_message",label:"Agent communication metadata"});expect(page.items[2]).toMatchObject({kind:"agent_message",label:"Agent message",fields:[{label:"message type",value:"NEW_TASK"},{label:"task",value:"/root/roadmap_audit"},{label:"sender",value:"/root"},{label:"recipient",value:"/root/roadmap_audit"},{label:"payload",value:"Review the roadmap."}]});expect(page.items[3]).toMatchObject({kind:"system_message",label:"Plugin metadata"});expect(decodeCodexHistoryPage(page)).not.toBeNull()})
  test("discovers archived zstd rollouts",()=>{const sessions=root();mkdirSync(sessions,{recursive:true});const archived=path.join(path.dirname(sessions),"archived_sessions");mkdirSync(archived,{recursive:true});const body=JSON.stringify({timestamp:"2026-07-10T00:00:00.000Z",type:"session_meta",payload:{id:"archived",source:"cli"}})+"\n";writeFileSync(path.join(archived,"archived.jsonl.zst"),Bun.zstdCompressSync(Buffer.from(body)));expect(readCodexHistoryCatalog(sessions).roots.map(item=>item.threadRef)).toEqual(["archived"])})
})
