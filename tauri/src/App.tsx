import "./App.css";
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar";
//
import { useDarkModeRoot } from "@/lib/useDarkMode";
import { MyRuntimeProvider } from "@/runtime/MyRuntimeProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { MobileConnectionHandler } from "@/components/mobile/MobileConnectionHandler";
import { useWorkingDirStore } from "@/lib/working-dir-store";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

function App() {
  // Ensure dark variables apply to portals (e.g., shadcn Select)
  useDarkModeRoot();

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
