/**
 * Mobile Connection Handler
 *
 * Handles server discovery and connection management on mobile platforms.
 * On desktop, this component renders children immediately.
 * On mobile, it discovers servers and manages the WebSocket connection.
 */

import { useEffect, useState } from "react";
import {
  isMobile,
  autoDiscoverServer,
  discoverServers,
  getServerWebSocketUrl,
  saveLastServer,
  type ServerInfo,
} from "@/lib/mobileServerDiscovery";
import { setWebSocketUrl } from "@/lib/tinyvexWebSocketSingleton";
import { getDefaultTinyvexWsUrl } from "@/config/acp";
import { ServerSelection } from "./ServerSelection";
import { ConnectionStatus } from "./ConnectionStatus";

type ConnectionState =
  | { status: "checking" }
  | { status: "discovering" }
  | { status: "selecting"; servers: ServerInfo[] }
  | { status: "connected"; server: ServerInfo }
  | { status: "error"; message: string };

export function MobileConnectionHandler({ children }: { children: React.ReactNode }) {
  const [mobile, setMobile] = useState<boolean | null>(null);
  const [state, setState] = useState<ConnectionState>({ status: "checking" });

  useEffect(() => {
    async function init() {
      // Check if we're on mobile
      const isMobilePlatform = await isMobile();
      setMobile(isMobilePlatform);

      if (!isMobilePlatform) {
        // Desktop - get WebSocket URL from backend and set it
        const wsUrl = await getDefaultTinyvexWsUrl();
        setWebSocketUrl(wsUrl);

        // Extract port from URL for display (ws://host:port/ws)
        const portMatch = wsUrl.match(/:(\d+)\//);
        const port = portMatch ? parseInt(portMatch[1]) : 9100;

        setState({ status: "connected", server: { name: "localhost", host: "127.0.0.1", port, discoveredAt: Date.now() } });
        return;
      }

      // Mobile - discover server
      setState({ status: "discovering" });

      try {
        const server = await autoDiscoverServer();

        if (!server) {
          // No servers found - try manual discovery
          const allServers = await discoverServers();

          if (allServers.length === 0) {
            setState({
              status: "error",
              message: "No OpenAgents desktop servers found on the network. Please ensure your desktop app is running and on the same WiFi network.",
            });
            return;
          }

          // Multiple servers - show selection UI
          setState({ status: "selecting", servers: allServers });
          return;
        }

        // Server found - connect
        const wsUrl = getServerWebSocketUrl(server);
        setWebSocketUrl(wsUrl);
        setState({ status: "connected", server });
      } catch (error) {
        console.error("Failed to discover server:", error);
        setState({
          status: "error",
          message: `Failed to discover servers: ${error instanceof Error ? error.message : String(error)}`,
        });
      }
    }

    init();
  }, []);

  const handleServerSelect = (server: ServerInfo) => {
    const wsUrl = getServerWebSocketUrl(server);
    setWebSocketUrl(wsUrl);
    saveLastServer(server);
    setState({ status: "connected", server });
  };

  const handleRefresh = async () => {
    setState({ status: "discovering" });

    try {
      const servers = await discoverServers();

      if (servers.length === 0) {
        setState({
          status: "error",
          message: "No servers found. Please ensure your desktop app is running.",
        });
        return;
      }

      if (servers.length === 1) {
        handleServerSelect(servers[0]);
        return;
      }

      setState({ status: "selecting", servers });
    } catch (error) {
      console.error("Failed to refresh servers:", error);
      setState({
        status: "error",
        message: `Failed to scan for servers: ${error instanceof Error ? error.message : String(error)}`,
      });
    }
  };

  // Show loading while checking platform
  if (mobile === null) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-zinc-900 text-white">
        <div className="text-center">
          <div className="mb-4 text-lg">Starting OpenAgents...</div>
        </div>
      </div>
    );
  }

  // Desktop - render immediately
  if (!mobile) {
    return <>{children}</>;
  }

  // Mobile - handle connection states
  if (state.status === "checking" || state.status === "discovering") {
    return <ConnectionStatus status={state.status} onRefresh={handleRefresh} />;
  }

  if (state.status === "selecting") {
    return <ServerSelection servers={state.servers} onSelect={handleServerSelect} onRefresh={handleRefresh} />;
  }

  if (state.status === "error") {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-zinc-900 text-white">
        <div className="max-w-md text-center">
          <div className="mb-4 text-xl font-bold text-red-500">Connection Error</div>
          <div className="mb-6 text-zinc-300">{state.message}</div>
          <button
            onClick={handleRefresh}
            className="rounded bg-blue-600 px-6 py-3 font-semibold text-white hover:bg-blue-700"
          >
            Try Again
          </button>
        </div>
      </div>
    );
  }

  // Connected - render children
  return <>{children}</>;
}
