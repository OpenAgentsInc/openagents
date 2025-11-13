/**
 * Singleton WebSocket connection for tinyvex
 *
 * Ensures only one WebSocket connection exists across all runtimes,
 * preventing duplicate connections during hot-reload.
 */

import { useTinyvexWebSocket, type TinyvexWebSocketHandle } from "./useTinyvexWebSocket";
import { TINYVEX_WS_URL } from "@/config/acp";
import { useEffect, useRef } from "react";

let globalWsHandle: TinyvexWebSocketHandle | null = null;
let refCount = 0;

/**
 * Hook that returns a shared WebSocket connection
 *
 * Uses reference counting to ensure the connection stays alive
 * as long as at least one component is using it.
 */
export function useSharedTinyvexWebSocket(): TinyvexWebSocketHandle {
  const localWs = useTinyvexWebSocket({
    url: TINYVEX_WS_URL,
    autoConnect: false  // Don't auto-connect, we'll manage it
  });

  const isInitialized = useRef(false);

  useEffect(() => {
    if (isInitialized.current) return;
    isInitialized.current = true;

    refCount++;

    // First mount - create and connect
    if (refCount === 1) {
      globalWsHandle = localWs;
      globalWsHandle.connect();
      console.log("[tinyvex-singleton] First connection, refCount:", refCount);
    } else {
      console.log("[tinyvex-singleton] Reusing connection, refCount:", refCount);
    }

    return () => {
      refCount--;
      console.log("[tinyvex-singleton] Cleanup, refCount:", refCount);

      // Last unmount - disconnect
      if (refCount === 0 && globalWsHandle) {
        globalWsHandle.disconnect();
        globalWsHandle = null;
        console.log("[tinyvex-singleton] Disconnected");
      }
    };
  }, [localWs]);

  return globalWsHandle || localWs;
}
