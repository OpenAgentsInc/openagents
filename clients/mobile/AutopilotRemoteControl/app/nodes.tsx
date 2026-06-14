import { type ReactNode, useCallback, useEffect, useRef, useState } from "react"
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"
import { Drawer } from "react-native-drawer-layout"
import { useNavigation } from "@react-navigation/native"

import {
  type ConnectInfo,
  type ControlSessionEventRow,
  type ControlSessionRow,
  type AccountRow,
  type SessionArtifact,
  type WalletStatus,
  type ApprovalRow,
  type AssignmentRow,
  type IntentRow,
  type DeployStatus,
  cancelSession,
  decodeConnectCode,
  deployToCloud,
  fetchAccounts,
  fetchAccountsRaw,
  fetchApprovals,
  fetchAssignments,
  fetchCoordinatorPaused,
  fetchDeployStatus,
  fetchIntents,
  fetchWalletStatus,
  resolveApproval,
  setCoordinatorPaused,
  fetchSessionArtifact,
  fetchSessionEvents,
  fetchSessions,
  submitIntent,
} from "../src/control/control-client"
import { parseNodesResponse, pickConnect } from "../src/control/discovery-client"
import { ensureNotificationPermission, notifyNewSessionStates } from "../src/notifications"
import { fixedRowLabelHeight } from "../src/ui/row-metrics"
import { DrawerIconButton } from "../src/ui/DrawerIconButton"
import {
  CANONICAL_DARK,
  capacityBar,
  projectAccountRegistryDetail,
  projectFailover,
  renderCloudCard,
  validateIntentDraft,
} from "@openagentsinc/autopilot-control-protocol"

// Map an intent status to a short round-trip ship line for the originating
// client (CL-40). Terminal = shipped/failed.
function shipStatusLine(status: string): { text: string; terminal: boolean; tone: string } {
  switch (status) {
    case "received": return { text: "received", terminal: false, tone: CANONICAL_DARK.textSecondary }
    case "planning": return { text: "planning…", terminal: false, tone: CANONICAL_DARK.info }
    case "fanning_out": return { text: "agents working…", terminal: false, tone: CANONICAL_DARK.info }
    case "shipping": return { text: "shipping…", terminal: false, tone: CANONICAL_DARK.warning }
    case "shipped": return { text: "✓ shipped", terminal: true, tone: CANONICAL_DARK.success }
    case "failed": return { text: "✗ failed", terminal: true, tone: CANONICAL_DARK.danger }
    default: return { text: status, terminal: false, tone: CANONICAL_DARK.textSecondary }
  }
}

// Discovery broker (Cloud Run today; updates.openagents.com once DNS lands).
// Owner is single-tenant for now ("fine for now security-wise").
const BROKER = "https://oa-updates-ezxz4mgdsq-uc.a.run.app"
const OWNER = "chris"
const POLL_MS = 4000

// CL-31: the shared canonical dark palette, sourced from the protocol package
// (RN-safe) so mobile, desktop, and web stay in theming parity from one source.
const C = CANONICAL_DARK
const ROW_LABEL_FONT_SIZE = 13
const ROW_LABEL_LINE_HEIGHT = 18

const stateTone = (state: string): string =>
  state === "completed"
    ? C.success
    : state === "running" || state === "started"
      ? C.info
      : state === "queued"
        ? C.warning
        : state === "failed" || state === "cancelled"
          ? C.danger
          : C.outline

type Status = "discovering" | "manual" | "connecting" | "connected" | "error"

export default function NodesScreen() {
  const [code, setCode] = useState("")
  const [conn, setConn] = useState<ConnectInfo | null>(null)
  const [sessions, setSessions] = useState<ControlSessionRow[]>([])
  const [status, setStatus] = useState<Status>("discovering")
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [events, setEvents] = useState<ControlSessionEventRow[]>([])
  const [expanded, setExpanded] = useState<number | null>(null)
  const [askTitle, setAskTitle] = useState("")
  const [askBody, setAskBody] = useState("")
  const [askStatus, setAskStatus] = useState<string | null>(null)
  const [nodeName, setNodeName] = useState<string | null>(null)
  const [accounts, setAccounts] = useState<AccountRow[]>([])
  const [accountsRaw, setAccountsRaw] = useState<unknown[]>([])
  const [accountsExpanded, setAccountsExpanded] = useState(false)
  const [wallet, setWallet] = useState<WalletStatus | null>(null)
  const [assignments, setAssignments] = useState<AssignmentRow[]>([])
  const [intents, setIntents] = useState<IntentRow[]>([])
  const [approvals, setApprovals] = useState<ApprovalRow[]>([])
  const [coordPaused, setCoordPaused] = useState<boolean | null>(null)
  // CL-26 "Deploy to Cloud": last-deploy status + a transient line after a tap.
  const [deployStatus, setDeployStatus] = useState<DeployStatus | null>(null)
  const [deployLine, setDeployLine] = useState<string | null>(null)
  const [drawerOpen, setDrawerOpen] = useState(false)
  const navigation = useNavigation<{ navigate: (route: string) => void }>()
  // CL-30 notifications: permission + the set of session states already notified.
  const notifPermitted = useRef(false)
  const seenNotifRefs = useRef<string[]>([])
  const [artifact, setArtifact] = useState<SessionArtifact | null>(null)
  const timer = useRef<ReturnType<typeof setInterval> | null>(null)
  const eventsTimer = useRef<ReturnType<typeof setInterval> | null>(null)

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

  useEffect(() => {
    if (conn === null) return
    timer.current = setInterval(() => void poll(conn), POLL_MS)
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [conn, poll])

  // Live session-detail timeline: poll the selected session's recent events.
  const pollEvents = useCallback(
    async (c: ConnectInfo, sessionRef: string) => {
      try {
        setEvents(await fetchSessionEvents(c, sessionRef))
      } catch {
        // transient; keep the last good timeline
      }
    },
    [],
  )
  // Accounts change rarely — fetch once per connection (CL-18).
  useEffect(() => {
    if (conn === null) return
    let cancelled = false
    // CL-30: request OS notification permission once per connection.
    void ensureNotificationPermission()
      .then((p) => {
        notifPermitted.current = p.state === "enabled"
      })
      .catch(() => {})
    void fetchAccounts(conn)
      .then((rows) => {
        if (!cancelled) setAccounts(rows)
      })
      .catch(() => {})
    void fetchAccountsRaw(conn)
      .then((rows) => {
        if (!cancelled) setAccountsRaw(rows)
      })
      .catch(() => {})
    void fetchAssignments(conn)
      .then((rows) => {
        if (!cancelled) setAssignments(rows)
      })
      .catch(() => {})
    // Live MDK wallet balance (CL-23) + ship-status round-trip (CL-40),
    // refreshed periodically.
    const loadWallet = () => {
      void fetchWalletStatus(conn).then((w) => {
        if (!cancelled) setWallet(w)
      })
      void fetchIntents(conn).then((rows) => {
        if (!cancelled) setIntents(rows)
      })
      void fetchApprovals(conn).then((rows) => {
        if (!cancelled) setApprovals(rows)
      })
      void fetchDeployStatus(conn).then((s) => {
        if (!cancelled) setDeployStatus(s)
      })
      void fetchCoordinatorPaused(conn).then((p) => {
        if (!cancelled) setCoordPaused(p)
      })
    }
    loadWallet()
    const walletTimer = setInterval(loadWallet, 8000)
    return () => {
      cancelled = true
      clearInterval(walletTimer)
    }
  }, [conn])

  // CL-30: fire OS notifications when sessions newly enter a notify-worthy
  // state (needs_decision / failed / completed). Derive via the shared core;
  // seen set persists across polls so each transition notifies once.
  useEffect(() => {
    if (sessions.length === 0) return
    let cancelled = false
    void notifyNewSessionStates(
      sessions.map((s) => ({ sessionRef: s.sessionRef, state: s.state, latestActivity: s.latestActivity })),
      seenNotifRefs.current,
      notifPermitted.current,
    ).then((next) => {
      if (!cancelled) seenNotifRefs.current = next
    })
    return () => {
      cancelled = true
    }
  }, [sessions])

  useEffect(() => {
    if (conn === null || selected === null) return
    setEvents([])
    setArtifact(null)
    void pollEvents(conn, selected)
    void fetchSessionArtifact(conn, selected)
      .then((a) => setArtifact(a.kind === "none" ? null : a))
      .catch(() => {})
    eventsTimer.current = setInterval(() => void pollEvents(conn, selected), POLL_MS)
    return () => {
      if (eventsTimer.current) clearInterval(eventsTimer.current)
    }
  }, [conn, selected, pollEvents])

  const submitAsk = useCallback(() => {
    if (conn === null) return
    const v = validateIntentDraft({ title: askTitle, body: askBody })
    if (!v.ok) {
      setAskStatus(`error: ${v.errors[0] ?? "invalid"}`)
      return
    }
    setAskStatus("sending…")
    void submitIntent(conn, { title: v.title, body: v.body })
      .then((s) => {
        setAskStatus(`sent · ${s}`)
        setAskTitle("")
        setAskBody("")
      })
      .catch((e) => setAskStatus(`error: ${e instanceof Error ? e.message : String(e)}`))
  }, [conn, askTitle, askBody])

  // CL-26: trigger a deploy of the node's own Cloud Run service through OUR
  // pipeline. Sensible default target/ref (cloudrun · main · production). The
  // node fail-safe-gates execution behind OA_DEPLOY_ENABLE=1, so a tap with the
  // gate unset comes back {accepted:false, reason:"deploy_disabled"} and the UI
  // shows it without anything deploying.
  const triggerDeploy = useCallback(() => {
    if (conn === null) return
    setDeployLine("deploying…")
    void deployToCloud(conn, { target: "cloudrun", ref: "main", env: "production" })
      .then((r) => {
        setDeployLine(
          r.accepted
            ? "queued · cloudrun · main"
            : r.reason === "deploy_disabled"
              ? "disabled (set OA_DEPLOY_ENABLE=1 on the node)"
              : `not accepted: ${r.errors[0] ?? r.reason}`,
        )
        void fetchDeployStatus(conn).then(setDeployStatus)
      })
      .catch((e) => setDeployLine(`error: ${e instanceof Error ? e.message : String(e)}`))
  }, [conn])

  const connectManual = useCallback(() => {
    const info = decodeConnectCode(code)
    if (info === null) {
      setError("Invalid connect code")
      return
    }
    setConn(info)
    setStatus("connecting")
    void poll(info)
  }, [code, poll])

  return (
    <Drawer
      open={drawerOpen}
      onOpen={() => setDrawerOpen(true)}
      onClose={() => setDrawerOpen(false)}
      drawerType="front"
      drawerStyle={styles.drawer}
      renderDrawerContent={() => (
        <DrawerNav
          nodeName={nodeName}
          onNavigate={(route) => {
            setDrawerOpen(false)
            navigation.navigate(route)
          }}
        />
      )}
    >
      <View style={styles.container}>
        <View style={styles.headerBar}>
          <DrawerIconButton onPress={() => setDrawerOpen((o) => !o)} />
          {conn !== null && coordPaused !== null ? (
            <Pressable
              style={[styles.coordToggle, coordPaused ? styles.coordPaused : null]}
              onPress={() => {
                if (conn === null) return
                const next = !coordPaused
                setCoordPaused(next)
                void setCoordinatorPaused(conn, next)
                  .then((p) => setCoordPaused(p))
                  .catch(() => {})
              }}
            >
              <Text style={styles.coordToggleText}>{coordPaused ? "▶ Resume" : "⏸ Pause"}</Text>
            </Pressable>
          ) : null}
        </View>
        <ScrollView contentContainerStyle={styles.content}>
          <Text style={styles.h1}>Autopilot</Text>
          <Text style={styles.subtitle}>Nodes</Text>

        {selected !== null ? (
          <>
            <Pressable style={styles.back} onPress={() => setSelected(null)}>
              <Text style={styles.backText}>‹ sessions</Text>
            </Pressable>
            <Text style={styles.detailRef}>{selected}</Text>
            {(() => {
              const s = sessions.find((x) => x.sessionRef === selected)
              if (!s) return null
              const verify =
                s.state === "completed"
                  ? `✓ verify passed${s.artifactRef ? ` · artifact ${s.artifactRef.slice(-12)}` : ""}`
                  : s.state === "failed"
                    ? `✗ verify failed${s.errorClass ? ` · ${s.errorClass}` : ""}`
                    : s.state === "cancelled"
                      ? "cancelled"
                      : `${s.state}…`
              const tone =
                s.state === "completed" ? C.success : s.state === "failed" ? C.danger : C.textSecondary
              return <Text style={[styles.verifyLine, { color: tone }]}>{verify}</Text>
            })()}
            {artifact ? (
              <Text style={styles.artifactLine}>
                artifact: {artifact.outcome ?? artifact.kind}
                {artifact.editedFileCount !== null ? ` · ${artifact.editedFileCount} files` : ""}
                {artifact.commandCount !== null ? ` · ${artifact.commandCount} cmds` : ""}
                {artifact.totalTokens !== null ? ` · ${artifact.totalTokens} tok` : ""}
              </Text>
            ) : null}
            {(() => {
              const s = sessions.find((x) => x.sessionRef === selected)
              const cancellable = s && (s.state === "running" || s.state === "queued" || s.state === "started")
              if (!cancellable || conn === null) return null
              return (
                <Pressable
                  style={styles.cancelBtn}
                  onPress={() => {
                    void cancelSession(conn, selected).then(() => poll(conn))
                  }}
                >
                  <Text style={styles.cancelText}>Cancel session</Text>
                </Pressable>
              )
            })()}
            {events.length === 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No events yet.</Text>
              </View>
            ) : (
              events.map((e) => {
                const isOpen = expanded === e.eventIndex
                const expandable = e.full.length > 0 || e.detail.length > 30
                return (
                  <Pressable
                    key={e.eventIndex}
                    style={styles.eventRow}
                    onPress={() => expandable && setExpanded(isOpen ? null : e.eventIndex)}
                  >
                    <View style={[styles.dot, { backgroundColor: stateTone(e.state) }]} />
                    <View style={styles.rowText}>
                      <Text style={[styles.rowLabel, isOpen ? styles.expandedRowLabel : null]} numberOfLines={isOpen ? undefined : 2}>
                        {isOpen ? e.full || e.detail || e.phase : e.detail || e.phase}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {e.detail ? `${e.phase} · ` : ""}#{e.eventIndex} · {e.observedAt.slice(11, 19) || e.state}
                        {expandable ? (isOpen ? " · tap to collapse" : " · tap to expand") : ""}
                      </Text>
                    </View>
                  </Pressable>
                )
              })
            )}
          </>
        ) : status === "discovering" ? (
          <View style={styles.statusRow}>
            <ActivityIndicator color={C.info} />
            <Text style={styles.statusText}>finding your node…</Text>
          </View>
        ) : conn === null ? (
          <View style={styles.card}>
            <Text style={styles.cardTitle}>Connect to a Pylon node</Text>
            <Text style={styles.cardBody}>
              No node found automatically. Paste a connect code (tailnet/LAN
              address + token) to connect manually.
            </Text>
            <TextInput
              style={styles.input}
              placeholder="connect code"
              placeholderTextColor={C.textSecondary}
              autoCapitalize="none"
              autoCorrect={false}
              value={code}
              onChangeText={setCode}
            />
            <Pressable style={styles.button} onPress={connectManual}>
              <Text style={styles.buttonText}>Connect</Text>
            </Pressable>
            {error ? <Text style={styles.error}>{error}</Text> : null}
          </View>
        ) : (
          <>
            <View style={styles.statusRow}>
              <View style={[styles.dot, { backgroundColor: status === "connected" ? C.success : status === "error" ? C.danger : C.warning }]} />
              <Text style={styles.statusText}>
                {nodeName ? `${nodeName} · ` : ""}
                {status === "connected"
                  ? `online · ${sessions.length} ${sessions.length === 1 ? "session" : "sessions"}`
                  : status === "error"
                    ? `error: ${error ?? "unknown"}`
                    : "connecting…"}
              </Text>
            </View>
            {status === "connected" && sessions.length > 0 ? (
              <Text style={styles.breakdown}>
                {(() => {
                  const by: Record<string, number> = {}
                  for (const s of sessions) by[s.state] = (by[s.state] ?? 0) + 1
                  return ["running", "queued", "completed", "failed", "cancelled"]
                    .filter((k) => by[k])
                    .map((k) => `${by[k]} ${k}`)
                    .join(" · ")
                })()}
              </Text>
            ) : null}
            <View style={styles.card}>
              <Text style={styles.cardTitle}>Ask Autopilot</Text>
              <TextInput
                style={styles.input}
                placeholder="title — what do you want done?"
                placeholderTextColor={C.textSecondary}
                value={askTitle}
                onChangeText={setAskTitle}
              />
              <TextInput
                style={[styles.input, styles.inputMultiline]}
                placeholder="details (optional)"
                placeholderTextColor={C.textSecondary}
                value={askBody}
                onChangeText={setAskBody}
                multiline
              />
              <Pressable style={styles.button} onPress={submitAsk}>
                <Text style={styles.buttonText}>Send to node</Text>
              </Pressable>
              {askStatus ? <Text style={styles.askStatus}>{askStatus}</Text> : null}
            </View>
            <View style={styles.card}>
              <Text style={styles.cardTitle}>Deploy to Cloud</Text>
              <Text style={styles.cardBody}>
                Deploy this node's Cloud Run service (cloudrun · main · production)
                through our pipeline. Disabled unless the node has OA_DEPLOY_ENABLE=1.
              </Text>
              <Pressable style={styles.button} onPress={triggerDeploy}>
                <Text style={styles.buttonText}>Deploy to Cloud</Text>
              </Pressable>
              <Text style={styles.askStatus}>
                {deployLine ??
                  (deployStatus ? `${deployStatus.state} · ${deployStatus.message}` : "no deploy yet")}
              </Text>
            </View>
            {approvals.length > 0 ? (
              <View style={[styles.card, styles.approvalCard]}>
                <Text style={styles.cardTitle}>Needs you ({approvals.length})</Text>
                {approvals.map((a) => {
                  const resolve = (decision: "approve" | "deny") => {
                    if (conn === null) return
                    setApprovals((prev) => prev.filter((x) => x.approvalRef !== a.approvalRef))
                    void resolveApproval(conn, { approvalRef: a.approvalRef, decision }).catch(() => {})
                  }
                  return (
                    <View key={a.approvalRef} style={styles.approvalRow}>
                      <Text style={styles.acctText} numberOfLines={2}>
                        {a.prompt || a.kind}
                      </Text>
                      <View style={styles.approvalButtons}>
                        <Pressable style={[styles.approvalBtn, styles.approveBtn]} onPress={() => resolve("approve")}>
                          <Text style={styles.approvalBtnText}>Approve</Text>
                        </Pressable>
                        <Pressable style={[styles.approvalBtn, styles.denyBtn]} onPress={() => resolve("deny")}>
                          <Text style={styles.approvalBtnText}>Deny</Text>
                        </Pressable>
                      </View>
                    </View>
                  )
                })}
              </View>
            ) : null}
            {intents.length > 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardTitle}>Your asks</Text>
                {intents.slice(0, 5).map((it) => {
                  const s = shipStatusLine(it.status)
                  return (
                    <View key={it.intentId} style={styles.acctRow}>
                      <View style={[styles.dot, { backgroundColor: s.tone }]} />
                      <Text style={styles.acctText} numberOfLines={1}>
                        {it.title || it.intentId.slice(-8)} · {s.text}
                      </Text>
                    </View>
                  )
                })}
              </View>
            ) : null}
            {wallet ? (
              <View style={styles.card}>
                <Text style={styles.cardTitle}>Balance</Text>
                <Text style={styles.balanceValue}>
                  {wallet.balanceSats !== null ? `${wallet.balanceSats.toLocaleString()} sats` : "—"}
                </Text>
                <Text style={styles.acctSummary}>
                  {wallet.daemonOnline ? "wallet online" : "wallet offline"} · {wallet.readiness}
                  {wallet.receiveReady ? " · receive ✓" : ""}
                </Text>
              </View>
            ) : null}
            {accounts.length > 0 ? (
              <View style={styles.card}>
                {(() => {
                  const detail = projectAccountRegistryDetail(accountsRaw)
                  const f = projectFailover(accounts.map((a) => ({ provider: a.provider, ready: a.ready })))
                  return (
                    <>
                      <Pressable
                        style={styles.acctHeader}
                        onPress={() => setAccountsExpanded((v) => !v)}
                      >
                        <Text style={styles.cardTitle}>
                          Accounts ({detail.total})
                        </Text>
                        <Text style={styles.chevron}>{accountsExpanded ? "⌄" : "›"}</Text>
                      </Pressable>
                      <Text style={styles.acctSummary}>
                        {detail.readyCount} ready · {detail.exhaustedCount} exhausted
                        {f.failedOver ? " · ⚠ failed over" : ""}
                      </Text>
                      {!accountsExpanded
                        ? accounts.map((a, i) => (
                            <View key={`${a.provider}-${i}`} style={styles.acctRow}>
                              <View style={[styles.dot, { backgroundColor: a.ready ? C.success : C.warning }]} />
                              <Text style={styles.acctText}>
                                {a.provider} · {a.homeState} · {a.ready ? "ready" : "blocked"}
                              </Text>
                            </View>
                          ))
                        : detail.accounts.map((a, i) => {
                            const bar = capacityBar({
                              usedPct: a.capacity?.usedPct ?? null,
                              exhausted: a.exhausted,
                            })
                            const tone = a.exhausted ? C.danger : a.ready ? C.success : C.warning
                            return (
                              <View key={`${a.identityLabel}-${i}`} style={styles.acctDetailRow}>
                                <View style={styles.acctDetailTop}>
                                  <View style={[styles.dot, { backgroundColor: tone }]} />
                                  <Text style={styles.acctText}>{a.identityLabel}</Text>
                                </View>
                                <Text style={styles.acctMeta}>
                                  {a.provider} · {a.homeState} ·{" "}
                                  {a.exhausted ? "exhausted" : a.ready ? "ready" : "blocked"} · {bar.label}
                                </Text>
                                {a.blockerRefs.length > 0 ? (
                                  <Text style={styles.acctBlockers} numberOfLines={2}>
                                    {a.blockerRefs.join(", ")}
                                  </Text>
                                ) : null}
                              </View>
                            )
                          })}
                    </>
                  )
                })()}
              </View>
            ) : null}
            {status === "connected"
              ? (() => {
                  // CL-27 (rescoped): a contributor node exposes no cloud-metering
                  // feed, so render the honest "unavailable" cloud card. Provider
                  // failover is shown in the Accounts card above.
                  const cloud = renderCloudCard(null)
                  return cloud.visible ? (
                    <View style={styles.card}>
                      <Text style={styles.cardTitle}>{cloud.title}</Text>
                      <Text style={styles.acctSummary}>{cloud.body}</Text>
                      <Text style={styles.acctSummary}>Provider failover: see Accounts above.</Text>
                    </View>
                  ) : null
                })()
              : null}
            {assignments.length > 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardTitle}>Assignments ({assignments.length})</Text>
                <Text style={styles.acctSummary}>open work leases · read-only</Text>
                {assignments.map((a) => (
                  <View key={a.leaseRef} style={styles.acctDetailRow}>
                    <Text style={styles.acctText} numberOfLines={2}>
                      {a.goal || a.assignmentRef.slice(-8)}
                    </Text>
                    <Text style={styles.acctMeta}>
                      {a.paymentMode}
                      {a.expiresAt ? ` · expires ${a.expiresAt.slice(0, 10)}` : ""} · {a.assignmentRef.slice(-6)}
                    </Text>
                  </View>
                ))}
              </View>
            ) : null}
            {sessions.length === 0 && status === "connected" ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No sessions yet. Spawn one on the node.</Text>
              </View>
            ) : (
              (() => {
                const childrenOf = (ref: string) => sessions.filter((c) => c.parentRef === ref)
                const isTop = (s: ControlSessionRow) =>
                  !s.parentRef || !sessions.some((p) => p.sessionRef === s.parentRef)
                const renderRow = (s: ControlSessionRow, child: boolean) => (
                  <Pressable
                    key={s.sessionRef}
                    style={[styles.row, child ? styles.childRow : null]}
                    onPress={() => setSelected(s.sessionRef)}
                  >
                    <View style={[styles.dot, { backgroundColor: stateTone(s.state) }]} />
                    <View style={styles.rowText}>
                      <Text style={styles.rowLabel} numberOfLines={2}>
                        {child ? "↳ " : ""}
                        {s.latestActivity || s.state}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {s.agentKind ? `${s.agentKind} · ` : ""}
                        {s.state} · {s.sessionRef.slice(-6)}
                      </Text>
                    </View>
                    <Text style={styles.chevron}>›</Text>
                  </Pressable>
                )
                const rows: ReactNode[] = []
                for (const s of sessions.filter(isTop)) {
                  rows.push(renderRow(s, false))
                  for (const c of childrenOf(s.sessionRef)) rows.push(renderRow(c, true))
                }
                return rows
              })()
            )}
          </>
        )}
        </ScrollView>
      </View>
    </Drawer>
  )
}

// Drawer panel — ignite DemoShowroomScreen layout: header at top, then a
// vertical list of nav destinations that route the stack and close the drawer.
function DrawerNav({
  nodeName,
  onNavigate,
}: {
  nodeName: string | null
  onNavigate: (route: string) => void
}) {
  const items: { label: string; route: string }[] = [
    { label: "Nodes", route: "Nodes" },
    { label: "Sessions", route: "Sessions" },
    { label: "Decisions", route: "Decisions" },
    { label: "Spawn", route: "Spawn" },
    { label: "Settings", route: "Settings" },
  ]
  return (
    <View style={styles.drawerContent}>
      <View style={styles.drawerHeader}>
        <Text style={styles.drawerTitle}>Autopilot</Text>
        <Text style={styles.drawerSubtitle}>{nodeName ?? "not connected"}</Text>
      </View>
      {items.map((item) => (
        <Pressable key={item.route} style={styles.drawerItem} onPress={() => onNavigate(item.route)}>
          <Text style={styles.drawerItemText}>{item.label}</Text>
        </Pressable>
      ))}
    </View>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  content: { padding: 24, paddingTop: 12 },
  headerBar: {
    alignItems: "center",
    backgroundColor: C.bg,
    flexDirection: "row",
    justifyContent: "space-between",
    paddingBottom: 4,
    paddingHorizontal: 16,
    paddingTop: 50,
  },
  coordToggle: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 6, borderWidth: 1, paddingHorizontal: 12, paddingVertical: 6 },
  coordPaused: { borderColor: C.warning },
  coordToggleText: { color: C.text, fontFamily: "Courier", fontSize: 12 },
  drawer: { backgroundColor: C.bgSecondary, width: 300 },
  drawerContent: { backgroundColor: C.bgSecondary, flex: 1, paddingTop: 60 },
  drawerHeader: {
    borderBottomColor: C.outline,
    borderBottomWidth: 1,
    paddingBottom: 20,
    paddingHorizontal: 20,
  },
  drawerTitle: { color: C.primary, fontSize: 20, fontWeight: "700" },
  drawerSubtitle: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 4 },
  drawerItem: {
    borderBottomColor: C.outline,
    borderBottomWidth: StyleSheet.hairlineWidth,
    paddingHorizontal: 20,
    paddingVertical: 16,
  },
  drawerItemText: { color: C.text, fontSize: 16 },
  h1: { color: C.primary, fontSize: 22, fontWeight: "700" },
  subtitle: { color: C.textSecondary, fontSize: 13, letterSpacing: 1, marginTop: 4, textTransform: "uppercase" },
  card: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, marginTop: 24, padding: 18 },
  cardTitle: { color: C.primary, fontSize: 16, fontWeight: "600" },
  cardBody: { color: C.textSecondary, fontSize: 14, lineHeight: 20, marginTop: 8 },
  input: { backgroundColor: C.bg, borderColor: C.outline, borderRadius: 6, borderWidth: 1, color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 14, padding: 12 },
  inputMultiline: { minHeight: 64, textAlignVertical: "top" },
  askStatus: { color: C.textSecondary, fontSize: 12, marginTop: 10 },
  button: { alignItems: "center", backgroundColor: C.primary, borderRadius: 6, marginTop: 12, padding: 12 },
  buttonText: { color: C.bg, fontSize: 15, fontWeight: "700" },
  error: { color: C.danger, fontSize: 13, marginTop: 10 },
  statusRow: { alignItems: "center", flexDirection: "row", gap: 10, marginTop: 24 },
  statusText: { color: C.text, fontFamily: "Courier", fontSize: 14 },
  breakdown: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 6 },
  acctRow: { alignItems: "center", flexDirection: "row", marginTop: 10 },
  acctText: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  acctSummary: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 6 },
  approvalCard: { borderColor: C.warning, borderWidth: 1 },
  approvalRow: { borderTopColor: C.outline, borderTopWidth: StyleSheet.hairlineWidth, marginTop: 10, paddingTop: 10 },
  approvalButtons: { flexDirection: "row", gap: 10, marginTop: 8 },
  approvalBtn: { borderRadius: 6, paddingHorizontal: 16, paddingVertical: 8 },
  approveBtn: { backgroundColor: C.success },
  denyBtn: { backgroundColor: C.danger },
  approvalBtnText: { color: C.bg, fontWeight: "700" },
  acctHeader: { alignItems: "center", flexDirection: "row", justifyContent: "space-between" },
  acctDetailRow: {
    borderTopColor: C.outline,
    borderTopWidth: StyleSheet.hairlineWidth,
    marginTop: 10,
    paddingTop: 10,
  },
  acctDetailTop: { alignItems: "center", flexDirection: "row" },
  acctMeta: { color: C.textSecondary, fontFamily: "Courier", fontSize: 11, marginTop: 4 },
  acctBlockers: { color: C.warning, fontFamily: "Courier", fontSize: 10, marginTop: 3 },
  balanceValue: { color: C.text, fontFamily: "Courier", fontSize: 22, fontWeight: "600", marginTop: 4 },
  row: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, flexDirection: "row", marginTop: 12, padding: 14 },
  childRow: { marginLeft: 22, marginTop: 6, backgroundColor: C.bg },
  dot: { borderRadius: 6, height: 12, marginRight: 12, width: 12 },
  rowText: { flex: 1 },
  rowLabel: {
    color: C.text,
    fontFamily: "Courier",
    fontSize: ROW_LABEL_FONT_SIZE,
    height: fixedRowLabelHeight(ROW_LABEL_LINE_HEIGHT),
    lineHeight: ROW_LABEL_LINE_HEIGHT,
  },
  expandedRowLabel: { height: undefined },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
  chevron: { color: C.textSecondary, fontSize: 20, marginLeft: 8 },
  back: { marginTop: 20 },
  backText: { color: C.info, fontSize: 15 },
  detailRef: { color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 10 },
  verifyLine: { fontFamily: "Courier", fontSize: 13, marginTop: 8 },
  artifactLine: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 4 },
  cancelBtn: { alignItems: "center", borderColor: C.danger, borderRadius: 6, borderWidth: 1, marginTop: 12, padding: 10 },
  cancelText: { color: C.danger, fontSize: 14, fontWeight: "600" },
  eventRow: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 6, borderWidth: 1, flexDirection: "row", marginTop: 8, padding: 12 },
})
