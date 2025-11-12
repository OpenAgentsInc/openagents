import "./App.css";
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar";
//
import { useDarkModeRoot } from "@/lib/useDarkMode";
import { MyRuntimeProvider } from "@/runtime/MyRuntimeProvider";
import { ErrorBoundary } from "@/components/ErrorBoundary";

function App() {
  // Ensure dark variables apply to portals (e.g., shadcn Select)
  useDarkModeRoot();
  return (
    <ErrorBoundary>
      <MyRuntimeProvider>
        <div className="dark fixed inset-0 flex h-screen w-screen bg-zinc-900 text-white">
          <AssistantSidebar />
        </div>
      </MyRuntimeProvider>
    </ErrorBoundary>
  );
}

export default App;
