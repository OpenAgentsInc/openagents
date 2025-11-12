import { useModelStore } from "@/lib/model-store";

export function AppHeader() {
  const model = useModelStore((s) => s.selected);
  const setModel = useModelStore((s) => s.setSelected);

  return (
    <div className="flex h-12 w-full items-center border-b border-zinc-800 bg-zinc-950 pr-4 pl-16 md:pl-20">
      <div className="flex items-center gap-2">
        <img src="/oalogo.png" alt="OpenAgents" className="size-5" />
        <span className="font-semibold">OpenAgents</span>
      </div>
      <div className="ml-auto flex items-center gap-2">
        <label className="text-xs text-zinc-400">Model</label>
        <select
          className="bg-zinc-900 text-zinc-100 text-xs border border-zinc-700 rounded px-2 py-1"
          value={model}
          onChange={(e) => setModel(e.target.value as any)}
        >
          <option value="codex">Codex (ACP)</option>
          <option value="claude-code">Claude Code</option>
          <option value="ollama">Ollama (glm-4.6:cloud)</option>
        </select>
      </div>
    </div>
  );
}
