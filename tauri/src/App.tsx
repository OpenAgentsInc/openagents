import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <div className="h-screen w-screen bg-black text-white flex flex-col overflow-hidden">
      {/* Custom title bar drag region - keeps traffic lights */}
      <div data-tauri-drag-region className="h-12 flex-shrink-0 w-full" />

      {/* Main content */}
      <div className="flex-1 flex flex-col items-center justify-center p-8">
        <h1 className="text-4xl font-bold mb-8">OpenAgents</h1>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            greet();
          }}
          className="flex flex-col gap-4 w-full max-w-md"
        >
          <input
            id="greet-input"
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="Enter a name..."
            className="px-4 py-2 bg-zinc-900 text-white border border-zinc-700 rounded-md focus:outline-none focus:ring-2 focus:ring-zinc-500"
          />
          <Button type="submit">
            Greet
          </Button>
        </form>

        {greetMsg && (
          <p className="mt-4 text-zinc-300">{greetMsg}</p>
        )}
      </div>
    </div>
  );
}

export default App;
