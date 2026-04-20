import "./App.css";
import * as React from "react";
import {
  CheckCircle,
  Command as CommandIcon,
  Pulse,
} from "@phosphor-icons/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Separator } from "@/components/ui/separator";

type DemoView = "runtime" | "evidence";

const viewLabels: Record<DemoView, string> = {
  runtime: "Runtime",
  evidence: "Evidence",
};

function App() {
  const [activeView, setActiveView] = React.useState<DemoView>("runtime");
  const [commandOpen, setCommandOpen] = React.useState(false);

  usePreferredThemeClass();

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setCommandOpen((open) => !open);
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const selectView = (view: DemoView) => {
    setActiveView(view);
    setCommandOpen(false);
  };

  return (
    <main className="shell grid place-items-center p-4">
      <section className="command-stage">
        <div className="command-stage__bar">
          <Badge variant="outline">ACTIVE: {viewLabels[activeView]}</Badge>

          <Button
            type="button"
            variant="outline"
            onClick={() => setCommandOpen(true)}
          >
            <CommandIcon aria-hidden="true" data-icon="inline-start" />
            Command
            <KbdGroup className="ml-1">
              <Kbd>⌘</Kbd>
              <Kbd>K</Kbd>
            </KbdGroup>
          </Button>
        </div>

        <Separator />

        <div className="command-stage__card" aria-live="polite">
          {activeView === "runtime" ? <RuntimeDemoCard /> : <EvidenceDemoCard />}
        </div>
      </section>

      <CommandDialog
        open={commandOpen}
        onOpenChange={setCommandOpen}
        title="Autopilot Command Menu"
        description="Switch between Autopilot demo views."
        className="sm:max-w-lg"
      >
        <Command label="Autopilot demo commands" loop>
          <CommandInput placeholder="Type a command or filter by evidence, runtime, proof..." />
          <CommandList>
            <CommandEmpty>No command matched.</CommandEmpty>
            <CommandGroup heading="Demo views">
              <CommandItem
                value="show runtime card"
                keywords={["runtime", "state", "worker"]}
                data-checked={activeView === "runtime"}
                onSelect={() => selectView("runtime")}
              >
                <Pulse aria-hidden="true" data-icon="inline-start" />
                <span>Show Runtime Card</span>
                <CommandShortcut>runtime</CommandShortcut>
              </CommandItem>
              <CommandSeparator />
              <CommandItem
                value="show evidence card"
                keywords={["evidence", "verification", "proof"]}
                data-checked={activeView === "evidence"}
                onSelect={() => selectView("evidence")}
              >
                <CheckCircle aria-hidden="true" data-icon="inline-start" />
                <span>Show Evidence Card</span>
                <CommandShortcut>evidence</CommandShortcut>
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </CommandDialog>
    </main>
  );
}

function RuntimeDemoCard() {
  return (
    <Card className="demo-card">
      <CardHeader>
        <CardTitle>Runtime</CardTitle>
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
        <CardTitle>Evidence</CardTitle>
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

function usePreferredThemeClass() {
  React.useLayoutEffect(() => {
    const root = document.documentElement;
    const preference = window.matchMedia("(prefers-color-scheme: dark)");

    const applyPreference = () => {
      root.classList.toggle("dark", preference.matches);
      root.style.colorScheme = preference.matches ? "dark" : "light";
    };

    applyPreference();
    preference.addEventListener("change", applyPreference);

    return () => {
      preference.removeEventListener("change", applyPreference);
      root.classList.remove("dark");
      root.style.removeProperty("color-scheme");
    };
  }, []);
}

export default App;
