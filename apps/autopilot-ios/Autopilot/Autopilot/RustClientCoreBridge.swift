import Foundation
import Darwin

enum RustClientCoreBridge {
    struct ParsedKhalaFrame {
        let joinRef: String?
        let ref: String?
        let topic: String
        let event: String
        let payload: JSONValue
    }

    private typealias TransformFunction = @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
    private typealias FreeFunction = @convention(c) (UnsafeMutablePointer<CChar>?) -> Void

    private struct Symbols {
        let normalizeEmail: TransformFunction?
        let normalizeVerificationCode: TransformFunction?
        let normalizeMessageText: TransformFunction?
        let extractDesktopHandshakeAckID: TransformFunction?
        let parseKhalaFrame: TransformFunction?
        let freeString: FreeFunction?

        static func load() -> Symbols {
            let handle = dlopen(nil, RTLD_NOW)

            func loadSymbol<T>(_ name: String, as type: T.Type) -> T? {
                guard let handle,
                      let symbol = dlsym(handle, name) else {
                    return nil
                }

                return unsafeBitCast(symbol, to: type)
            }

            return Symbols(
                normalizeEmail: loadSymbol("oa_client_core_normalize_email", as: TransformFunction.self),
                normalizeVerificationCode: loadSymbol(
                    "oa_client_core_normalize_verification_code",
                    as: TransformFunction.self
                ),
                normalizeMessageText: loadSymbol("oa_client_core_normalize_message_text", as: TransformFunction.self),
                extractDesktopHandshakeAckID: loadSymbol(
                    "oa_client_core_extract_desktop_handshake_ack_id",
                    as: TransformFunction.self
                ),
                parseKhalaFrame: loadSymbol("oa_client_core_parse_khala_frame", as: TransformFunction.self),
                freeString: loadSymbol("oa_client_core_free_string", as: FreeFunction.self)
            )
        }
    }

    private static let symbols = Symbols.load()

    static var isAvailable: Bool {
        symbols.normalizeEmail != nil
            && symbols.normalizeVerificationCode != nil
            && symbols.normalizeMessageText != nil
            && symbols.extractDesktopHandshakeAckID != nil
            && symbols.parseKhalaFrame != nil
            && symbols.freeString != nil
    }

    static func normalizeEmail(_ value: String) -> String? {
        invoke(symbols.normalizeEmail, with: value)
    }

    static func normalizeVerificationCode(_ value: String) -> String? {
        invoke(symbols.normalizeVerificationCode, with: value)
    }

    static func normalizeMessageText(_ value: String) -> String? {
        invoke(symbols.normalizeMessageText, with: value)
    }

    static func extractDesktopHandshakeAckID(payloadJSON: String) -> String? {
        invoke(symbols.extractDesktopHandshakeAckID, with: payloadJSON)
    }

    static func parseKhalaFrame(raw: String) -> ParsedKhalaFrame? {
        guard let frameJSON = invoke(symbols.parseKhalaFrame, with: raw),
              let frameData = frameJSON.data(using: .utf8),
              let decoded = try? JSONDecoder().decode(DecodedKhalaFrame.self, from: frameData) else {
            return nil
        }

        return ParsedKhalaFrame(
            joinRef: decoded.joinRef,
            ref: decoded.ref,
            topic: decoded.topic,
            event: decoded.event,
            payload: decoded.payload
        )
    }

    private static func invoke(_ function: TransformFunction?, with input: String) -> String? {
        guard let function,
              let freeString = symbols.freeString else {
            return nil
        }

        return input.withCString { pointer in
            guard let rawOutput = function(pointer) else {
                return nil
            }

            defer {
                freeString(rawOutput)
            }

            return String(cString: rawOutput)
        }
    }

    private struct DecodedKhalaFrame: Decodable {
        let joinRef: String?
        let ref: String?
        let topic: String
        let event: String
        let payload: JSONValue

        enum CodingKeys: String, CodingKey {
            case joinRef = "join_ref"
            case ref = "reference"
            case topic
            case event
            case payload
        }
    }
}
