// CL-54 mobile foundation: a single shared connection + polling layer for every
// screen. Until now the whole app lived in app/nodes.tsx, which owned discovery,
// polling, and the control verbs in local state — so the other drawer
// destinations (Sessions/Decisions/Spawn/Settings) were inert stubs with no way
// to reach the node. This provider lifts that data layer out so any screen can
// `useConnection()` to read the live node projection and dispatch control verbs,
// while each screen keeps its own UI-local state.

import { createContext, type ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react"

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
  type SessionArtifact,
  type WalletStatus,
  cancelSession as cancelSessionCall,
  decodeConnectCode,
  deployToCloud as deployToCloudCall,
  fetchAccounts,
  fetchAccountsRaw,
  fetchApprovals,
  fetchAssignments,
  fetchCoordinatorPaused,
  fetchDeployStatus,
  fetchIntents,
  fetchSessionArtifact as fetchSessionArtifactCall,
  fetchSessionEvents as fetchSessionEventsCall,
  fetchSessions,
  fetchWalletStatus,
  resolveApproval as resolveApprovalCall,
  setCoordinatorPaused as setCoordinatorPausedCall,
  spawnSession as spawnSessionCall,
  submitIntent as submitIntentCall,
} from "../control/control-client"
import { parseNodesResponse, pickConnect } from "../control/discovery-client"

// Discovery broker (Cloud Run today; updates.openagents.com once DNS lands).
// Owner is single-tenant for now ("fine for now security-wise").
const BROKER = "https://oa-updates-ezxz4mgdsq-uc.a.run.app"
const OWNER = "chris"
const POLL_MS = 4000
const WALLET_POLL_MS = 8000

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
  spawnSession(draft: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[] }): Promise<string>
  deploy(input: { target: DeployTarget; ref: string; env?: DeployEnv }): Promise<DeployResult>
  fetchSessionEvents(sessionRef: string): Promise<ControlSessionEventRow[]>
  fetchSessionArtifact(sessionRef: string): Promise<SessionArtifact>
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

  const timer = useRef<ReturnType<typeof setInterval> | null>(null)

  const poll = useCallback(async (c: ConnectInfo) => {
    try {
      setSessions(await fetchSessions(c))
      setStatus("connected")
      setError(null)
    } catch (e) {
      setStatus("error")
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

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

  // Session list poll.
  useEffect(() => {
    if (conn === null) return
    timer.current = setInterval(() => void poll(conn), POLL_MS)
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [conn, poll])

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
      return submitIntentCall(conn, draft)
    },
    [conn],
  )

  const resolveApproval = useCallback(
    async (input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string }) => {
      if (conn === null) return
      // Optimistically drop the resolved approval from the shared list.
      setApprovals((prev) => prev.filter((a) => a.approvalRef !== input.approvalRef))
      await resolveApprovalCall(conn, input).catch(() => {})
    },
    [conn],
  )

  const setCoordinatorPaused = useCallback(
    async (paused: boolean) => {
      if (conn === null) return null
      setCoordPaused(paused)
      const next = await setCoordinatorPausedCall(conn, paused).catch(() => paused)
      setCoordPaused(next)
      return next
    },
    [conn],
  )

  const cancelSession = useCallback(
    async (sessionRef: string) => {
      if (conn === null) return
      await cancelSessionCall(conn, sessionRef)
      await poll(conn)
    },
    [conn, poll],
  )

  const spawnSession = useCallback(
    async (draft: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[] }) => {
      if (conn === null) throw new Error("not connected")
      const ref = await spawnSessionCall(conn, draft)
      await poll(conn)
      return ref
    },
    [conn, poll],
  )

  const deploy = useCallback(
    async (input: { target: DeployTarget; ref: string; env?: DeployEnv }) => {
      if (conn === null) return { accepted: false, reason: "not connected", errors: [] }
      const r = await deployToCloudCall(conn, input)
      void fetchDeployStatus(conn).then(setDeployStatus)
      return r
    },
    [conn],
  )

  const fetchSessionEvents = useCallback(
    async (sessionRef: string) => {
      if (conn === null) return []
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
    deploy,
    fetchSessionEvents,
    fetchSessionArtifact,
  }

  return <ConnectionContext.Provider value={value}>{children}</ConnectionContext.Provider>
}

export function useConnection(): ConnectionValue {
  const ctx = useContext(ConnectionContext)
  if (ctx === null) throw new Error("useConnection must be used within a ConnectionProvider")
  return ctx
}
