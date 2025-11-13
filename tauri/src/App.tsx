import "./App.css";
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar";
//
import { useDarkModeRoot } from "@/lib/useDarkMode";
import { MyRuntimeProvider } from "@/runtime/MyRuntimeProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { MobileConnectionHandler } from "@/components/mobile/MobileConnectionHandler";
import { useWorkingDirStore } from "@/lib/working-dir-store";
import { useEffect } from "react";
import { useProjectStore } from "@/lib/project-store";
import { invoke } from "@tauri-apps/api/core";

function App() {
  // Ensure dark variables apply to portals (e.g., shadcn Select)
  useDarkModeRoot();
  const projects = useProjectStore((s) => s.projects);

  // Initialize working directory store
  useEffect(() => {
    const workingDirStore = useWorkingDirStore.getState();

    // If no default is set yet (not in localStorage), use repo root as reasonable default
    if (!workingDirStore.defaultCwd) {
      // Default to the openagents repo root
      const defaultCwd = "/Users/christopherdavid/code/openagents";
      workingDirStore.setDefaultCwd(defaultCwd);
    }
  }, []);

  // One-time repair: if the global default was polluted by a project path, reset to repo root
  useEffect(() => {
    const workingDirStore = useWorkingDirStore.getState();
    const current = workingDirStore.defaultCwd;
    if (current && projects.some((p) => p.path === current)) {
      const repoRoot = "/Users/christopherdavid/code/openagents";
      if (current !== repoRoot) {
        workingDirStore.setDefaultCwd(repoRoot);
      }
    }
  }, [projects]);

  return (
    <ErrorBoundary>
      <MobileConnectionHandler>
        <MyRuntimeProvider>
          <div className="dark fixed inset-0 flex h-screen w-screen bg-zinc-900 text-white">
            <AssistantSidebar />
          </div>
        </MyRuntimeProvider>
      </MobileConnectionHandler>
    </ErrorBoundary>
  );
}

export default App;
