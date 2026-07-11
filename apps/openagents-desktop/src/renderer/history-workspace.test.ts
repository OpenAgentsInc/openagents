import { describe, expect, test } from "bun:test"
import type { View } from "@effect-native/core"
import { historyAgentTraversalTarget, historyWorkspaceView, isHistoryAgentTraversalShortcut, projectHistoryEntries, projectHistoryTimelineEvents, visibleHistoryAgents, type HistoryWorkspaceState } from "./history-workspace.ts"
import type { CodexHistoryPage } from "../codex-history-contract.ts"

const page: CodexHistoryPage = { rootThreadRef:"root",selectedThreadRef:"child",offset:0,limit:200,totalItems:1,hasPrevious:false,hasNext:false,completeness:{source:1,rendered:1,redactions:0,gaps:0,complete:true},agents:[{threadRef:"root",parentThreadRef:null,title:"Root",status:"completed",createdAt:"2026-07-10T00:00:00Z",updatedAt:"2026-07-10T00:00:00Z",depth:0,descendantCount:1,model:null,role:null,nickname:null,agentPath:null,sourceVersion:null,reasoning:null},{threadRef:"child",parentThreadRef:"root",title:"Worker",status:"running",createdAt:"2026-07-10T00:00:00Z",updatedAt:"2026-07-10T00:00:00Z",depth:1,descendantCount:0,model:"gpt",role:"worker",nickname:"worker",agentPath:"root/worker",sourceVersion:"v2",reasoning:null}],items:[{itemRef:"child:0",threadRef:"child",sequence:0,timestamp:"2026-07-10T00:00:00Z",kind:"tool_call",label:"exec",summary:"bun test",status:"running",fields:[{label:"input",value:"bun test"}],redacted:false,sourceType:"response_item/function_call"}] }
const nodes=(view:View):any[]=>{const value=view as any; return [view,...(Array.isArray(value.children)?value.children.flatMap(nodes):[]),...(Array.isArray(value.items)?value.items.flatMap((item:any)=>item?._tag?nodes(item):[item]):[]),...(Array.isArray(value.sections)?value.sections.flatMap((section:any)=>[section,...(section.items??[])]):[]),...(Array.isArray(value.panes)?value.panes.flatMap((p:any)=>nodes(p.content)):[])]}
const timelineEvents=(all:any[]):any[]=>all.filter(n=>n._tag==="Timeline").flatMap(n=>n.events)
const item=(sequence:number,kind:CodexHistoryPage["items"][number]["kind"],label:string,summary:string,fields:CodexHistoryPage["items"][number]["fields"]=[]):CodexHistoryPage["items"][number]=>({itemRef:`child:${sequence}`,threadRef:"child",sequence,timestamp:"2026-07-10T00:00:00Z",kind,label,summary,status:"completed",fields,redacted:false,sourceType:`fixture/${kind}`})
const stateWith=(pageValue:CodexHistoryPage,selectedItemRef:string|null=null):HistoryWorkspaceState=>({catalog:{roots:[],agents:[]},page:pageValue,selectedItemRef,railCollapsed:false,expandedThreadRefs:["root"],visibleRootCount:40})
/** Generic walk over the typed markdown model (blocks + inlines). */
const markdownWalk=(value:any,visit:(node:any)=>void):void=>{
  if(Array.isArray(value)){value.forEach(entry=>markdownWalk(entry,visit));return}
  if(typeof value!=="object"||value===null)return
  if(typeof value.kind==="string")visit(value)
  if(Array.isArray(value.children))markdownWalk(value.children,visit)
  if(Array.isArray(value.items))markdownWalk(value.items,visit)
}
/** All literal text carried by markdown inline nodes under one Markdown view. */
const markdownTexts=(view:any):string[]=>{
  const out:string[]=[]
  if(view._tag==="Markdown")markdownWalk(view.blocks,node=>{if((node.kind==="text"||node.kind==="code")&&typeof node.text==="string")out.push(node.text)})
  return out
}
const markdownNodes=(all:any[],kind:string):any[]=>{
  const found:any[]=[]
  for(const node of all.filter(n=>n._tag==="Markdown"))markdownWalk(node.blocks,entry=>{if(entry.kind===kind)found.push(entry)})
  return found
}

describe("history workspace",()=>{
  test("renders a bounded split view with a semantic agent tree and icon-only lifecycle state",()=>{const view=historyWorkspaceView(stateWith(page));const all=nodes(view) as any[];const root=all.find(n=>n.id==="history-agent-root");const child=all.find(n=>n.id==="history-agent-child");expect(all.find(n=>n.key==="history-workspace-split")?._tag).toBe("SplitPane");expect(all.find(n=>n.key==="history-agents-drawer")).toMatchObject({_tag:"IconButton",icon:"Agent"});expect(all.find(n=>n.key==="history-center-title")).toBeUndefined();expect(all.find(n=>n.key==="history-center-status")).toBeUndefined();expect(all.find(n=>n.key==="history-completeness")).toBeUndefined();expect(all.find(n=>n.key==="history-agent-config")).toBeUndefined();expect(all.find(n=>n.key==="history-agent-list")).toMatchObject({_tag:"NavRail",role:"tree",activeId:"history-agent-child"});expect(root).toMatchObject({icon:"Check"});expect(root.meta).toBeUndefined();expect(child).toMatchObject({depth:1,selected:true,icon:"Play"});expect(child.meta).toBeUndefined();expect(child.accessibilityLabel).toContain("Running")})
  test("selected item opens structured detail with back action",()=>{const all=nodes(historyWorkspaceView(stateWith(page,"child:0"))) as any[];expect(all.find(n=>n.key==="history-item-inspector")).toBeDefined();expect(all.find(n=>n.key==="history-item-back")?.onPress.name).toBe("HistoryItemSelected")})
  test("agent metadata expands inline without replacing the agent tree",()=>{const metadata={...page.items[0]!,itemRef:"child:metadata",kind:"metadata" as const,label:"Agent metadata",summary:"# AGENTS.md instructions for /workspace\n\n<INSTRUCTIONS>\nKeep the contract.\n</INSTRUCTIONS>"};const metadataPage={...page,items:[metadata]};const all=nodes(historyWorkspaceView(stateWith(metadataPage,"child:metadata"))) as any[];const events=timelineEvents(all);expect(all.find(n=>n.key==="history-agent-tree-region")).toBeDefined();expect(all.find(n=>n.key==="history-item-inspector")).toBeUndefined();expect(events[0]).toMatchObject({label:"Agent metadata",time:"Click to collapse",variant:"metadata",icon:"ChevronDown"});expect(events[0].detail).toContain("Keep the contract.")})
  test("bounded timeline uses typed selectable events and compact row previews",()=>{const long={...page,items:[{...page.items[0]!,summary:"x".repeat(2_000)}]};const all=nodes(historyWorkspaceView(stateWith(long))) as any[];const container=all.find(n=>n.key==="history-timeline-page");expect(container._tag).toBe("Stack");const timeline=all.find(n=>n._tag==="Timeline");expect(timeline.onEventSelect.name).toBe("HistoryItemSelected");const row=timeline.events[0];expect(row.key).toBe("history-item-child:0");expect(row.detail.length).toBeLessThan(500);expect(row.detail.endsWith("…")).toBe(true)})
  test("projects the raw ledger into prose entries, composed tool rows, and agent cards",()=>{
    const entries=projectHistoryEntries([
      item(0,"session","Session started","Codex session metadata"),
      item(1,"system_message","Message · developer","internal instructions"),
      item(2,"usage","Usage","Token usage update"),
      item(7,"reasoning","Reasoning summary","[REDACTED: reasoning not persisted as summary]"),
      item(8,"assistant_message","Assistant","Assistant"),
      item(3,"user_message","You","Run the tests"),
      item(4,"tool_call","exec_command","bun test",[{label:"call",value:"call-1"},{label:"input",value:"bun test"}]),
      item(5,"tool_result","Tool result","12 passed",[{label:"call",value:"call-1"}]),
      {...item(9,"collaboration","sub agent activity","started",[{label:"agent",value:"worker"},{label:"operation",value:"sub_agent_activity"},{label:"activity",value:"started"}]),relatedAgent:{threadRef:"worker",title:"Roadmap audit",status:"running",updatedAt:"2026-07-10T00:00:04Z",latest:{label:"Assistant",summary:"Reviewing the roadmap milestones",kind:"assistant_message",timestamp:"2026-07-10T00:00:03Z"}}},
      item(6,"assistant_message","Assistant","All tests pass"),
    ])
    // Every retained source item projects exactly once: prose OR event.
    expect(entries.map(entry=>entry.kind==="prose"?`prose:${entry.item.itemRef}`:entry.events.map(event=>`event:${event.id}`).join(","))).toEqual(["prose:child:3","event:child:4,event:child:9","prose:child:6"])
    const events=entries.flatMap(entry=>entry.kind==="events"?entry.events:[])
    expect(events[0]).toMatchObject({label:"Terminal",detail:"bun test",variant:"tool",icon:"Terminal",status:"success",refs:["child","child:5"]})
    expect(events[1]).toMatchObject({label:"Subagent · Roadmap audit",detail:"Assistant — Reviewing the roadmap milestones",variant:"agent",icon:"Agent",status:"active",time:"Running",refs:["child","worker"]})
    expect(events[1]?.onSelect?.name).toBe("HistoryAgentSelected")
    const metadata=item(10,"metadata","Agent metadata","# AGENTS.md instructions for /workspace\n\n<INSTRUCTIONS>\nKeep the contract.\n</INSTRUCTIONS>")
    expect(projectHistoryTimelineEvents([metadata])[0]).toMatchObject({label:"Agent metadata",time:"Click to expand",variant:"metadata",icon:"ChevronRight"})
    expect(projectHistoryTimelineEvents([metadata])[0]?.detail).toBeUndefined()
    expect(projectHistoryTimelineEvents([metadata],metadata.itemRef)[0]).toMatchObject({time:"Click to collapse",icon:"ChevronDown"})
    const handoff=item(11,"agent_message","Agent message","Message Type: NEW_TASK",[{label:"message type",value:"NEW_TASK"},{label:"task",value:"/root/roadmap_audit"},{label:"sender",value:"/root"},{label:"recipient",value:"/root/roadmap_audit"},{label:"payload",value:"Review the roadmap."}])
    const handoffEntries=projectHistoryEntries([handoff])
    expect(handoffEntries[0]?.kind).toBe("prose")
    const routeless=item(12,"agent_message","Agent message","Message Type: CLOSE",[{label:"message type",value:"CLOSE"}])
    expect(projectHistoryTimelineEvents([routeless])[0]).toMatchObject({label:"Agent closed",variant:"agent",icon:"Agent"})
  })

  // EP250 owner contract: "i see assistant messages showing raw markdown what
  // the fuck. need to use the same markdown renderer we use elsewhere"
  describe("message prose renders through the shared markdown projector",()=>{
    const assistant=item(1,"assistant_message","Assistant","### [Episode 248](/path/to/ep248) intro\n\nShip **bold** work with `inline()` code.\n\n```ts\nrunTests()\n```")
    const prosePage={...page,items:[assistant],totalItems:1}
    test("assistant markdown becomes typed Markdown/CodeBlock views with no literal markers",()=>{
      const all=nodes(historyWorkspaceView(stateWith(prosePage))) as any[]
      const row=all.find(n=>n.key==="history-item-child:1")
      expect(row?._tag).toBe("Stack")
      const rowNodes=nodes(row)
      const headings=markdownNodes(rowNodes,"heading")
      expect(headings.length).toBe(1)
      expect(headings[0].level).toBe(3)
      expect(markdownNodes(rowNodes,"strong").length).toBe(1)
      expect(markdownNodes(rowNodes,"code").length).toBe(1)
      expect(rowNodes.some(n=>n._tag==="CodeBlock")).toBe(true)
      const texts=rowNodes.filter(n=>n._tag==="Markdown").flatMap(markdownTexts)
      expect(texts.join(" ")).not.toContain("###")
      expect(texts.join(" ")).not.toContain("**")
      // Links render as safe text — label plus visible path, no navigation.
      expect(texts.join(" ")).toContain("Episode 248")
      expect(texts.join(" ")).toContain("(/path/to/ep248)")
      // The prose item is NOT double-rendered as a timeline event.
      expect(timelineEvents(nodes(historyWorkspaceView(stateWith(prosePage)))).some(event=>event.id==="child:1")).toBe(false)
      // Details affordance dispatches the SAME inspector intent timeline rows use.
      const details=nodes(historyWorkspaceView(stateWith(prosePage))).find(n=>n.key==="history-item-details-child:1")
      expect(details?.onPress?.name).toBe("HistoryItemSelected")
      expect(details).toMatchObject({_tag:"Button",variant:"ghost"})
      expect(details.style).toMatchObject({typeScale:"caption",color:"textFaint"})
    })
    test("user and agent-message prose route through the same projector",()=>{
      const user=item(2,"user_message","You","Please check `pkg.json` and **fix** it")
      const handoff=item(3,"agent_message","Agent message","Message Type: NEW_TASK",[{label:"message type",value:"NEW_TASK"},{label:"task",value:"/root/audit"},{label:"sender",value:"/root"},{label:"recipient",value:"/root/audit"},{label:"payload",value:"Audit the **roadmap** now."}])
      const all=nodes(historyWorkspaceView(stateWith({...page,items:[user,handoff]}))) as any[]
      const userRow=nodes(all.find(n=>n.key==="history-item-child:2"))
      expect(userRow.find(n=>n.key==="history-item-child:2-header")?.content).toBe("YOU")
      expect(markdownNodes(userRow,"strong").length).toBe(1)
      expect(markdownNodes(userRow,"code").length).toBe(1)
      const handoffRow=nodes(all.find(n=>n.key==="history-item-child:3"))
      const header=handoffRow.find(n=>n.key==="history-item-child:3-header")
      expect(header?.content).toContain("Task assigned · audit")
      expect(header?.content).toContain("root → audit")
      expect(header?.color).toBe("textFaint")
      expect(markdownNodes(handoffRow,"strong").length).toBe(1)
      expect(markdownNodes(handoffRow,"strong")[0]?.children?.[0]?.text).toBe("roadmap")
    })
    test("loss-accounting notices stay plain text, apart from the prose projection",()=>{
      const redactedAssistant={...item(4,"assistant_message","Assistant","[REDACTED: message not persisted]"),redacted:true}
      const gap=item(5,"gap","Unreadable source record","This record could not be decoded.")
      const all=nodes(historyWorkspaceView(stateWith({...page,items:[redactedAssistant,gap]}))) as any[]
      // Neither notice renders as markdown prose; both remain plain event rows.
      expect(all.find(n=>n.key==="history-item-child:4")).toBeUndefined()
      expect(all.filter(n=>n._tag==="Markdown").length).toBe(0)
      const events=timelineEvents(all)
      expect(events.map(event=>event.id)).toEqual(["child:4","child:5"])
      expect(events[1]).toMatchObject({variant:"error",status:"failed"})
      // Inline [REDACTED] markers inside real prose stay literal text.
      const inline=item(6,"assistant_message","Assistant","Kept **this** but [REDACTED] elsewhere.")
      const proseNodes=nodes(historyWorkspaceView(stateWith({...page,items:[inline]}))) as any[]
      const texts=proseNodes.filter(n=>n._tag==="Markdown").flatMap(markdownTexts)
      expect(texts.join(" ")).toContain("[REDACTED]")
    })
    test("the item inspector renders prose bodies through the projector and notices as plain text",()=>{
      const assistantAll=nodes(historyWorkspaceView(stateWith(prosePage,"child:1"))) as any[]
      const inspector=assistantAll.find(n=>n.key==="history-item-inspector")
      expect(inspector).toBeDefined()
      const inspectorNodes=nodes(inspector)
      expect(markdownNodes(inspectorNodes,"heading").length).toBe(1)
      expect(inspectorNodes.find(n=>n.key==="history-item-summary")).toBeUndefined()
      const gap=item(5,"gap","Unreadable source record","This record could not be decoded.")
      const gapAll=nodes(historyWorkspaceView(stateWith({...page,items:[gap]},"child:5"))) as any[]
      const gapInspector=nodes(gapAll.find(n=>n.key==="history-item-inspector"))
      expect(gapInspector.find(n=>n.key==="history-item-summary")?.content).toBe("This record could not be decoded.")
      expect(gapInspector.filter(n=>n._tag==="Markdown").length).toBe(0)
    })
  })

  // EP250 owner contract: "spawn agent card is still showing a fucking json
  // object in the card tool thing, not good"
  describe("historical tool cards humanize through the shared chat table",()=>{
    const blob="gAAAAB"+"Zm9ya3R1cm5zY29udGludWF0aW9uYmxvYjEyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejEyMzQ1Njc4OTA"
    test("spawn_agent renders a humanized primary line and never the continuation blob",()=>{
      const spawn=item(1,"tool_call","spawn_agent",JSON.stringify({task_name:"issue_audit",fork_turns:"3",message:blob}),[{label:"input",value:JSON.stringify({task_name:"issue_audit",fork_turns:"3",message:blob})}])
      const events=projectHistoryTimelineEvents([spawn])
      expect(events[0]?.label).toBe("Spawn agent")
      expect(events[0]?.detail).toBe("issue_audit · fork turns: 3")
      expect(events[0]?.detail).not.toContain("gAAAA")
      expect(events[0]?.detail).not.toContain("{")
      // The raw input (blob included) stays reachable through the inspector.
      const all=nodes(historyWorkspaceView(stateWith({...page,items:[spawn]},"child:1"))) as any[]
      const inspector=nodes(all.find(n=>n.key==="history-item-inspector"))
      expect(inspector.some(n=>typeof n.content==="string"&&n.content.includes("gAAAA"))).toBe(true)
    })
    test("table-driven humanization for the other historical item kinds",()=>{
      const cases:ReadonlyArray<[string,string,string,string]> = [
        ["exec_command",JSON.stringify({command:"bun test --watch"}),"Terminal","bun test --watch"],
        ["shell",JSON.stringify({cmd:"ls -la"}),"Terminal","ls -la"],
        ["web_search",JSON.stringify({query:"effect native timeline"}),"Web search","effect native timeline"],
        ["read_file",JSON.stringify({path:"/repo/src/index.ts"}),"Read file","/repo/src/index.ts"],
        ["grep",JSON.stringify({pattern:"HistoryAgentSelected"}),"Searched files","HistoryAgentSelected"],
        ["apply_patch","*** Begin Patch\n*** Update File: src/renderer/shell.ts\n@@\n*** End Patch","Edited files","src/renderer/shell.ts"],
        ["mcp__codex__spawn",JSON.stringify({task:"triage"}),"Spawn","task: triage"],
      ]
      for(const [label,args,title,detail] of cases){
        const events=projectHistoryTimelineEvents([item(1,"tool_call",label,args,[{label:"input",value:args}])])
        expect(events[0]?.label).toBe(title)
        expect(events[0]?.detail).toBe(detail)
        expect(events[0]?.detail).not.toContain("{")
      }
      // Unknown tools: prettified name + bounded key:value summary, no braces dump.
      const unknown=projectHistoryTimelineEvents([item(2,"tool_call","update_plan",JSON.stringify({explanation:"Reorder milestones",step:"draft"}))])
      expect(unknown[0]?.label).toBe("Update plan")
      expect(unknown[0]?.detail).toBe("explanation: Reorder milestones · step: draft")
    })
  })

  // EP250 owner contract: "just like command up and down scrolsl thru chats,
  // have command shift up and down go up and down the agents of a convo."
  describe("Cmd+Shift agent traversal walks the visible agent roster",()=>{
    const agent=(threadRef:string,parentThreadRef:string|null,depth:number,descendantCount:number)=>({threadRef,parentThreadRef,title:threadRef,status:"completed" as const,createdAt:"2026-07-10T00:00:00Z",updatedAt:"2026-07-10T00:00:00Z",depth,descendantCount,model:null,role:null,nickname:null,agentPath:null,sourceVersion:null,reasoning:null})
    const rosterPage:CodexHistoryPage={...page,selectedThreadRef:"b",agents:[agent("root",null,0,3),agent("a","root",1,0),agent("b","root",1,1),agent("b1","b",2,0)]}
    const rosterState=(selected:string,expanded:ReadonlyArray<string>=["root","b"]):HistoryWorkspaceState=>({catalog:{roots:[],agents:[]},page:{...rosterPage,selectedThreadRef:selected},selectedItemRef:null,railCollapsed:false,expandedThreadRefs:expanded,visibleRootCount:40})
    test("traverses down and up over the same roster the agent tree renders",()=>{
      expect(visibleHistoryAgents(rosterState("b")).map(a=>a.threadRef)).toEqual(["root","a","b","b1"])
      expect(historyAgentTraversalTarget(rosterState("b"),1)).toBe("b1")
      expect(historyAgentTraversalTarget(rosterState("b"),-1)).toBe("a")
      // Collapsed subtrees are skipped exactly like the rendered tree.
      expect(visibleHistoryAgents(rosterState("b",["root"])).map(a=>a.threadRef)).toEqual(["root","a","b"])
      expect(historyAgentTraversalTarget(rosterState("b",["root"]),1)).toBeNull()
    })
    test("ends clamp — the same boundary behavior as conversation traversal",()=>{
      expect(historyAgentTraversalTarget(rosterState("root"),-1)).toBeNull()
      expect(historyAgentTraversalTarget(rosterState("b1"),1)).toBeNull()
    })
    test("no-ops without an open conversation or roster",()=>{
      expect(historyAgentTraversalTarget({catalog:{roots:[],agents:[]},page:null,selectedItemRef:null,railCollapsed:false,expandedThreadRefs:[],visibleRootCount:40},1)).toBeNull()
      // Selected agent hidden by a collapsed subtree -> falls to the roster edge.
      expect(historyAgentTraversalTarget(rosterState("b",[]) ,1)).toBe("root")
    })
    test("dispatches the SAME intent the agent tree rows dispatch",()=>{
      const all=nodes(historyWorkspaceView(rosterState("b"))) as any[]
      const row=all.find(n=>n.id==="history-agent-a")
      expect(row?.onSelect?.name).toBe("HistoryAgentSelected")
    })
    test("shifted-vs-unshifted discrimination per platform",()=>{
      const chord=(overrides:Partial<{key:string;metaKey:boolean;ctrlKey:boolean;altKey:boolean;shiftKey:boolean}>)=>({key:"ArrowDown",metaKey:false,ctrlKey:false,altKey:false,shiftKey:false,...overrides})
      // Cmd+Shift+Down on darwin -> agents; Cmd+Down stays conversations.
      expect(isHistoryAgentTraversalShortcut(chord({metaKey:true,shiftKey:true}),"darwin")).toBe(true)
      expect(isHistoryAgentTraversalShortcut(chord({metaKey:true}),"darwin")).toBe(false)
      expect(isHistoryAgentTraversalShortcut(chord({key:"ArrowUp",metaKey:true,shiftKey:true}),"darwin")).toBe(true)
      expect(isHistoryAgentTraversalShortcut(chord({ctrlKey:true,shiftKey:true}),"darwin")).toBe(false)
      expect(isHistoryAgentTraversalShortcut(chord({metaKey:true,shiftKey:true,altKey:true}),"darwin")).toBe(false)
      expect(isHistoryAgentTraversalShortcut(chord({metaKey:true,shiftKey:true,key:"1"}),"darwin")).toBe(false)
      // Non-darwin uses Ctrl+Shift.
      expect(isHistoryAgentTraversalShortcut(chord({ctrlKey:true,shiftKey:true}),"linux")).toBe(true)
      expect(isHistoryAgentTraversalShortcut(chord({metaKey:true,shiftKey:true}),"linux")).toBe(false)
    })
  })
})
