/**
 * Server Selection Component
 *
 * Shows a list of discovered OpenAgents servers and allows the user to select one.
 */

import type { ServerInfo } from "@/lib/mobileServerDiscovery";

interface ServerSelectionProps {
  servers: ServerInfo[];
  onSelect: (server: ServerInfo) => void;
  onRefresh: () => void;
}

export function ServerSelection({ servers, onSelect, onRefresh }: ServerSelectionProps) {
  return (
    <div className="flex h-screen w-screen flex-col bg-zinc-900 text-white">
      <div className="flex-1 overflow-auto p-6">
        <div className="mx-auto max-w-md">
          <h1 className="mb-2 text-2xl font-bold">Select Desktop Server</h1>
          <p className="mb-6 text-sm text-zinc-400">
            Multiple OpenAgents servers found on your network. Select the one you want to connect to:
          </p>

          <div className="space-y-3">
            {servers.map((server, index) => (
              <button
                key={`${server.host}:${server.port}-${index}`}
                onClick={() => onSelect(server)}
                className="w-full rounded-lg border border-zinc-700 bg-zinc-800 p-4 text-left transition-colors hover:border-blue-500 hover:bg-zinc-750"
              >
                <div className="mb-1 font-semibold">{server.name}</div>
                <div className="text-sm text-zinc-400">
                  {server.host}:{server.port}
                </div>
                <div className="mt-2 text-xs text-zinc-500">
                  Discovered {new Date(server.discoveredAt).toLocaleTimeString()}
                </div>
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="border-t border-zinc-800 p-4">
        <button
          onClick={onRefresh}
          className="w-full rounded-lg bg-zinc-800 px-4 py-3 font-semibold text-white hover:bg-zinc-700"
        >
          Scan Again
        </button>
      </div>
    </div>
  );
}
