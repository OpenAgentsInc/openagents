import Foundation

enum KhalaClient {
    static let baseURL = URL(string: "https://openagents.com/api/v1")!
    static let freeKeyURL = URL(string: "https://openagents.com/api/keys/free")!
    static let model = "openagents/khala"

    enum KhalaError: Error, LocalizedError, Equatable {
        case missingKey
        case unauthorized
        case quotaExceeded
        case http(Int, String)
        case decoding
        case transport(String)

        var errorDescription: String? {
            recoveryMessage
        }

        var recoveryTitle: String {
            switch self {
            case .quotaExceeded: return "Free quota reached"
            case .http(let code, _) where code >= 500: return "Temporary Khala error"
            case .transport: return "Connection interrupted"
            case .missingKey: return "Missing API key"
            case .unauthorized: return "Key rejected"
            case .http: return "Khala API error"
            case .decoding: return "Unexpected response"
            }
        }

        var recoveryMessage: String {
            switch self {
            case .quotaExceeded:
                return "Your free quota is out for now. Add credits or wait for the UTC reset before sending again."
            case .http(let code, _) where code >= 500:
                return "The server returned \(code). This is usually temporary, so the same message can be retried."
            case .transport(let message):
                return "The request did not reach Khala cleanly: \(message). Check the connection and retry."
            case .missingKey:
                return "Mint or paste a Khala key in Settings, then send the message again."
            case .unauthorized:
                return "Khala did not accept this API key. Open Settings to mint a free key or paste a valid one."
            case .http(let code, _):
                return "Khala returned HTTP \(code). Update the request or try again after checking Settings."
            case .decoding:
                return "Khala responded, but the app could not read the response body."
            }
        }

        var isRetryable: Bool {
            switch self {
            case .http(let code, _) where code >= 500: return true
            case .transport: return true
            default: return false
            }
        }
    }

    static func mintFreeKey(session: URLSession = .shared) async throws -> String {
        var request = URLRequest(url: freeKeyURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let credential = object["credential"] as? [String: Any],
                  let token = credential["token"] as? String,
                  !token.isEmpty
            else {
                throw KhalaError.decoding
            }
            return token
        } catch let error as KhalaError {
            throw error
        } catch {
            throw KhalaError.transport(error.localizedDescription)
        }
    }
}
