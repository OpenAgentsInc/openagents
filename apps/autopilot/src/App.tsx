import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AutopilotStatus = {
  product: string;
  shell: string;
  rustAuthority: string;
  runtimeLane: string;
};

const fallbackStatus: AutopilotStatus = {
  product: "Autopilot",
  shell: "Tauri",
  rustAuthority: "connecting",
  runtimeLane: "prototype",
};

function App() {
  const [status, setStatus] = useState<AutopilotStatus>(fallbackStatus);

  useEffect(() => {
    invoke<AutopilotStatus>("autopilot_status")
      .then(setStatus)
      .catch(() => setStatus(fallbackStatus));
  }, []);

  return (
    <main className="shell">
      <section className="app-frame" aria-label="Autopilot">
        <aside className="side-rail" aria-label="Autopilot navigation">
          <div className="brand-row">
            <div className="brand-mark" aria-hidden="true">
              OA
            </div>
            <div>
              <span>OpenAgents</span>
              <strong>{status.product}</strong>
            </div>
          </div>

          <nav className="nav-stack" aria-label="Primary">
            <a aria-current="page" href="#mission">
              Mission
            </a>
            <a href="#runtime">Runtime</a>
          </nav>

          <div className="rail-meter" aria-label="Operator mode">
            <span>Mode</span>
            <strong>LOCAL</strong>
          </div>
        </aside>

        <section className="stage">
          <header className="topbar">
            <div>
              <span className="section-kicker">\\ MISSION CONTROL</span>
              <h1>{status.product}</h1>
            </div>
            <div className="status-pill" aria-label="Shell status">
              <span aria-hidden="true" />
              Online
            </div>
          </header>

          <section className="workspace" id="mission">
            <article className="mission-panel">
              <div className="panel-header">
                <span>\\ OPERATOR SHELL</span>
                <strong>READY</strong>
              </div>
              <p className="terminal-line">local operator shell online</p>
              <div className="status-grid" aria-label="Autopilot status">
                <article>
                  <span>Shell</span>
                  <strong>{status.shell}</strong>
                </article>
                <article>
                  <span>Rust core</span>
                  <strong>{status.rustAuthority}</strong>
                </article>
                <article>
                  <span>Runtime lane</span>
                  <strong>{status.runtimeLane}</strong>
                </article>
              </div>
            </article>

            <aside className="telemetry-panel" id="runtime" aria-label="Runtime">
              <div className="panel-header">
                <span>\\ RUNTIME</span>
                <strong>TAURI</strong>
              </div>
              <dl>
                <div>
                  <dt>Window</dt>
                  <dd>Desktop</dd>
                </div>
                <div>
                  <dt>Authority</dt>
                  <dd>Rust</dd>
                </div>
                <div>
                  <dt>Surface</dt>
                  <dd>TypeScript</dd>
                </div>
              </dl>
            </aside>
          </section>
        </section>
      </section>
    </main>
  );
}

export default App;
