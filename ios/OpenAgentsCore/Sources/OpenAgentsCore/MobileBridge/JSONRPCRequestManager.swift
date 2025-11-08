import Foundation

/// Manages in-flight JSON-RPC request completions keyed by request id.
/// Stores typed completion handlers and delivers results when responses arrive.
final class JSONRPCRequestManager {
    private var pending: [String: (Data) -> Void] = [:]

    /// Register a typed completion for a request id.
    func addExpectation<R: Decodable>(id: String, completion: @escaping (R?) -> Void) {
        pending[id] = { data in
            if let decoded = try? JSONDecoder().decode(R.self, from: data) {
                completion(decoded)
            } else {
                completion(nil)
            }
        }
    }

    /// Remove a pending expectation (e.g., if send failed).
    func remove(id: String) {
        _ = pending.removeValue(forKey: id)
    }

    /// Deliver a JSON result object to the pending handler for the given id.
    /// Returns true if a handler was found and invoked.
    func fulfill(id: String, withJsonResult resultObject: Any) -> Bool {
        guard let handler = pending.removeValue(forKey: id) else { return false }
        if let data = try? JSONSerialization.data(withJSONObject: resultObject) {
            handler(data)
            return true
        }
        return false
    }
}

