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
      <section className="workspace">
        <div className="eyebrow">OpenAgents desktop</div>
        <h1>{status.product}</h1>
        <p className="lede">Local operator shell online.</p>
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
      </section>
    </main>
  );
}

export default App;
