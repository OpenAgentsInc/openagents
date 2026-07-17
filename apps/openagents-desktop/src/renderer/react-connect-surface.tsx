import { useState, type ReactElement } from "react"
import { Cloud, Link2, MonitorSmartphone, RefreshCw, ShieldCheck, Smartphone } from "lucide-react"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import { Badge } from "#components/ui/badge"
import { Button } from "#components/ui/button"
import { Input } from "#components/ui/input"
import type { RemoteConnectProjection } from "./remote-connect.ts"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload) as Effect.Effect<void, IntentError>).catch(() => undefined)
}

export const ReactConnectSurface = ({ connections, report }: {
  readonly connections: RemoteConnectProjection
  readonly report: IntentReporter
}): ReactElement => {
  const [environmentId, setEnvironmentId] = useState("")
  const [execServerUrl, setExecServerUrl] = useState("")
  const busy = connections.phase === "loading" || connections.phase === "mutating"
  const remote = connections.remote
  return <div className="oa-react-connect-stack">
    <section className="oa-react-settings-card" aria-labelledby="settings-connections-heading">
      <header><div><p>Portable operation</p><h2 id="settings-connections-heading">Connections</h2></div><Button size="sm" variant="outline" disabled={busy} onClick={() => dispatch(report, "DesktopConnectionsRefreshRequested")}><RefreshCw aria-hidden="true" />Refresh</Button></header>
      {connections.phase === "idle" || connections.phase === "loading" ? <div className="oa-react-settings-skeleton" aria-label="Loading connections"><span /><span /><span /></div> : connections.phase === "unavailable" ? <div className="oa-react-settings-empty"><Cloud aria-hidden="true" /><h3>Remote control unavailable</h3><p>{connections.notice ?? "The current Codex runtime did not admit its experimental remote-control contract."}</p></div> : <>
        <div className="oa-react-connect-summary"><div><span className="oa-react-connect-state" data-state={remote.state} /><div><strong>{remote.state === "connected" ? "Remote control connected" : remote.state === "connecting" ? "Connecting remote control" : remote.state === "errored" ? "Remote control needs attention" : "Remote control is off"}</strong><small>Codex experimental manifest {connections.manifestReady ? "verified" : "incomplete"} · revision {connections.revision}</small></div></div><div>{remote.state === "disabled" ? <Button disabled={busy || !connections.manifestReady} onClick={() => dispatch(report, "DesktopRemoteControlEnabled")}>Enable</Button> : <Button variant="outline" disabled={busy} onClick={() => dispatch(report, "DesktopRemoteControlDisabled")}>Disable</Button>}</div></div>
        <div className="oa-react-connect-grid">
          <article><Smartphone aria-hidden="true" /><div><strong>Pair OpenAgents mobile</strong><p>Create a short-lived pairing session. Pairing codes and installation identity stay in main memory and never enter this renderer.</p></div><Button disabled={busy || remote.state !== "connected"} onClick={() => dispatch(report, "DesktopRemotePairingStarted", true)}>Start pairing</Button></article>
          <article><MonitorSmartphone aria-hidden="true" /><div><strong>Portable session</strong><p>Queue, Steer, Stop, and thread projections remain scoped to the capabilities granted by the connected runtime.</p></div><Badge variant="outline">capability scoped</Badge></article>
        </div>
        {remote.pairing === null ? null : <div className="oa-react-connect-pairing" role="status"><div><strong>Pairing {remote.pairing.state}</strong><span>{remote.pairing.pairingRef} · expires {new Date(remote.pairing.expiresAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span></div><Button size="sm" variant="outline" disabled={busy || remote.pairing.state !== "pending"} onClick={() => dispatch(report, "DesktopRemotePairingChecked", remote.pairing!.pairingRef)}>Check status</Button></div>}
      </>}
    </section>

    <section className="oa-react-settings-card" aria-labelledby="settings-environments-heading">
      <header><div><p>Execution</p><h2 id="settings-environments-heading">Remote environments</h2></div></header>
      <div className="oa-react-settings-rows">{connections.environments.length === 0 ? <p>No remote execution environment is connected.</p> : connections.environments.map(environment => <article key={environment.environmentRef}><div><strong>{environment.environmentRef}</strong><small>{environment.shell ?? "shell unknown"} · {environment.cwdRef ?? "cwd protected"}</small></div><Badge variant="outline">{environment.state}</Badge><span>Reference-only projection</span></article>)}</div>
      <form className="oa-react-connect-form" onSubmit={event => { event.preventDefault(); if (environmentId.trim() === "" || execServerUrl.trim() === "") return; dispatch(report, "DesktopRemoteEnvironmentAdded", { environmentId: environmentId.trim(), execServerUrl: execServerUrl.trim() }) }}>
        <label><span>Environment ID</span><Input value={environmentId} maxLength={160} autoComplete="off" onChange={event => setEnvironmentId(event.currentTarget.value)} placeholder="development-mac" /></label>
        <label><span>Exec server URL</span><Input value={execServerUrl} maxLength={2048} autoComplete="off" onChange={event => setExecServerUrl(event.currentTarget.value)} placeholder="wss://approved-host.example/exec" /></label>
        <Button type="submit" disabled={busy || environmentId.trim() === "" || execServerUrl.trim() === ""}>Connect environment</Button>
      </form>
      <p className="oa-react-settings-boundary"><ShieldCheck aria-hidden="true" />This form passes a confirmed typed environment request to main. It stores no SSH key, password, or service credential in renderer state.</p>
    </section>

    <section className="oa-react-settings-card" aria-labelledby="settings-mobile-clients-heading">
      <header><div><p>Access</p><h2 id="settings-mobile-clients-heading">Mobile clients</h2></div>{remote.environmentRef === null ? null : <Button size="sm" variant="outline" disabled={busy} onClick={() => dispatch(report, "DesktopRemoteClientsRequested", remote.environmentRef!)}>Refresh clients</Button>}</header>
      <div className="oa-react-settings-rows">{remote.clients.length === 0 ? <p>No granted mobile clients are visible.</p> : remote.clients.map(client => <article key={client.clientRef}><div><strong>{client.displayName ?? "Unnamed client"}</strong><small>{client.platform ?? "unknown platform"} · {client.clientRef}</small></div><Badge variant="outline">{client.state}</Badge><Button size="sm" variant="outline" disabled={busy || client.state === "revoked" || remote.environmentRef === null} onClick={() => dispatch(report, "DesktopRemoteClientRevoked", { environmentRef: remote.environmentRef!, clientRef: client.clientRef })}>Revoke</Button></article>)}</div>
      <div className="oa-react-connect-unavailable"><Link2 aria-hidden="true" /><div><strong>SSH credential prompts stay native</strong><p>No admitted renderer contract can read, store, or submit an SSH password or private key. A future native credential broker may add that path without widening this surface.</p></div></div>
    </section>
    {connections.notice === null || connections.phase === "unavailable" ? null : <p className="oa-react-connect-notice" role="status">{connections.notice}</p>}
  </div>
}
