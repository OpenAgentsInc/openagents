// CL-54 mobile foundation: a single shared connection + polling layer for every
// screen. Until now the whole app lived in app/nodes.tsx, which owned discovery,
// polling, and the control verbs in local state — so the other drawer
// destinations (Sessions/Decisions/Spawn/Settings) were inert stubs with no way
// to reach the node. This provider lifts that data layer out so any screen can
// `useConnection()` to read the live node projection and dispatch control verbs,
// while each screen keeps its own UI-local state.

import { createContext, type ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react"

import {
  acceptEvent,
  createActionQueue,
  initialCursor,
  type ArtifactContentView,
  type SessionEvent,
  type StreamCursor,
} from "@openagentsinc/autopilot-control-protocol"
import {
  type AccountRow,
  type ApprovalRow,
  type AssignmentRow,
  type ConnectInfo,
  type ControlSessionEventRow,
  type ControlSessionRow,
  type DeployEnv,
  type DeployResult,
  type DeployStatus,
  type DeployTarget,
  type IntentRow,
  type BridgeSession,
  type SessionArtifact,
  type WalletStatus,
  cancelSession as cancelSessionCall,
  cancelSessionViaBridge,
  connectBridge,
  decodeConnectCode,
  deployToCloud as deployToCloudCall,
  deployToCloudViaBridge,
  fetchAccounts,
  fetchAccountsRaw,
  fetchApprovals,
  fetchAssignments,
  fetchCoordinatorPaused,
  fetchDeployStatus,
  fetchIntents,
  fetchSessionArtifact as fetchSessionArtifactCall,
  fetchSessionArtifactContent as fetchSessionArtifactContentCall,
  fetchSessionArtifactContentViaBridge,
  fetchSessionEvents as fetchSessionEventsCall,
  fetchSessionEventBatchViaBridge,
  fetchSessionEventsViaBridge,
  fetchSessionRowsViaBridge,
  fetchSessions,
  fetchWalletStatus,
  resolveApproval as resolveApprovalCall,
  resolveDecisionViaBridge,
  setCoordinatorPaused as setCoordinatorPausedCall,
  setCoordinatorPausedViaBridge,
  spawnSession as spawnSessionCall,
  spawnSessionViaBridge,
  steerTurn as steerTurnCall,
  steerTurnViaBridge,
  submitIntent as submitIntentCall,
  submitIntentViaBridge,
} from "../control/control-client"
import { parseNodesResponse, pickConnect } from "../control/discovery-client"

// Discovery broker (Cloud Run today; updates.openagents.com once DNS lands).
// Owner is single-tenant for now ("fine for now security-wise").
const BROKER = "https://oa-updates-ezxz4mgdsq-uc.a.run.app"
const OWNER = "chris"
// Backstop polling cadence. Used as the live-streaming fallback (and the only
// loop before a bridge credential exists). #5493 drives the UI off the bridge
// `session.subscribe` cursor stream when one is available; this 4s timer only
// reasserts the list when the stream is unavailable/dropped.
const POLL_MS = 4000
// Live-stream sweep cadence (#5493). While a bridge credential is healthy the
// Sessions list refreshes off detected event batches at this tighter interval
// instead of the 4s blind poll, so state transitions surface promptly.
const STREAM_MS = 1500
const WALLET_POLL_MS = 8000

// Sessions in these states are done — no further events arrive, so the stream
// sweep skips them rather than subscribing to a closed timeline.
const TERMINAL_STATES = new Set(["completed", "failed", "cancelled"])

export type ConnectionStatus = "discovering" | "manual" | "connecting" | "connected" | "error"

export type ConnectionValue = {
  // connection
  readonly status: ConnectionStatus
  readonly conn: ConnectInfo | null
  readonly nodeName: string | null
  readonly error: string | null
  // live node projection
  readonly sessions: ControlSessionRow[]
  readonly accounts: AccountRow[]
  readonly accountsRaw: unknown[]
  readonly wallet: WalletStatus | null
  readonly assignments: AssignmentRow[]
  readonly intents: IntentRow[]
  readonly approvals: ApprovalRow[]
  readonly coordPaused: boolean | null
  readonly deployStatus: DeployStatus | null
  // actions (conn is captured; no-ops when disconnected)
  connectManual(code: string): boolean
  refresh(): void
  submitAsk(draft: { title: string; body: string }): Promise<string>
  resolveApproval(input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }): Promise<void>
  setCoordinatorPaused(paused: boolean): Promise<boolean | null>
  cancelSession(sessionRef: string): Promise<void>
  spawnSession(draft: {
    adapter: "codex" | "claude_agent"
    objective: string
    verify?: string[]
    lane?: "auto" | "local" | "cloud-gcp" | "cloud-shc"
  }): Promise<string>
  steerTurn(input: { sessionRef: string; instruction: string; timeoutSeconds?: number }): Promise<string>
  deploy(input: { target: DeployTarget; ref: string; env?: DeployEnv }): Promise<DeployResult>
  fetchSessionEvents(sessionRef: string): Promise<ControlSessionEventRow[]>
  fetchSessionArtifact(sessionRef: string): Promise<SessionArtifact>
  // G3 (#5495): the full artifact/diff content (changed files, dev-check
  // transcript, deviations, verbatim text). Bridge read_artifact preferred,
  // dev-token session.artifact fallback. null when the session has no artifact.
  fetchSessionArtifactContent(sessionRef: string): Promise<ArtifactContentView | null>
  // #5493 live streaming. Subscribe to a session's `session.subscribe` cursor
  // stream over the capability-scoped bridge: `onBatch` fires whenever the node
  // replays one or more *new* (cursor-advancing, deduped) events. Returns a
  // disposer plus `streaming` — false when no bridge credential is available, so
  // the caller keeps its own poll as the graceful fallback. Idempotent to call;
  // each call owns its own cursor.
  subscribeSession(
    sessionRef: string,
    onBatch: (events: SessionEvent[]) => void,
  ): { streaming: boolean; dispose: () => void }
}

const ConnectionContext = createContext<ConnectionValue | null>(null)

export function ConnectionProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<ConnectionStatus>("discovering")
  const [conn, setConn] = useState<ConnectInfo | null>(null)
  const [nodeName, setNodeName] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [sessions, setSessions] = useState<ControlSessionRow[]>([])
  const [accounts, setAccounts] = useState<AccountRow[]>([])
  const [accountsRaw, setAccountsRaw] = useState<unknown[]>([])
  const [wallet, setWallet] = useState<WalletStatus | null>(null)
  const [assignments, setAssignments] = useState<AssignmentRow[]>([])
  const [intents, setIntents] = useState<IntentRow[]>([])
  const [approvals, setApprovals] = useState<ApprovalRow[]>([])
  const [coordPaused, setCoordPaused] = useState<boolean | null>(null)
  const [deployStatus, setDeployStatus] = useState<DeployStatus | null>(null)
  // #5493: flips true once the capability-scoped bridge credential is paired, so
  // the live `session.subscribe` stream can drive the Sessions list (and detail
  // screen) instead of the blind poll. Stays a render-triggering state (the
  // credential itself lives in the `bridge` ref) so streaming effects re-run.
  const [bridgeReady, setBridgeReady] = useState(false)

  const timer = useRef<ReturnType<typeof setInterval> | null>(null)
  // #5001: a scoped bridge credential for capability-gated reads. Established
  // additively after connect; the session event stream reads over it (no
  // long-lived dev token on the wire), falling back to the dev-token path.
  const bridge = useRef<BridgeSession | null>(null)
  // #5002: offline queue for write actions taken while disconnected. Drained +
  // replayed on reconnect; entries past TTL are dropped rather than replayed stale.
  const actionQueue = useRef(
    createActionQueue<{ approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }>({
      ttlMs: 600_000,
    }),
  )

  const poll = useCallback(async (c: ConnectInfo) => {
    try {
      // Prefer the bridge-native session.list over the capability-scoped
      // credential (#5001) so the session list polls with no long-lived dev
      // token on the wire; fall back to the dev-token /command path when no
      // bridge credential is established yet or the bridge read fails. Matches
      // the bridge-prefer pattern used by cancel/events/decision dispatch.
      const session = bridge.current
      if (session !== null) {
        try {
          setSessions(await fetchSessionRowsViaBridge(session))
          setStatus("connected")
          setError(null)
          return
        } catch {
          // fall through to dev-token path
        }
      }
      setSessions(await fetchSessions(c))
      setStatus("connected")
      setError(null)
    } catch (e) {
      setStatus("error")
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  // #5493: subscribe to one session's `session.subscribe` cursor stream over the
  // capability-scoped bridge. A tight loop fetches the next cursor-resumable
  // event batch; the shared `acceptEvent` cursor dedups + advances, and `onBatch`
  // fires only with genuinely-new events. When no bridge credential exists we
  // return `{ streaming: false }` so the caller falls back to its own poll. On a
  // network drop the loop keeps retrying at the same cadence; the caller's poll
  // backstop covers any gap. `dispose()` stops the loop. Declared before the
  // streaming effects that consume it.
  const subscribeSession = useCallback(
    (sessionRef: string, onBatch: (events: SessionEvent[]) => void) => {
      const bridgeSession = bridge.current
      if (bridgeSession === null) return { streaming: false, dispose: () => {} }

      let stopped = false
      let cursor: StreamCursor = initialCursor()
      let timer: ReturnType<typeof setTimeout> | null = null

      const tick = async () => {
        if (stopped) return
        try {
          const batch = await fetchSessionEventBatchViaBridge(bridgeSession, sessionRef, cursor.lastSequence)
          if (stopped) return
          const accepted: SessionEvent[] = []
          for (const event of [...batch].sort((l, r) => l.sequence - r.sequence)) {
            const result = acceptEvent(cursor, event)
            cursor = result.cursor
            if (result.accepted) accepted.push(event)
          }
          if (accepted.length > 0) onBatch(accepted)
        } catch {
          // Transient stream drop: the caller's poll backstop covers the gap;
          // keep retrying on the same cadence.
        } finally {
          if (!stopped) timer = setTimeout(() => void tick(), STREAM_MS)
        }
      }

      void tick()
      return {
        streaming: true,
        dispose: () => {
          stopped = true
          if (timer) clearTimeout(timer)
        },
      }
    },
    [],
  )

  // Auto-detect: on launch, ask the broker for this owner's nodes and connect to
  // the first reachable one (tailnet-first). Falls back to manual paste.
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const res = await fetch(`${BROKER}/${OWNER}/nodes`)
        const nodes = parseNodesResponse(await res.json())
        if (cancelled) return
        if (nodes.length > 0) {
          const info = pickConnect(nodes[0])
          setNodeName(nodes[0].name ?? nodes[0].id ?? "node")
          setConn(info)
          setStatus("connecting")
          void poll(info)
          return
        }
        setStatus("manual")
      } catch {
        if (!cancelled) setStatus("manual")
      }
    })()
    return () => {
      cancelled = true
    }
  }, [poll])

  // Session list poll — the graceful fallback / backstop. Before a bridge is
  // paired this is the only refresh; once #5493 streaming is live below it keeps
  // running at the slower cadence so the list still reasserts if the stream drops.
  useEffect(() => {
    if (conn === null) return
    timer.current = setInterval(() => void poll(conn), POLL_MS)
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [conn, poll])

  // #5493 live Sessions list: drive list refresh off the bridge
  // `session.subscribe` stream. While a bridge credential is healthy, subscribe
  // to every non-terminal session; whenever the node replays a new event
  // (state/progress transition) refresh the full list immediately, so the
  // Sessions screen advances within ~STREAM_MS instead of waiting on the 4s
  // poll. The poll effect above remains the fallback when no bridge exists or
  // the stream drops. We key the subscriptions on the set of running refs so a
  // newly-spawned (or newly-terminal) session re-subscribes without thrashing.
  const runningRefsKey = sessions
    .filter((s) => !TERMINAL_STATES.has(s.state))
    .map((s) => s.sessionRef)
    .sort()
    .join("|")
  useEffect(() => {
    if (conn === null || !bridgeReady) return
    const refs = runningRefsKey.length > 0 ? runningRefsKey.split("|") : []
    if (refs.length === 0) return
    let refreshing = false
    const refreshNow = () => {
      if (refreshing) return
      refreshing = true
      void poll(conn).finally(() => {
        refreshing = false
      })
    }
    const subs = refs.map((ref) => subscribeSession(ref, () => refreshNow()))
    // If the bridge could not actually stream (credential vanished mid-flight),
    // bail out and let the poll backstop carry it.
    if (!subs.some((s) => s.streaming)) {
      for (const s of subs) s.dispose()
      return
    }
    return () => {
      for (const s of subs) s.dispose()
    }
  }, [conn, bridgeReady, runningRefsKey, poll, subscribeSession])

  // #5001: pair onto the capability-scoped bridge once per connection. Reads
  // (the session event stream) prefer this credential over the dev token; if
  // the node doesn't expose bridge pairing, connectBridge returns null and the
  // dev-token path is used unchanged.
  useEffect(() => {
    if (conn === null) {
      bridge.current = null
      setBridgeReady(false)
      return
    }
    let cancelled = false
    // #5002 + #5494 (epic #5492 G1) + #5495 (G3): request the full capability set
    // so the scoped credential covers all six working steer-actions plus the
    // artifact read over the bridge — resolve decisions (answer_decision), cancel
    // (cancel), spawn (spawn_session), submit-intent (send_instruction),
    // pause/resume (pause_resume), deploy (deploy_cloud), and read retained
    // proof/failure artifacts for the diff viewer (read_artifact, still
    // read-only) — on the owner's own phone, with no long-lived dev token on the
    // wire.
    void connectBridge(conn, {
      capabilities: [
        "observe_public",
        "answer_decision",
        "cancel",
        "spawn_session",
        "send_instruction",
        "pause_resume",
        "deploy_cloud",
        "read_artifact",
      ],
    })
      .then((session) => {
        if (cancelled) return
        bridge.current = session
        // #5493: signal readiness so the live `session.subscribe` stream lights up.
        setBridgeReady(session !== null)
      })
      .catch(() => {})
    return () => {
      cancelled = true
      bridge.current = null
      setBridgeReady(false)
    }
  }, [conn])

  // Accounts (once per connection) + the periodic wallet/intents/approvals/
  // deploy/coordinator refresh, plus one notification-permission request.
  useEffect(() => {
    if (conn === null) return
    let cancelled = false
    void fetchAccounts(conn)
      .then((rows) => !cancelled && setAccounts(rows))
      .catch(() => {})
    void fetchAccountsRaw(conn)
      .then((rows) => !cancelled && setAccountsRaw(rows))
      .catch(() => {})
    void fetchAssignments(conn)
      .then((rows) => !cancelled && setAssignments(rows))
      .catch(() => {})
    const loadWallet = () => {
      void fetchWalletStatus(conn).then((w) => !cancelled && setWallet(w))
      void fetchIntents(conn).then((rows) => !cancelled && setIntents(rows))
      void fetchApprovals(conn).then((rows) => !cancelled && setApprovals(rows))
      void fetchDeployStatus(conn).then((s) => !cancelled && setDeployStatus(s))
      void fetchCoordinatorPaused(conn).then((p) => !cancelled && setCoordPaused(p))
    }
    loadWallet()
    const walletTimer = setInterval(loadWallet, WALLET_POLL_MS)
    return () => {
      cancelled = true
      clearInterval(walletTimer)
    }
  }, [conn])

  // NOTE: OS notifications are intentionally NOT fired here. The Nodes screen
  // (app/nodes.tsx) stays mounted as the initial route and owns the
  // notify-on-new-session-state effect; firing it here too would double-notify.

  const connectManual = useCallback(
    (code: string): boolean => {
      const info = decodeConnectCode(code)
      if (info === null) {
        setError("Invalid connect code")
        return false
      }
      setConn(info)
      setStatus("connecting")
      void poll(info)
      return true
    },
    [poll],
  )

  const refresh = useCallback(() => {
    if (conn !== null) void poll(conn)
  }, [conn, poll])

  const submitAsk = useCallback(
    async (draft: { title: string; body: string }) => {
      if (conn === null) throw new Error("not connected")
      // #5494: prefer the capability-scoped bridge (send_instruction); fall
      // back to the dev-token path if no bridge credential or the call fails.
      const session = bridge.current
      if (session !== null) {
        try {
          return await submitIntentViaBridge(session, draft)
        } catch {
          // fall through to dev-token path
        }
      }
      return submitIntentCall(conn, draft)
    },
    [conn],
  )

  // #5002: resolve a decision over the capability-scoped bridge when paired,
  // falling back to the dev-token path. Throws on a network failure so the
  // caller can queue for replay.
  const dispatchResolve = useCallback(
    async (input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }) => {
      const c = conn
      if (c === null) return
      const session = bridge.current
      if (session !== null) {
        await resolveDecisionViaBridge(session, input)
        return
      }
      await resolveApprovalCall(c, input)
    },
    [conn],
  )

  const resolveApproval = useCallback(
    async (input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }) => {
      if (conn === null) return
      // Optimistically drop the resolved approval from the shared list.
      setApprovals((prev) => prev.filter((a) => a.approvalRef !== input.approvalRef))
      try {
        await dispatchResolve(input)
      } catch {
        // Offline/unreachable: queue for replay on reconnect (TTL-bounded).
        actionQueue.current.enqueue({ id: input.approvalRef, action: input, nowMs: Date.now() })
      }
    },
    [conn, dispatchResolve],
  )

  // #5002: on reconnect, replay queued write actions (oldest-first); expired
  // entries are dropped rather than replayed stale.
  useEffect(() => {
    if (status !== "connected" || conn === null) return
    const { ready } = actionQueue.current.drain(Date.now())
    for (const entry of ready) {
      void dispatchResolve(entry.action).catch(() => {
        actionQueue.current.enqueue({ id: entry.id, action: entry.action, nowMs: Date.now() })
      })
    }
  }, [status, conn, dispatchResolve])

  const setCoordinatorPaused = useCallback(
    async (paused: boolean) => {
      if (conn === null) return null
      setCoordPaused(paused)
      // #5494: prefer the capability-scoped bridge (pause_resume); fall back to
      // the dev-token path if no bridge credential or the call fails.
      const session = bridge.current
      let next = paused
      if (session !== null) {
        try {
          next = await setCoordinatorPausedViaBridge(session, paused)
          setCoordPaused(next)
          return next
        } catch {
          // fall through to dev-token path
        }
      }
      next = await setCoordinatorPausedCall(conn, paused).catch(() => paused)
      setCoordPaused(next)
      return next
    },
    [conn],
  )

  const cancelSession = useCallback(
    async (sessionRef: string) => {
      if (conn === null) return
      // Prefer the capability-scoped bridge (cancel capability); fall back to
      // the dev-token path if no bridge credential or the bridge call fails.
      const session = bridge.current
      if (session !== null) {
        try {
          await cancelSessionViaBridge(session, sessionRef)
          await poll(conn)
          return
        } catch {
          // fall through to dev-token path
        }
      }
      await cancelSessionCall(conn, sessionRef)
      await poll(conn)
    },
    [conn, poll],
  )

  const spawnSession = useCallback(
    async (draft: {
      adapter: "codex" | "claude_agent"
      objective: string
      verify?: string[]
      lane?: "auto" | "local" | "cloud-gcp" | "cloud-shc"
    }) => {
      if (conn === null) throw new Error("not connected")
      // #5494: prefer the capability-scoped bridge (spawn_session); fall back to
      // the dev-token path if no bridge credential or the call fails.
      const session = bridge.current
      let ref: string | null = null
      if (session !== null) {
        try {
          ref = await spawnSessionViaBridge(session, draft)
        } catch {
          ref = null
        }
      }
      if (ref === null) ref = await spawnSessionCall(conn, draft)
      await poll(conn)
      return ref
    },
    [conn, poll],
  )

  const steerTurn = useCallback(
    async (input: { sessionRef: string; instruction: string; timeoutSeconds?: number }) => {
      if (conn === null) throw new Error("not connected")
      const session = bridge.current
      let ref: string | null = null
      if (session !== null) {
        try {
          ref = (await steerTurnViaBridge(session, input)).sessionRef
        } catch {
          ref = null
        }
      }
      if (ref === null || ref.length === 0) {
        ref = (await steerTurnCall(conn, input)).sessionRef
      }
      await poll(conn)
      return ref
    },
    [conn, poll],
  )

  const deploy = useCallback(
    async (input: { target: DeployTarget; ref: string; env?: DeployEnv }) => {
      if (conn === null) return { accepted: false, reason: "not connected", errors: [] }
      // #5494: prefer the capability-scoped bridge (deploy_cloud); fall back to
      // the dev-token path if no bridge credential or the call fails.
      const session = bridge.current
      let r: DeployResult | null = null
      if (session !== null) {
        try {
          r = await deployToCloudViaBridge(session, input)
        } catch {
          r = null
        }
      }
      if (r === null) r = await deployToCloudCall(conn, input)
      void fetchDeployStatus(conn).then(setDeployStatus)
      return r
    },
    [conn],
  )

  const fetchSessionEvents = useCallback(
    async (sessionRef: string) => {
      if (conn === null) return []
      // Prefer the capability-scoped bridge (#5000 session.history); fall back
      // to the dev-token path if no bridge credential or the read fails.
      const session = bridge.current
      if (session !== null) {
        try {
          return await fetchSessionEventsViaBridge(session, sessionRef)
        } catch {
          // fall through to dev-token path
        }
      }
      return fetchSessionEventsCall(conn, sessionRef)
    },
    [conn],
  )

  const fetchSessionArtifact = useCallback(
    async (sessionRef: string): Promise<SessionArtifact> => {
      if (conn === null) return { kind: "none", outcome: null, editedFileCount: null, commandCount: null, totalTokens: null }
      return fetchSessionArtifactCall(conn, sessionRef)
    },
    [conn],
  )

  const fetchSessionArtifactContent = useCallback(
    async (sessionRef: string): Promise<ArtifactContentView | null> => {
      if (conn === null) return null
      // Prefer the capability-scoped bridge (#5495 read_artifact); fall back to
      // the dev-token session.artifact path if no bridge credential or it fails.
      const session = bridge.current
      if (session !== null) {
        try {
          return await fetchSessionArtifactContentViaBridge(session, sessionRef)
        } catch {
          // fall through to dev-token path
        }
      }
      return fetchSessionArtifactContentCall(conn, sessionRef)
    },
    [conn],
  )

  const value: ConnectionValue = {
    status,
    conn,
    nodeName,
    error,
    sessions,
    accounts,
    accountsRaw,
    wallet,
    assignments,
    intents,
    approvals,
    coordPaused,
    deployStatus,
    connectManual,
    refresh,
    submitAsk,
    resolveApproval,
    setCoordinatorPaused,
    cancelSession,
    spawnSession,
    steerTurn,
    deploy,
    fetchSessionEvents,
    fetchSessionArtifact,
    fetchSessionArtifactContent,
    subscribeSession,
  }

  return <ConnectionContext.Provider value={value}>{children}</ConnectionContext.Provider>
}

export function useConnection(): ConnectionValue {
  const ctx = useContext(ConnectionContext)
  if (ctx === null) throw new Error("useConnection must be used within a ConnectionProvider")
  return ctx
}
