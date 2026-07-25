/** Dependency-free document ownership shared by Start and the Worker. */
export const knownDocumentPathPatterns: ReadonlyArray<RegExp> = [
  /^\/app\/?$/,
  /^\/X\/$/,
  /^\/activity$/,
  // #9188: admin-only operator dashboard. Auth is enforced server-side by the
  // `/api/admin/operator/overview` endpoint; a non-admin gets the refusal
  // view.
  /^\/admin\/analytics$/,
  /^\/admin\/operator$/,
  /^\/(?:adjutant|artanis)\/?$/,
  // /aisdk + /aisdk/docs: public OpenAgents AI SDK page and docs, added at
  // owner direction 2026-07-21 (SDK extracted to OpenAgentsInc/ai and
  // published as the @openagentsinc rc train on npm).
  /^\/aisdk\/?$/,
  /^\/aisdk\/docs(?:\/[^/]+)?\/?$/,
  /^\/agents\/[^/]+$/,
  /^\/agentchat$/,
  /^\/agentchat\/signer-callback$/,
  /^\/artanis\/(?:accounts|traces)$/,
  /^\/autopilot\/?$/,
  /^\/autopilot\/legal$/,
  /^\/business\/?$/,
  /^\/business\/kpi\/[^/]+$/,
  /^\/changelog$/,
  /^\/clients-preview$/,
  /^\/code\/?$/,
  /^\/code\/download$/,
  /^\/components(?:\/[^/]+)?\/?$/,
  /^\/download$/,
  /^\/docs(?:\/.*)?$/,
  // #9243: the invite-only Forge and its descendant document routes. The
  // Worker imports this table as its document-route mirror.
  /^\/forge(?:\/.*)?$/,
  /^\/forum(?:\/(?:f|t|receipts)\/[^/]+)?\/?$/,
  /^\/gym$/,
  /^\/khala\/?$/,
  /^\/khala\/chat-sync$/,
  /^\/landing-en$/,
  /^\/login$/,
  /^\/(?:astro|download|install)\/?$/,
  /^\/mirrorcode$/,
  /^\/new$/,
  /^\/onboarding$/,
  /^\/portal\/?$/,
  /^\/preview\/(?:landing|sales-landing)$/,
  /^\/tanstack$/,
  /^\/(?:privacy|promises|pylons|qa|run|stage1|stats|tassadar|terms)$/,
  /^\/share\/[^/]+$/,
  /^\/splash$/,
  /^\/trace\/[^/]+$/,
  /^\/training\/runs(?:\/[^/]+)?\/?$/,
  /^\/workspaces\/[^/]+$/,
  /^\/pylon\/codex\/assignments\/[^/]+$/,
]

export const isKnownStartDocumentPath = (pathname: string): boolean =>
  knownDocumentPathPatterns.some(pattern => pattern.test(pathname))
