import "./App.css";
import * as React from "react";
import {
  CheckCircle,
  Command as CommandIcon,
  Moon,
  Pulse,
  Sun,
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
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

type DemoView = "runtime" | "evidence";
type Theme = "light" | "dark";

const viewLabels: Record<DemoView, string> = {
  runtime: "Runtime",
  evidence: "Evidence",
};

function App() {
  const [activeView, setActiveView] = React.useState<DemoView>("runtime");
  const [commandOpen, setCommandOpen] = React.useState(false);
  const [theme, setTheme] = useTheme();

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

  const toggleTheme = () => {
    setTheme((currentTheme) => (currentTheme === "dark" ? "light" : "dark"));
    setCommandOpen(false);
  };

  return (
    <main className="shell grid place-items-center p-4">
      <section className="command-stage">
        <div className="command-stage__bar">
          <Badge variant="outline">ACTIVE: {viewLabels[activeView]}</Badge>

          <div className="command-stage__actions">
            <ToggleGroup
              aria-label="Theme"
              value={[theme]}
              variant="outline"
              size="sm"
              onValueChange={(value) => {
                const nextTheme = value[0];

                if (nextTheme === "light" || nextTheme === "dark") {
                  setTheme(nextTheme);
                }
              }}
            >
              <ToggleGroupItem value="dark" aria-label="Use dark theme">
                <Moon aria-hidden="true" data-icon="inline-start" />
                Dark
              </ToggleGroupItem>
              <ToggleGroupItem value="light" aria-label="Use light theme">
                <Sun aria-hidden="true" data-icon="inline-start" />
                Light
              </ToggleGroupItem>
            </ToggleGroup>

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
          <CommandInput placeholder="Type a command or filter by runtime, evidence, theme..." />
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
            <CommandSeparator />
            <CommandGroup heading="Theme">
              <CommandItem
                value="toggle theme"
                keywords={["light", "dark", "mode", "appearance"]}
                onSelect={toggleTheme}
              >
                {theme === "dark" ? (
                  <Sun aria-hidden="true" data-icon="inline-start" />
                ) : (
                  <Moon aria-hidden="true" data-icon="inline-start" />
                )}
                <span>
                  Switch to {theme === "dark" ? "Light" : "Dark"} Theme
                </span>
                <CommandShortcut>
                  {theme === "dark" ? "light" : "dark"}
                </CommandShortcut>
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

function useTheme(): [Theme, React.Dispatch<React.SetStateAction<Theme>>] {
  const [theme, setTheme] = React.useState<Theme>(() => {
    const storedTheme = window.localStorage.getItem("autopilot.theme");

    return storedTheme === "light" ? "light" : "dark";
  });

  React.useLayoutEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", theme === "dark");
    root.classList.toggle("light", theme === "light");
    root.style.colorScheme = theme;
    window.localStorage.setItem("autopilot.theme", theme);
  }, [theme]);

  return [theme, setTheme];
}

export default App;
