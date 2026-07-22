/** Dependency-free document ownership shared by Start and the Worker. */
export const knownDocumentPathPatterns: ReadonlyArray<RegExp> = [
  /^\/app\/?$/,
  /^\/X\/$/,
  /^\/activity$/,
  // #9188: admin-only operator dashboard. Auth is enforced server-side by the
  // `/api/admin/operator/overview` endpoint; a non-admin gets the refusal
  // view.
  /^\/admin\/operator$/,
  /^\/(?:adjutant|artanis)\/?$/,
  // /aisdk + /aisdk/docs: public OpenAgents AI SDK page and docs, added at
  // owner direction 2026-07-21 (SDK extracted to OpenAgentsInc/ai and
  // published as the @openagentsinc rc train on npm).
  /^\/aisdk\/?$/,
  /^\/aisdk\/docs(?:\/[^/]+)?\/?$/,
  /^\/agents\/[^/]+$/,
  /^\/artanis\/(?:accounts|traces)$/,
  /^\/autopilot\/?$/,
  /^\/autopilot\/legal$/,
  /^\/blog(?:\/[^/]+)?\/?$/,
  /^\/business\/?$/,
  /^\/business\/kpi\/[^/]+$/,
  /^\/changelog$/,
  /^\/clients-preview$/,
  /^\/code\/?$/,
  /^\/code\/download$/,
  /^\/components(?:\/[^/]+)?\/?$/,
  /^\/download$/,
  /^\/docs(?:\/.*)?$/,
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
