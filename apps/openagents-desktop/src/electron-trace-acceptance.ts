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
  const sidebarList = [...document.querySelectorAll('[aria-label]')].find(node => node.getAttribute('aria-label') === roots.length + ' Codex conversations')
  if (!sidebarList || sidebarRows.length === 0 || sidebarRows.some(row => !rootRefs.has(row.getAttribute('data-en-key').slice('sidebar-thread-'.length)))) return {ok:false,reason:"child_leaked_to_sidebar"}
  if ([...document.querySelectorAll('[data-en-key*="loading"]')].some(node => node.getClientRects().length > 0)) return {ok:false,reason:"stale_loading_copy"}

  const rootsWithTopology = roots.map(root => {
    const family = agents.filter(agent => agent.threadRef === root.threadRef || agent.parentThreadRef !== null)
    const children = agents.filter(agent => agent.parentThreadRef === root.threadRef)
    const grandchild = agents.find(agent => children.some(child => agent.parentThreadRef === child.threadRef))
    return {root, children, grandchild, family}
  })
  const visibleRefs = new Set(sidebarRows.map(row => row.getAttribute('data-en-key').slice('sidebar-thread-'.length)))
  const visibleCandidates = rootsWithTopology.filter(value => visibleRefs.has(value.root.threadRef))
  const candidate = visibleCandidates.find(value => value.children.length >= 2 && value.grandchild) ?? visibleCandidates.sort((a,b)=>b.root.descendantCount-a.root.descendantCount)[0]
  if (!candidate) return {ok:false,reason:"candidate_not_visible"}
  const rootButton = sidebarRows.find(row => row.getAttribute('data-en-key') === 'sidebar-thread-' + candidate.root.threadRef)
  if (!rootButton) return {ok:false,reason:"candidate_not_visible"}
  const selectionStart = performance.now()
  rootButton.click()
  const workspace = await until(() => document.querySelector('[data-en-key="history-workspace-split"]'))
  if (!workspace) return {ok:false,reason:"workspace_not_ready"}
  const pageReadyMs = Math.round(performance.now() - selectionStart)
  const rootPageResponse = await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-root-page",query:{id:"codex.history.page",threadRef:candidate.root.threadRef,offset:0,limit:200}})
  if (rootPageResponse?.kind !== "codex_history_page") return {ok:false,reason:"root_page_unavailable"}
  const page = rootPageResponse.page
  const completeness = page.completeness
  if (completeness.source !== completeness.rendered + completeness.redactions + completeness.gaps) return {ok:false,reason:"silent_loss"}
  const treeItems = [...document.querySelectorAll('[role="treeitem"]')]
  const agentList = [...document.querySelectorAll('[aria-label]')].find(node => node.getAttribute('aria-label') === page.agents.length + ' agents')
  if (treeItems.length === 0 || !agentList) return {ok:false,reason:"descendants_hidden",visibleAgentCount:treeItems.length,projectedAgentCount:page.agents.length}
  const selectedBefore = [...document.querySelectorAll('[data-en-key^="history-agent-"]')].find(item => item.getAttribute('data-en-key') === 'history-agent-' + candidate.root.threadRef) ?? treeItems[0]
  selectedBefore?.focus()
  const keyboardEvent = new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',bubbles:true,cancelable:true})
  selectedBefore?.dispatchEvent(keyboardEvent)
  await wait(300)
  if (!keyboardEvent.defaultPrevented) return {ok:false,reason:"keyboard_tree_stuck"}

  let toolPage = null
  for (const agent of page.agents) {
    const response = await bridge.runtimeRequest({kind:"query",requestId:"trace-acceptance-tool-page",query:{id:"codex.history.page",threadRef:agent.threadRef,offset:0,limit:200}})
    if (response?.kind === "codex_history_page" && response.page.items.some(item => item.kind === "tool_call" || item.kind === "tool_result")) { toolPage=response.page; break }
  }
  if (!toolPage) return {ok:false,reason:"tool_trace_missing"}
  const agentButton = [...document.querySelectorAll('[data-en-key^="history-agent-"]')].find(row => row.getAttribute('data-en-key') === 'history-agent-' + toolPage.selectedThreadRef)
  agentButton?.click()
  const toolRef = toolPage.items.find(item => item.kind === 'tool_call' || item.kind === 'tool_result').itemRef
  const toolButton = await until(() => [...document.querySelectorAll('[data-en-key^="history-item-"]')].find(row => row.getAttribute('data-en-key') === 'history-item-' + toolRef))
  if (!toolButton) return {ok:false,reason:"tool_row_inaccessible"}
  toolButton.click()
  const inspectorStart = performance.now()
  const inspector = await until(() => document.querySelector('[data-en-key="history-item-inspector"]'))
  const inspectorReadyMs = Math.round(performance.now() - inspectorStart)
  if (!inspector || !document.querySelector('[data-en-key="history-item-back"]')) return {ok:false,reason:"inspector_inaccessible"}
  const saved = JSON.parse(localStorage.getItem('openagents.desktop.history.v1') ?? 'null')
  if (!saved || typeof saved.selectedThreadRef !== 'string' || typeof saved.selectedItemRef !== 'string' || !Array.isArray(saved.expandedThreadRefs)) return {ok:false,reason:"ref_restore_missing"}
  sessionStorage.setItem('openagents.desktop.trace-acceptance.expected', JSON.stringify({selectedThreadRef:saved.selectedThreadRef,selectedItemRef:saved.selectedItemRef,expandedThreadRefs:saved.expandedThreadRefs}))
  return {ok:true,shellReadyMs,catalogReadyMs,pageReadyMs,inspectorReadyMs,rootCount:roots.length,agentCount:page.agents.length,childCount:candidate.children.length,grandchildCount:candidate.grandchild?1:0,toolItemCount:toolPage.items.filter(item=>item.kind==='tool_call'||item.kind==='tool_result').length,gapCount:completeness.gaps}
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
