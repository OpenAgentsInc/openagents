import Foundation

public enum WebSocketMessage {

    public struct ProtocolVersion {
        public static let current: Int = 1
        private init() {}
    }

    public struct Envelope: Codable {
        public let type: String
        public let data: Data

        public init(type: String, data: Data) {
            self.type = type
            self.data = data
        }

        public func decodedMessage<T: Codable>(as type: T.Type) throws -> T {
            return try JSONDecoder().decode(type, from: data)
        }

        public static func envelope<T: Codable>(for message: T, type: String) throws -> Envelope {
            let encodedData = try JSONEncoder().encode(message)
            return Envelope(type: type, data: encodedData)
        }

        public func jsonString(prettyPrinted: Bool = false) throws -> String {
            var options: JSONSerialization.WritingOptions = []
            if prettyPrinted {
                options = .prettyPrinted
            }
            let dict: [String: Any] = [
                "type": type,
                "data": try JSONSerialization.jsonObject(with: data, options: [])
            ]
            let jsonData = try JSONSerialization.data(withJSONObject: dict, options: options)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                throw EncodingError.invalidValue(dict, EncodingError.Context(codingPath: [], debugDescription: "Unable to convert JSON data to string"))
            }
            return jsonString
        }

        public static func from(jsonString: String) throws -> Envelope {
            guard let jsonData = jsonString.data(using: .utf8) else {
                throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "String to data conversion failed"))
            }
            let raw = try JSONSerialization.jsonObject(with: jsonData, options: [])
            guard let dict = raw as? [String: Any],
                  let type = dict["type"] as? String,
                  let dataObj = dict["data"] else {
                throw DecodingError.dataCorrupted(.init(codingPath: [], debugDescription: "Missing required keys"))
            }
            let data = try JSONSerialization.data(withJSONObject: dataObj, options: [])
            return Envelope(type: type, data: data)
        }
    }


    public struct Ping: Codable {
        public init() {}
    }

    public struct Pong: Codable {
        public init() {}
    }

    // MARK: - Threads list messages
    public struct ThreadsListRequest: Codable {
        public let topK: Int?
        public init(topK: Int? = nil) { self.topK = topK }
    }

    public struct ThreadsListResponse: Codable {
        public let items: [ThreadSummary]
        public init(items: [ThreadSummary]) { self.items = items }
    }
}

// Compatibility aliases for earlier code and client delegate APIs
public typealias BridgeMessages = WebSocketMessage
public typealias BridgeMessage = WebSocketMessage.Envelope
