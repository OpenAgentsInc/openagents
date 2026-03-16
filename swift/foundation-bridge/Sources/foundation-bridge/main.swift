import Foundation

@main
struct FoundationBridge {
    static func main() async {
        let port: UInt16
        if CommandLine.arguments.count > 1,
           let parsedPort = UInt16(CommandLine.arguments[1])
        {
            port = parsedPort
        } else {
            port = 11435
        }

        print("""
        Foundation Models HTTP Bridge
        ==============================
        Starting server on port \(port)...
        Endpoints:
          GET  /health
          POST /control/shutdown
          GET  /v1/models
          GET  /v1/adapters
          POST /v1/adapters/load
          DELETE /v1/adapters/{id}
          POST /v1/sessions
          GET  /v1/sessions/{id}
          GET  /v1/sessions/{id}/transcript
          POST /v1/sessions/{id}/adapter
          DELETE /v1/sessions/{id}/adapter
          POST /v1/sessions/{id}/responses
          POST /v1/sessions/{id}/responses/structured
          POST /v1/sessions/{id}/responses/stream
          POST /v1/sessions/{id}/reset
          DELETE /v1/sessions/{id}
          POST /v1/chat/completions

        Press Ctrl+C to stop.
        """)

        let chatHandler = ChatHandler()
        let server = HTTPServer(port: port, chatHandler: chatHandler)

        let (available, _, message) = await chatHandler.getAvailabilityStatus()
        if available {
            print("Foundation Models: Available")
        } else {
            print("Foundation Models: UNAVAILABLE - \(message)")
            print("Server will still run, but completions will return 503 until the model is available.")
        }

        do {
            try await server.start()
        } catch {
            print("Failed to start server: \(error)")
            exit(1)
        }
    }
}
