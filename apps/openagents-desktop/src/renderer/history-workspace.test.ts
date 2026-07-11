import { describe, expect, test } from "bun:test"
import type { View } from "@effect-native/core"
import { historyWorkspaceView, projectHistoryTimelineEvents } from "./history-workspace.ts"
import type { CodexHistoryPage } from "../codex-history-contract.ts"

const page: CodexHistoryPage = { rootThreadRef:"root",selectedThreadRef:"child",offset:0,limit:200,totalItems:1,hasPrevious:false,hasNext:false,completeness:{source:1,rendered:1,redactions:0,gaps:0,complete:true},agents:[{threadRef:"root",parentThreadRef:null,title:"Root",status:"completed",createdAt:"2026-07-10T00:00:00Z",updatedAt:"2026-07-10T00:00:00Z",depth:0,descendantCount:1,model:null,role:null,nickname:null,agentPath:null,sourceVersion:null,reasoning:null},{threadRef:"child",parentThreadRef:"root",title:"Worker",status:"running",createdAt:"2026-07-10T00:00:00Z",updatedAt:"2026-07-10T00:00:00Z",depth:1,descendantCount:0,model:"gpt",role:"worker",nickname:"worker",agentPath:"root/worker",sourceVersion:"v2",reasoning:null}],items:[{itemRef:"child:0",threadRef:"child",sequence:0,timestamp:"2026-07-10T00:00:00Z",kind:"tool_call",label:"exec",summary:"bun test",status:"running",fields:[{label:"input",value:"bun test"}],redacted:false,sourceType:"response_item/function_call"}] }
const nodes=(view:View):any[]=>{const value=view as any; return [view,...(Array.isArray(value.children)?value.children.flatMap(nodes):[]),...(Array.isArray(value.items)?value.items.flatMap((item:any)=>item?._tag?nodes(item):[item]):[]),...(Array.isArray(value.sections)?value.sections.flatMap((section:any)=>[section,...(section.items??[])]):[]),...(Array.isArray(value.panes)?value.panes.flatMap((p:any)=>nodes(p.content)):[])]}
describe("history workspace",()=>{
  test("renders a bounded split view with a semantic agent tree and no redundant trace chrome",()=>{const view=historyWorkspaceView({catalog:{roots:[],agents:[]},page,selectedItemRef:null,railCollapsed:false,expandedThreadRefs:["root"],visibleRootCount:40});const all=nodes(view) as any[];expect(all.find(n=>n.key==="history-workspace-split")?._tag).toBe("SplitPane");expect(all.find(n=>n.key==="history-agents-drawer")).toMatchObject({_tag:"IconButton",icon:"Agent"});expect(all.find(n=>n.key==="history-center-title")).toBeUndefined();expect(all.find(n=>n.key==="history-center-status")).toBeUndefined();expect(all.find(n=>n.key==="history-completeness")).toBeUndefined();expect(all.find(n=>n.key==="history-agent-config")).toBeUndefined();expect(all.find(n=>n.key==="history-agent-list")).toMatchObject({_tag:"NavRail",role:"tree",activeId:"history-agent-child"});expect(all.find(n=>n.id==="history-agent-child")).toMatchObject({depth:1,selected:true})})
  test("selected item opens structured detail with back action",()=>{const all=nodes(historyWorkspaceView({catalog:{roots:[],agents:[]},page,selectedItemRef:"child:0",railCollapsed:false,expandedThreadRefs:["root"],visibleRootCount:40})) as any[];expect(all.find(n=>n.key==="history-item-inspector")).toBeDefined();expect(all.find(n=>n.key==="history-item-back")?.onPress.name).toBe("HistoryItemSelected")})
  test("bounded timeline uses typed selectable events and compact row previews",()=>{const long={...page,items:[{...page.items[0]!,summary:"x".repeat(2_000)}]};const all=nodes(historyWorkspaceView({catalog:{roots:[],agents:[]},page:long,selectedItemRef:null,railCollapsed:false,expandedThreadRefs:["root"],visibleRootCount:40})) as any[];const timeline=all.find(n=>n.key==="history-timeline-page");expect(timeline).toMatchObject({_tag:"Timeline"});const row=timeline.events[0];expect(row.key).toBe("history-item-child:0");expect(row.detail.length).toBeLessThan(500);expect(row.detail.endsWith("…")).toBe(true)})
  test("projects the raw ledger into messages and composed tool rows",()=>{
    const item=(sequence:number,kind:CodexHistoryPage["items"][number]["kind"],label:string,summary:string,fields:CodexHistoryPage["items"][number]["fields"]=[]):CodexHistoryPage["items"][number]=>({itemRef:`child:${sequence}`,threadRef:"child",sequence,timestamp:"2026-07-10T00:00:00Z",kind,label,summary,status:"completed",fields,redacted:false,sourceType:`fixture/${kind}`})
    const events=projectHistoryTimelineEvents([
      item(0,"session","Session started","Codex session metadata"),
      item(1,"system_message","Message · developer","internal instructions"),
      item(2,"usage","Usage","Token usage update"),
      item(7,"reasoning","Reasoning summary","[REDACTED: reasoning not persisted as summary]"),
      item(8,"assistant_message","Assistant","Assistant"),
      item(3,"user_message","You","Run the tests"),
      item(4,"tool_call","exec_command","bun test",[{label:"call",value:"call-1"},{label:"input",value:"bun test"}]),
      item(5,"tool_result","Tool result","12 passed",[{label:"call",value:"call-1"}]),
      item(6,"assistant_message","Assistant","All tests pass"),
    ])
    expect(events.map(event=>event.id)).toEqual(["child:3","child:4","child:6"])
    expect(events[1]).toMatchObject({label:"Terminal",detail:"bun test",variant:"tool",icon:"Terminal",status:"success",refs:["child","child:5"]})
    expect(events[0]).toMatchObject({variant:"message",icon:"Chats"})
  })
})
