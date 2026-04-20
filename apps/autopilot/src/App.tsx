import "./App.css";
import * as React from "react";
import { Command } from "cmdk";
import { CheckCircle, Command as CommandIcon, Pulse } from "@phosphor-icons/react";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type DemoView = "runtime" | "evidence";

const viewLabels: Record<DemoView, string> = {
  runtime: "Runtime",
  evidence: "Evidence",
};

function App() {
  const [activeView, setActiveView] = React.useState<DemoView>("runtime");
  const commandInputRef = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        commandInputRef.current?.focus();
        commandInputRef.current?.select();
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const selectView = (view: DemoView) => {
    setActiveView(view);
    commandInputRef.current?.blur();
  };

  return (
    <main className="dark shell">
      <section className="autopilot-frame">
        <div className="command-panel">
          <div className="command-panel__header">
            <CommandIcon aria-hidden="true" data-icon="inline-start" />
            <span>COMMAND INDEX</span>
            <kbd>⌘K</kbd>
          </div>

          <Command label="Autopilot demo command index" loop>
            <Command.Input
              ref={commandInputRef}
              aria-label="Filter demo commands"
              placeholder="filter commands"
            />
            <Command.List>
              <Command.Empty>No command matched.</Command.Empty>
              <Command.Group heading="Demo views">
                <Command.Item
                  value="show runtime card"
                  keywords={["runtime", "state", "worker"]}
                  onSelect={() => selectView("runtime")}
                >
                  <Pulse aria-hidden="true" data-icon="inline-start" />
                  <span>Show Runtime Card</span>
                  {activeView === "runtime" ? <strong>ACTIVE</strong> : null}
                </Command.Item>
                <Command.Item
                  value="show evidence card"
                  keywords={["evidence", "verification", "proof"]}
                  onSelect={() => selectView("evidence")}
                >
                  <CheckCircle aria-hidden="true" data-icon="inline-start" />
                  <span>Show Evidence Card</span>
                  {activeView === "evidence" ? <strong>ACTIVE</strong> : null}
                </Command.Item>
              </Command.Group>
            </Command.List>
          </Command>
        </div>

        <div className="demo-panel" aria-live="polite">
          <div className="demo-panel__status">
            <span>ACTIVE VIEW</span>
            <strong>{viewLabels[activeView]}</strong>
          </div>

          {activeView === "runtime" ? <RuntimeDemoCard /> : <EvidenceDemoCard />}
        </div>
      </section>
    </main>
  );
}

function RuntimeDemoCard() {
  return (
    <Card className="demo-card">
      <CardHeader>
        <CardTitle>Runtime Card</CardTitle>
        <CardDescription>Command-selected view for worker state.</CardDescription>
      </CardHeader>
      <CardContent>
        <dl className="register-grid">
          <div>
            <dt>lane</dt>
            <dd>probe.worker.local</dd>
          </div>
          <div>
            <dt>heartbeat</dt>
            <dd>1.2s</dd>
          </div>
          <div>
            <dt>authority</dt>
            <dd>tauri.shell.prototype</dd>
          </div>
        </dl>
      </CardContent>
    </Card>
  );
}

function EvidenceDemoCard() {
  return (
    <Card className="demo-card">
      <CardHeader>
        <CardTitle>Evidence Card</CardTitle>
        <CardDescription>Command-selected view for verification state.</CardDescription>
      </CardHeader>
      <CardContent>
        <dl className="register-grid">
          <div>
            <dt>receipt</dt>
            <dd>accepted</dd>
          </div>
          <div>
            <dt>verification</dt>
            <dd>local.build.green</dd>
          </div>
          <div>
            <dt>delivery</dt>
            <dd>ready</dd>
          </div>
        </dl>
      </CardContent>
    </Card>
  );
}

export default App;
