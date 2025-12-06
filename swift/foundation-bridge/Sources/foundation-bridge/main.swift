import Foundation

/// Foundation Models HTTP Bridge
/// Usage: foundation-bridge [port]
/// Default port: 11435

@main
struct FoundationBridge {
    static func main() async {
        // Parse port from arguments
        let port: UInt16
        if CommandLine.arguments.count > 1,
           let parsedPort = UInt16(CommandLine.arguments[1]) {
            port = parsedPort
        } else {
            port = 11435
        }

        print("""
        Foundation Models HTTP Bridge
        ==============================
        Starting server on port \(port)...
        Endpoints:
          GET  /health              - Check server and model status
          GET  /v1/models           - List available models
          POST /v1/chat/completions - Chat completion (OpenAI-compatible)

        Press Ctrl+C to stop.
        """)

        // Create handlers
        let chatHandler = ChatHandler()
        let server = HTTPServer(port: port, chatHandler: chatHandler)

        // Check model availability on startup
        let (available, message) = await chatHandler.getAvailabilityStatus()
        if available {
            print("Foundation Models: Available")
        } else {
            print("Foundation Models: UNAVAILABLE - \(message)")
            print("Note: Server will still run but requests will return 503")
        }

        // Start server
        do {
            try await server.start()
        } catch {
            print("Failed to start server: \(error)")
            exit(1)
        }
    }
}
