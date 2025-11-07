#if os(macOS)
import Foundation

/// JSON-RPC 2.0 router for dispatching method calls to registered handlers.
///
/// Responsibilities:
/// - Parse incoming JSON-RPC requests from text
/// - Extract method, id, and params
/// - Route to registered handlers
/// - Provide helper methods for sending responses and errors
///
/// Not thread-safe - caller must ensure thread safety (e.g., via DispatchQueue).
public class JsonRpcRouter {
    // MARK: - Types

    /// Handler closure type
    /// - Parameters:
    ///   - id: The JSON-RPC request ID
    ///   - params: The request parameters as a dictionary (nil if no params)
    ///   - rawDict: The full raw request dictionary for custom parsing
    public typealias Handler = (_ id: JSONRPC.ID, _ params: [String: Any]?, _ rawDict: [String: Any]) async -> Void

    /// Result of parsing a JSON-RPC message
    public enum ParseResult {
        case request(method: String, id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any])
        case notification(method: String, params: [String: Any]?)
        case invalidJson
        case notJsonRpc
    }

    // MARK: - State

    private var handlers: [String: Handler] = [:]

    // MARK: - Initialization

    public init() {}

    // MARK: - Handler Registration

    /// Register a handler for a specific JSON-RPC method
    /// - Parameters:
    ///   - method: The JSON-RPC method name
    ///   - handler: The async closure to handle requests for this method
    public func register(method: String, handler: @escaping Handler) {
        handlers[method] = handler
    }

    /// Unregister a handler for a method
    /// - Parameter method: The JSON-RPC method name to unregister
    public func unregister(method: String) {
        handlers.removeValue(forKey: method)
    }

    /// Check if a method has a registered handler
    /// - Parameter method: The JSON-RPC method name
    /// - Returns: True if handler is registered
    public func hasHandler(for method: String) -> Bool {
        return handlers[method] != nil
    }

    // MARK: - Routing

    /// Parse JSON-RPC text and route to appropriate handler
    /// - Parameters:
    ///   - text: The raw JSON-RPC text
    ///   - onUnhandled: Optional closure called for unhandled methods
    /// - Returns: True if message was handled, false otherwise
    @discardableResult
    public func route(
        text: String,
        onUnhandled: ((String, JSONRPC.ID?) async -> Void)? = nil
    ) async -> Bool {
        let parseResult = parse(text: text)

        switch parseResult {
        case .request(let method, let id, let params, let rawDict):
            print("[JsonRpcRouter] Request: method=\(method) id=\(id.value)")
            if let handler = handlers[method] {
                await handler(id, params, rawDict)
                return true
            } else {
                print("[JsonRpcRouter] No handler for method: \(method)")
                await onUnhandled?(method, id)
                return false
            }

        case .notification(let method, _):
            print("[JsonRpcRouter] Notification: method=\(method)")
            // Notifications don't have handlers in this implementation
            return false

        case .invalidJson:
            print("[JsonRpcRouter] Invalid JSON")
            return false

        case .notJsonRpc:
            print("[JsonRpcRouter] Not a JSON-RPC 2.0 message")
            return false
        }
    }

    /// Parse JSON-RPC text into a ParseResult
    /// - Parameter text: The raw JSON-RPC text
    /// - Returns: ParseResult indicating the type of message
    public func parse(text: String) -> ParseResult {
        guard let data = text.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return .invalidJson
        }

        // Check for JSON-RPC 2.0
        guard (dict["jsonrpc"] as? String) == "2.0" else {
            return .notJsonRpc
        }

        // Extract method
        guard let method = dict["method"] as? String else {
            return .invalidJson
        }

        // Extract params (optional)
        let params = dict["params"] as? [String: Any]

        // Check if request (has id) or notification (no id)
        if let idAny = dict["id"] {
            let id = JsonRpcRouter.extractId(from: idAny)
            return .request(method: method, id: id, params: params, rawDict: dict)
        } else {
            return .notification(method: method, params: params)
        }
    }

    // MARK: - Helpers

    /// Extract JSONRPC.ID from various types (String, Int, etc.)
    /// - Parameter idAny: The raw id value
    /// - Returns: Normalized JSONRPC.ID
    public static func extractId(from idAny: Any) -> JSONRPC.ID {
        if let idNum = idAny as? Int {
            return JSONRPC.ID(String(idNum))
        } else if let idStr = idAny as? String {
            return JSONRPC.ID(idStr)
        } else {
            return JSONRPC.ID("1")
        }
    }

    /// Send a JSON-RPC success response
    /// - Parameters:
    ///   - id: The request ID
    ///   - result: The result to encode
    ///   - sendText: Closure to send the encoded text
    public static func sendResponse<T: Codable>(
        id: JSONRPC.ID,
        result: T,
        sendText: (String) -> Void
    ) {
        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: id, result: result)),
           let jtext = String(data: out, encoding: .utf8) {
            print("[JsonRpcRouter] Sending response for id=\(id.value)")
            sendText(jtext)
        } else {
            print("[JsonRpcRouter] Failed to encode response for id=\(id.value)")
        }
    }

    /// Send a JSON-RPC error response
    /// - Parameters:
    ///   - id: The request ID
    ///   - code: The error code
    ///   - message: The error message
    ///   - sendText: Closure to send the encoded text
    public static func sendError(
        id: JSONRPC.ID,
        code: Int,
        message: String,
        sendText: (String) -> Void
    ) {
        let error = JSONRPC.ErrorObject(code: code, message: message)
        let errorResponse = JSONRPC.ErrorResponse(id: id, error: error)

        if let out = try? JSONEncoder().encode(errorResponse),
           let jtext = String(data: out, encoding: .utf8) {
            print("[JsonRpcRouter] Sending error for id=\(id.value) code=\(code) message=\(message)")
            sendText(jtext)
        } else {
            print("[JsonRpcRouter] Failed to encode error for id=\(id.value)")
        }
    }

    /// Send a JSON-RPC notification
    /// - Parameters:
    ///   - method: The notification method
    ///   - params: The notification parameters
    ///   - sendText: Closure to send the encoded text
    public static func sendNotification<P: Codable>(
        method: String,
        params: P,
        sendText: (String) -> Void
    ) {
        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: method, params: params)),
           let jtext = String(data: out, encoding: .utf8) {
            print("[JsonRpcRouter] Sending notification method=\(method)")
            sendText(jtext)
        } else {
            print("[JsonRpcRouter] Failed to encode notification method=\(method)")
        }
    }
}
#endif
