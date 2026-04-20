import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AutopilotStatus = {
  product: string;
  shell: string;
  rustAuthority: string;
  runtimeLane: string;
};

type IpcState = "pending" | "ok" | "error";

type RegisterRow = {
  field: string;
  value: string;
  evidence: string;
  tone?: "positive" | "warning" | "negative" | "info";
};

const fallbackStatus: AutopilotStatus = {
  product: "Autopilot",
  shell: "Tauri",
  rustAuthority: "connecting",
  runtimeLane: "prototype",
};

function formatClock() {
  return new Date().toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function App() {
  const [status, setStatus] = useState<AutopilotStatus>(fallbackStatus);
  const [ipcState, setIpcState] = useState<IpcState>("pending");
  const [updatedAt, setUpdatedAt] = useState<string>("pending");

  useEffect(() => {
    invoke<AutopilotStatus>("autopilot_status")
      .then((nextStatus) => {
        setStatus(nextStatus);
        setIpcState("ok");
        setUpdatedAt(formatClock());
      })
      .catch(() => {
        setStatus(fallbackStatus);
        setIpcState("error");
        setUpdatedAt(formatClock());
      });
  }, []);

  const stateRows: RegisterRow[] = [
    {
      field: "PRODUCT",
      value: status.product,
      evidence: "tauri command payload",
    },
    {
      field: "SHELL",
      value: status.shell,
      evidence: "desktop host",
      tone: "info",
    },
    {
      field: "RUST_AUTHORITY",
      value: status.rustAuthority,
      evidence: "ipc bridge",
      tone: ipcState === "ok" ? "positive" : "warning",
    },
    {
      field: "RUNTIME_LANE",
      value: status.runtimeLane,
      evidence: "attached lane",
    },
    {
      field: "IPC_STATE",
      value: ipcState.toUpperCase(),
      evidence: "autopilot_status",
      tone:
        ipcState === "ok" ? "positive" : ipcState === "error" ? "negative" : "warning",
    },
    {
      field: "UPDATED_AT",
      value: updatedAt,
      evidence: "local clock",
    },
  ];

  const authorityRows: RegisterRow[] = [
    {
      field: "PRIVILEGED_STATE",
      value: "Rust",
      evidence: "Tauri command boundary",
    },
    {
      field: "PRODUCT_SURFACE",
      value: "TypeScript",
      evidence: "React projection",
    },
    {
      field: "CONTROL_PATTERN",
      value: "Explicit",
      evidence: "table rows, named fields",
      tone: "info",
    },
    {
      field: "VISUAL_BUDGET",
      value: "Dense",
      evidence: "11px register rhythm",
      tone: "warning",
    },
  ];

  return (
    <main className="shell">
      <section className="terminal" aria-label="Autopilot">
        <header className="status-strip">
          <span>OPENAGENTS</span>
          <span>AUTOPILOT</span>
          <span>DESKTOP</span>
          <span data-tone={ipcState === "ok" ? "positive" : "warning"}>
            {ipcState.toUpperCase()}
          </span>
        </header>

        <section className="command-strip" aria-label="Command state">
          <div>
            <span>COMMAND</span>
            <strong>autopilot_status</strong>
          </div>
          <div>
            <span>HOST</span>
            <strong>{status.shell}</strong>
          </div>
          <div>
            <span>AUTHORITY</span>
            <strong>{status.rustAuthority}</strong>
          </div>
          <div>
            <span>LANE</span>
            <strong>{status.runtimeLane}</strong>
          </div>
        </section>

        <section className="pane-grid">
          <article className="pane pane-wide">
            <div className="pane-header">
              <span>STATE REGISTER</span>
              <strong>{updatedAt}</strong>
            </div>
            <table className="data-table">
              <thead>
                <tr>
                  <th>FIELD</th>
                  <th>VALUE</th>
                  <th>EVIDENCE</th>
                </tr>
              </thead>
              <tbody>
                {stateRows.map((row) => (
                  <tr key={row.field}>
                    <td>{row.field}</td>
                    <td data-tone={row.tone}>{row.value}</td>
                    <td>{row.evidence}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </article>

          <article className="pane">
            <div className="pane-header">
              <span>AUTHORITY MAP</span>
              <strong>LOCAL</strong>
            </div>
            <table className="data-table compact">
              <tbody>
                {authorityRows.map((row) => (
                  <tr key={row.field}>
                    <td>{row.field}</td>
                    <td data-tone={row.tone}>{row.value}</td>
                    <td>{row.evidence}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </article>

          <article className="pane">
            <div className="pane-header">
              <span>EXECUTION SURFACE</span>
              <strong>READ ONLY</strong>
            </div>
            <div className="log-tape" aria-label="Execution details">
              <p>
                <span>01</span>
                <strong>UI</strong>
                React/TypeScript renders product state.
              </p>
              <p>
                <span>02</span>
                <strong>IPC</strong>
                Tauri command returns typed shell status.
              </p>
              <p>
                <span>03</span>
                <strong>CORE</strong>
                Rust remains the authority boundary.
              </p>
            </div>
          </article>
        </section>
      </section>
    </main>
  );
}

export default App;
