/**
 * Connection Status Component
 *
 * Shows connection status while discovering or connecting to servers.
 */

interface ConnectionStatusProps {
  status: "checking" | "discovering";
  onRefresh?: () => void;
}

export function ConnectionStatus({ status, onRefresh }: ConnectionStatusProps) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-zinc-900 text-white">
      <div className="text-center">
        <div className="mb-6">
          <div className="mx-auto mb-4 h-12 w-12 animate-spin rounded-full border-4 border-zinc-700 border-t-blue-500" />
        </div>

        <div className="mb-2 text-xl font-semibold">
          {status === "checking" && "Initializing..."}
          {status === "discovering" && "Discovering Servers"}
        </div>

        <div className="text-sm text-zinc-400">
          {status === "checking" && "Checking platform..."}
          {status === "discovering" && "Scanning local network for OpenAgents servers..."}
        </div>

        {onRefresh && status === "discovering" && (
          <button
            onClick={onRefresh}
            className="mt-6 rounded bg-zinc-800 px-4 py-2 text-sm hover:bg-zinc-700"
          >
            Scan Again
          </button>
        )}
      </div>
    </div>
  );
}
