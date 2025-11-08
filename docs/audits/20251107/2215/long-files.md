# Long Files (Focus Review List)

- DesktopWebSocketServer.swift (~1181 lines) — continue splitting: transport lifecycle vs JSON‑RPC routing vs domain handlers.
- MobileWebSocketClient.swift (~407 lines) — recent extraction of ReconnectPolicy/RequestManager helps; keep handlers small.
- OrchestrationTypes.swift / SessionTools.swift — verify single responsibility and keep variants isolated.
- CLIAgentProvider.swift — split provider discovery/launch IO from ACP translation.
- UI: SimplifiedMacOSView.swift / SimplifiedIOSView.swift — large views; consider extracting subviews if logic grows.

