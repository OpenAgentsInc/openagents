/**
 * Public-safe, real-Electron Codex trace acceptance (#8675).
 *
 * These scripts execute inside the signed renderer from Electron main. They
 * deliberately return only timings and aggregate counts: titles, transcript
 * text, local paths, and stable private refs never enter smoke output.
 */
export const traceAcceptanceJourney = `(async () => {
  const started = performance.now()
  const wait = (ms) => new Promise(resolve => setTimeout(resolve, ms))
  const until = async (read, timeout = 15000) => {
    const deadline = Date.now() + timeout
    let value
    while (Date.now() < deadline) {
      value = read()
      if (value) return value
      await wait(50)
    }
    return null
  }
  const bridge = globalThis.openagentsDesktop
  if (typeof bridge?.runtimeRequest !== "function") return { ok:false, reason:"bridge_missing" }
  const shell = await until(() => document.querySelector('[data-en-key="shell-root"]'))
  const shellReadyMs = Math.round(performance.now() - started)
  if (!shell) return { ok:false, reason:"blank_shell" }
  const catalogStart = performance.now()
  const catalogResponse = await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-catalog",query:{id:"codex.history.catalog"}})
  const catalogReadyMs = Math.round(performance.now() - catalogStart)
  if (catalogResponse?.kind !== "codex_history_catalog") return {ok:false,reason:"catalog_unavailable"}
  const { roots, agents } = catalogResponse.catalog
  if (roots.length === 0) return {ok:false,reason:"real_history_empty"}
  const forbiddenTitle = /^(untitled|codex history|new chat)$/i
  if (roots.some(root => !root.title.trim() || forbiddenTitle.test(root.title.trim()))) return {ok:false,reason:"known_title_fallback"}
  if (roots.some((root,index) => index > 0 && root.updatedAt > roots[index-1].updatedAt)) return {ok:false,reason:"catalog_order"}
  const sidebarRows = [...document.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')]
  const rootRefs = new Set(roots.map(root => root.threadRef))
  const sidebarList = [...document.querySelectorAll('[aria-label]')].find(node => node.getAttribute('aria-label')?.endsWith(' of ' + roots.length + ' Codex conversations'))
  if (!sidebarList || sidebarRows.length === 0 || sidebarRows.some(row => !rootRefs.has(row.getAttribute('data-en-key').slice('sidebar-thread-'.length)))) return {ok:false,reason:"child_leaked_to_sidebar"}
  if ([...document.querySelectorAll('[data-en-key*="loading"]')].some(node => node.getClientRects().length > 0)) return {ok:false,reason:"stale_loading_copy"}

  const rootsWithTopology = roots.map(root => {
    const children = agents.filter(agent => agent.parentThreadRef === root.threadRef)
    const grandchild = agents.find(agent => children.some(child => agent.parentThreadRef === child.threadRef))
    return {root, children, grandchild}
  })
  const visibleRefs = new Set(sidebarRows.map(row => row.getAttribute('data-en-key').slice('sidebar-thread-'.length)))
  const visibleCandidates = rootsWithTopology.filter(value => visibleRefs.has(value.root.threadRef))
  const candidateOrder = [...visibleCandidates].sort((a,b) => (b.children.length >= 2 && b.grandchild ? 1 : 0) - (a.children.length >= 2 && a.grandchild ? 1 : 0) || b.root.descendantCount - a.root.descendantCount)
  const candidate = candidateOrder[0]
  if (!candidate) return {ok:false,reason:"candidate_not_visible"}
  const rootPageResponse = await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-root-page",query:{id:"codex.history.page",threadRef:candidate.root.threadRef,offset:0,limit:50}})
  if (rootPageResponse?.kind !== "codex_history_page") return {ok:false,reason:"root_page_unavailable"}
  let inlineCandidate = null
  let inlinePageResponse = null
  for (const value of candidateOrder) {
    const response = value.root.threadRef === candidate.root.threadRef ? rootPageResponse : await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-inline-page",query:{id:"codex.history.page",threadRef:value.root.threadRef,offset:0,limit:50}})
    if (response?.kind === "codex_history_page" && response.page.items.some(item => item.relatedAgent)) { inlineCandidate=value; inlinePageResponse=response; break }
  }
  if (!inlineCandidate || inlinePageResponse?.kind !== "codex_history_page") return {ok:false,reason:"inline_agent_preview_missing"}
  const selectedRef=()=>{try{return JSON.parse(localStorage.getItem('openagents.desktop.history.v1')??'null')?.selectedThreadRef??null}catch{return null}}
  const inlineRootButton = sidebarRows.find(row => row.getAttribute('data-en-key') === 'sidebar-thread-' + inlineCandidate.root.threadRef)
  inlineRootButton?.click()
  if (!await until(() => selectedRef() === inlineCandidate.root.threadRef && document.querySelector('[data-en-key="history-workspace-split"]'))) return {ok:false,reason:"workspace_not_ready"}
  const inlineItems = inlinePageResponse.page.items.filter(item => item.relatedAgent)
  if (inlineItems.length === 0) return {ok:false,reason:"inline_agent_preview_missing"}
  const inlineItem = inlineItems[0]
  const inlineCard = await until(() => document.querySelector('[data-en-key="history-item-' + inlineItem.itemRef + '"][data-en-variant="agent"]'))
  if (!inlineCard) return {ok:false,reason:"inline_agent_preview_missing"}
  const latestSummary = inlineItem.relatedAgent.latest?.summary?.slice(0,40)
  if (latestSummary && !inlineCard.textContent?.includes(latestSummary)) return {ok:false,reason:"inline_agent_preview_stale"}
  inlineCard.click()
  if (!await until(() => selectedRef() === inlineItem.relatedAgent.threadRef && document.querySelector('[data-en-key="history-agent-' + inlineItem.relatedAgent.threadRef + '"][aria-selected="true"]'))) return {ok:false,reason:"inline_agent_navigation_failed"}
  const rootButton = [...document.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')].find(row => row.getAttribute('data-en-key') === 'sidebar-thread-' + candidate.root.threadRef)
  if (!rootButton) return {ok:false,reason:"candidate_not_visible"}
  const selectionStart = performance.now()
  rootButton.click()
  if (!await until(() => selectedRef() === candidate.root.threadRef)) return {ok:false,reason:"inline_agent_return_failed"}
  const pageReadyMs = Math.round(performance.now() - selectionStart)
  const page = rootPageResponse.page
  const completeness = page.completeness
  if (completeness.source !== completeness.rendered + completeness.redactions + completeness.gaps) return {ok:false,reason:"silent_loss"}
  const metadataItem = page.items.find(item => item.kind === 'metadata')
  if (!metadataItem) return {ok:false,reason:"agent_metadata_missing"}
  const collapsedMetadata = await until(() => document.querySelector('[data-en-key="history-item-' + metadataItem.itemRef + '"][data-en-variant="metadata"]'))
  if (!collapsedMetadata || collapsedMetadata.querySelector('[data-en-role="detail"]') || !collapsedMetadata.textContent?.includes('Click to expand')) return {ok:false,reason:"agent_metadata_not_collapsed"}
  collapsedMetadata.click()
  const expandedMetadata = await until(() => { const row=document.querySelector('[data-en-key="history-item-' + metadataItem.itemRef + '"][aria-selected="true"]');return row?.querySelector('[data-en-role="detail"]')?row:null })
  if (!expandedMetadata) return {ok:false,reason:"agent_metadata_expand_failed"}
  if (!document.querySelector('[data-en-key="history-agent-list"]')) return {ok:false,reason:"agent_metadata_replaced_tree"}
  expandedMetadata.click()
  if (!await until(() => { const row=document.querySelector('[data-en-key="history-item-' + metadataItem.itemRef + '"]');return row&&row.getAttribute('aria-selected')==='false'&&!row.querySelector('[data-en-role="detail"]')?row:null })) return {ok:false,reason:"agent_metadata_collapse_failed"}
  const candidateIndex=roots.findIndex(root=>root.threadRef===candidate.root.threadRef)
  const modifier=bridge.platform==='darwin'?{metaKey:true}:{ctrlKey:true}
  const modifierKey=bridge.platform==='darwin'?'Meta':'Control'
  window.dispatchEvent(new KeyboardEvent('keydown',{key:modifierKey,code:bridge.platform==='darwin'?'MetaLeft':'ControlLeft',bubbles:true,cancelable:true,...modifier}))
  const firstHint=await until(()=>document.querySelector('[data-en-key="sidebar-thread-'+roots[0].threadRef+'"] [data-en-role="meta"]')?.textContent==='1')
  if(!firstHint)return {ok:false,reason:'history_shortcut_hint_missing'}
  window.dispatchEvent(new KeyboardEvent('keydown',{key:'1',code:'Digit1',bubbles:true,cancelable:true,...modifier}))
  const numberLoaded=await until(()=>selectedRef()===roots[0].threadRef)
  if(!numberLoaded)return {ok:false,reason:'history_shortcut_number_failed'}
  window.dispatchEvent(new KeyboardEvent('keyup',{key:modifierKey,code:bridge.platform==='darwin'?'MetaLeft':'ControlLeft',bubbles:true,cancelable:true}))
  const candidateButtonAgain=[...document.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')].find(row=>row.getAttribute('data-en-key')==='sidebar-thread-'+candidate.root.threadRef)
  candidateButtonAgain?.click()
  if(!await until(()=>selectedRef()===candidate.root.threadRef))return {ok:false,reason:'history_shortcut_restore_failed'}
  const shortcutTarget=roots[candidateIndex+1]
  if(shortcutTarget){
    window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',bubbles:true,cancelable:true,...modifier}))
    const downLoaded=await until(()=>selectedRef()===shortcutTarget.threadRef)
    if(!downLoaded)return {ok:false,reason:'history_shortcut_down_failed'}
    window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowUp',code:'ArrowUp',bubbles:true,cancelable:true,...modifier}))
    const upLoaded=await until(()=>selectedRef()===candidate.root.threadRef)
    if(!upLoaded)return {ok:false,reason:'history_shortcut_up_failed'}
  }
  const heldTargetIndex=Math.min(roots.length-1,candidateIndex+45)
  const heldTarget=roots[heldTargetIndex]
  if(heldTargetIndex>candidateIndex){
    for(let index=candidateIndex;index<heldTargetIndex;index++)window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',repeat:true,bubbles:true,cancelable:true,...modifier}))
    const heldLoaded=await until(()=>selectedRef()===heldTarget.threadRef)
    if(!heldLoaded)return {ok:false,reason:'history_shortcut_hold_failed'}
    const heldVisible=await until(()=>{
      const heldRow=[...document.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')].find(row=>row.getAttribute('data-en-key')==='sidebar-thread-'+heldTarget.threadRef)
      const sidebarList=document.querySelector('[data-en-key="sidebar-history-list"]')
      const rowRect=heldRow?.getBoundingClientRect();const listRect=sidebarList?.getBoundingClientRect()
      return rowRect&&listRect&&rowRect.top>=listRect.top-1&&rowRect.bottom<=listRect.bottom+1?true:null
    },5000)
    if(!heldVisible){const sidebarList=document.querySelector('[data-en-key="sidebar-history-list"]');const rows=[...(sidebarList?.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')??[])];return {ok:false,reason:'history_shortcut_offscreen',listScrollTop:sidebarList?.scrollTop??0,listScrollHeight:sidebarList?.scrollHeight??0,listClientHeight:sidebarList?.clientHeight??0,rowCount:rows.length,firstRowHeight:rows[0]?.getBoundingClientRect().height??0,targetIndex:rows.findIndex(row=>row.getAttribute('data-en-key')==='sidebar-thread-'+heldTarget.threadRef),overflowY:sidebarList?getComputedStyle(sidebarList).overflowY:'missing'}}
    for(let index=candidateIndex;index<heldTargetIndex;index++)window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowUp',code:'ArrowUp',repeat:true,bubbles:true,cancelable:true,...modifier}))
    const heldReturned=await until(()=>selectedRef()===candidate.root.threadRef)
    if(!heldReturned)return {ok:false,reason:'history_shortcut_hold_return_failed'}
    window.dispatchEvent(new KeyboardEvent('keyup',{key:modifierKey,code:bridge.platform==='darwin'?'MetaLeft':'ControlLeft',bubbles:true,cancelable:true}))
  }
  const rootButtonBeforeTree = [...document.querySelectorAll('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]')].find(row => row.getAttribute('data-en-key') === 'sidebar-thread-' + candidate.root.threadRef)
  rootButtonBeforeTree?.click()
  if (!await until(() => selectedRef() === candidate.root.threadRef && document.querySelectorAll('[role="treeitem"]').length > 1)) return {ok:false,reason:"descendants_hidden",visibleAgentCount:document.querySelectorAll('[role="treeitem"]').length,projectedAgentCount:page.agents.length}
  const treeItems = [...document.querySelectorAll('[role="treeitem"]')]
  const agentList = [...document.querySelectorAll('[aria-label]')].find(node => node.getAttribute('aria-label') === page.agents.length + ' agents')
  if (treeItems.length === 0 || !agentList) return {ok:false,reason:"descendants_hidden",visibleAgentCount:treeItems.length,projectedAgentCount:page.agents.length}
  if (treeItems.some(item => item.querySelector('[data-en-role="meta"]'))) return {ok:false,reason:"agent_status_word_visible"}
  if (treeItems.some(item => item.querySelector('[data-en-icon]') === null)) return {ok:false,reason:"agent_status_icon_missing"}
  const selectedBefore = [...document.querySelectorAll('[data-en-key^="history-agent-"]')].find(item => item.getAttribute('data-en-key') === 'history-agent-' + candidate.root.threadRef) ?? treeItems[0]
  selectedBefore?.focus()
  const keyboardEvent = new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',bubbles:true,cancelable:true})
  selectedBefore?.dispatchEvent(keyboardEvent)
  await wait(300)
  if (!keyboardEvent.defaultPrevented) return {ok:false,reason:"keyboard_tree_stuck"}

  let toolPage = null
  let communicationPage = null
  for (const agent of page.agents) {
    const response = await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-tool-page",query:{id:"codex.history.page",threadRef:agent.threadRef,offset:0,limit:50}})
    if (response?.kind !== "codex_history_page") continue
    if (!toolPage && response.page.items.some(item => item.kind === "tool_call" || item.kind === "tool_result")) toolPage=response.page
    if (!communicationPage && response.page.items.some(item => item.kind === "agent_message" && item.fields.some(field => field.label === "message type" && field.value === "NEW_TASK"))) communicationPage=response.page
    if (toolPage && communicationPage) break
  }
  if (!toolPage) return {ok:false,reason:"tool_trace_missing"}
  if (!communicationPage) return {ok:false,reason:"agent_handoff_missing"}
  const communicationAgentButton = [...document.querySelectorAll('[data-en-key^="history-agent-"]')].find(row => row.getAttribute('data-en-key') === 'history-agent-' + communicationPage.selectedThreadRef)
  communicationAgentButton?.click()
  await until(() => document.querySelector('[data-en-key="history-agent-' + communicationPage.selectedThreadRef + '"][aria-selected="true"]'))
  const handoffItem = communicationPage.items.find(item => item.kind === 'agent_message' && item.fields.some(field => field.label === 'message type' && field.value === 'NEW_TASK'))
  const protocolItems = communicationPage.items.filter(item => item.label === 'Agent communication metadata' || item.label === 'Plugin metadata')
  if (protocolItems.some(item => document.querySelector('[data-en-key="history-item-' + item.itemRef + '"]'))) return {ok:false,reason:"protocol_metadata_visible"}
  const handoffButton = await until(() => document.querySelector('[data-en-key="history-item-' + handoffItem.itemRef + '"][data-en-variant="agent"]'))
  if (!handoffButton || !handoffButton.textContent?.includes('Task assigned') || handoffButton.textContent?.includes('Message Type:')) return {ok:false,reason:"agent_handoff_card_incomplete"}
  handoffButton.click()
  const handoffInspector = await until(() => {const inspector=document.querySelector('[data-en-key="history-item-inspector"]');const fields=inspector?.querySelector('[data-en-key="history-item-fields"]')?.textContent??'';const kind=inspector?.querySelector('[data-en-key="history-item-kind"]')?.textContent;return kind===handoffItem.kind&&['message type','task','sender','recipient'].every(label=>fields.includes(label))?inspector:null})
  const handoffFields = handoffInspector?.querySelector('[data-en-key="history-item-fields"]')?.textContent ?? ''
  const handoffFieldChecks={messageType:handoffFields.includes('message type'),task:handoffFields.includes('task'),sender:handoffFields.includes('sender'),recipient:handoffFields.includes('recipient')}
  if (!Object.values(handoffFieldChecks).every(Boolean)) return {ok:false,reason:"agent_handoff_inspector_incomplete",fieldChecks:handoffFieldChecks,fieldLabels:[...(handoffInspector?.querySelectorAll('[data-en-key^="history-item-field-label-"]')??[])].map(node=>node.textContent)}
  document.querySelector('[data-en-key="history-item-back"]')?.click()
  await until(() => document.querySelector('[data-en-key="history-agent-list"]'))
  const agentButton = [...document.querySelectorAll('[data-en-key^="history-agent-"]')].find(row => row.getAttribute('data-en-key') === 'history-agent-' + toolPage.selectedThreadRef)
  if (agentButton?.getAttribute('aria-selected') !== 'true') agentButton?.click()
  await until(() => document.querySelector('[data-en-key="history-agent-' + toolPage.selectedThreadRef + '"][aria-selected="true"]'))
  const toolRef = (toolPage.items.find(item => item.kind === 'tool_call') ?? toolPage.items.find(item => item.kind === 'tool_result')).itemRef
  const toolButton = await until(() => [...document.querySelectorAll('[data-en-key^="history-item-"]')].find(row => row.getAttribute('data-en-key') === 'history-item-' + toolRef))
  if (!toolButton) return {ok:false,reason:"tool_row_inaccessible"}
  toolButton.click()
  const inspectorStart = performance.now()
  const inspector = await until(() => document.querySelector('[data-en-key="history-item-inspector"]'))
  const inspectorReadyMs = Math.round(performance.now() - inspectorStart)
  if (!inspector || !document.querySelector('[data-en-key="history-item-back"]')) return {ok:false,reason:"inspector_inaccessible"}
  const timeline = document.querySelector('[data-en-key="history-timeline-page"]')
  if (!timeline || timeline.clientHeight < 100 || timeline.scrollHeight <= timeline.clientHeight) return {ok:false,reason:"timeline_not_scrollable",clientHeight:timeline?.clientHeight??0,scrollHeight:timeline?.scrollHeight??0}
  const beforeScroll = timeline.scrollTop
  timeline.scrollTop = Math.min(240, timeline.scrollHeight - timeline.clientHeight)
  timeline.dispatchEvent(new Event('scroll',{bubbles:true}))
  await wait(50)
  if (timeline.scrollTop <= beforeScroll) return {ok:false,reason:"timeline_scroll_stuck",clientHeight:timeline.clientHeight,scrollHeight:timeline.scrollHeight}
  const scrollBeforeModifier = timeline.scrollTop
  window.dispatchEvent(new KeyboardEvent('keydown',{key:modifierKey,code:bridge.platform==='darwin'?'MetaLeft':'ControlLeft',bubbles:true,cancelable:true,...modifier}))
  if (!await until(() => document.querySelector('[data-en-key="sidebar-thread-' + roots[0].threadRef + '"] [data-en-role="meta"]')?.textContent === '1')) return {ok:false,reason:"history_modifier_hint_missing",metaText:document.querySelector('[data-en-key="sidebar-thread-' + roots[0].threadRef + '"] [data-en-role="meta"]')?.textContent??null,rowPresent:document.querySelector('[data-en-key="sidebar-thread-' + roots[0].threadRef + '"]')!==null,metaPresent:document.querySelector('[data-en-key="sidebar-thread-' + roots[0].threadRef + '"] [data-en-role="meta"]')!==null}
  await wait(50)
  if (Math.abs(timeline.scrollTop-scrollBeforeModifier)>1) return {ok:false,reason:"history_modifier_scroll_reset",phase:"down"}
  window.dispatchEvent(new KeyboardEvent('keyup',{key:modifierKey,code:bridge.platform==='darwin'?'MetaLeft':'ControlLeft',bubbles:true,cancelable:true}))
  await until(() => document.querySelector('[data-en-key="sidebar-thread-' + roots[0].threadRef + '"] [data-en-role="meta"]')?.textContent !== '1')
  await wait(50)
  if (Math.abs(timeline.scrollTop-scrollBeforeModifier)>1) return {ok:false,reason:"history_modifier_scroll_reset",phase:"up"}
  const saved = JSON.parse(localStorage.getItem('openagents.desktop.history.v1') ?? 'null')
  if (!saved || typeof saved.selectedThreadRef !== 'string' || typeof saved.selectedItemRef !== 'string' || !Array.isArray(saved.expandedThreadRefs)) return {ok:false,reason:"ref_restore_missing"}
  sessionStorage.setItem('openagents.desktop.trace-acceptance.expected', JSON.stringify({selectedThreadRef:saved.selectedThreadRef,selectedItemRef:saved.selectedItemRef,expandedThreadRefs:saved.expandedThreadRefs}))
  return {ok:true,shellReadyMs,catalogReadyMs,pageReadyMs,inspectorReadyMs,rootCount:roots.length,agentCount:page.agents.length,childCount:candidate.children.length,grandchildCount:candidate.grandchild?1:0,inlinePreviewCount:inlineItems.length,toolItemCount:toolPage.items.filter(item=>item.kind==='tool_call'||item.kind==='tool_result').length,gapCount:completeness.gaps,timelineClientHeight:timeline.clientHeight,timelineScrollHeight:timeline.scrollHeight}
})()`

export const traceAcceptanceReload = `(async () => {
  const started=performance.now();const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));const deadline=Date.now()+15000
  const expected=JSON.parse(sessionStorage.getItem('openagents.desktop.trace-acceptance.expected')??'null')
  if(!expected)return {ok:false,reason:'restart_expectation_missing'}
  while(Date.now()<deadline&&document.querySelector('[data-en-key="history-workspace-split"]')===null)await wait(50)
  const restored=JSON.parse(localStorage.getItem('openagents.desktop.history.v1')??'null')
  const workspace=document.querySelector('[data-en-key="history-workspace-split"]')
  const inspector=document.querySelector('[data-en-key="history-item-inspector"]')
  const sameRefs=restored?.selectedThreadRef===expected.selectedThreadRef&&restored?.selectedItemRef===expected.selectedItemRef&&JSON.stringify(restored?.expandedThreadRefs)===JSON.stringify(expected.expandedThreadRefs)
  return {ok:sameRefs&&workspace!==null&&inspector!==null,reloadReadyMs:Math.round(performance.now()-started),selectionRestored:sameRefs&&workspace!==null,itemInspectorRestored:inspector!==null,expandedCount:Array.isArray(restored?.expandedThreadRefs)?restored.expandedThreadRefs.length:0}
})()`
