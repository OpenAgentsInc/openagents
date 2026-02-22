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

    struct KhalaSessionEvent: Decodable {
        let seq: Int?
        let payload: JSONValue
    }

    struct KhalaSessionStep: Decodable {
        let kind: String
        let frame: String?
        let events: [KhalaSessionEvent]?
        let watermark: Int?
        let code: String?
        let message: String?
        let status: Int?
        let staleCursor: Bool?
        let unauthorized: Bool?
        let forbidden: Bool?

        enum CodingKeys: String, CodingKey {
            case kind
            case frame
            case events
            case watermark
            case code
            case message
            case status
            case staleCursor = "stale_cursor"
            case unauthorized
            case forbidden
        }
    }

    private static let expectedFFIContractVersion: UInt32 = 1

    private typealias TransformFunction = @convention(c) (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
    private typealias FreeFunction = @convention(c) (UnsafeMutablePointer<CChar>?) -> Void
    private typealias ContractVersionFunction = @convention(c) () -> UInt32
    private typealias KhalaSessionCreateFunction = @convention(c) (
        UnsafePointer<CChar>?,
        UnsafePointer<CChar>?,
        UInt64
    ) -> UnsafeMutableRawPointer?
    private typealias KhalaSessionStepFunction = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?
    private typealias KhalaSessionOnFrameFunction = @convention(c) (
        UnsafeMutableRawPointer?,
        UnsafePointer<CChar>?
    ) -> UnsafeMutablePointer<CChar>?
    private typealias KhalaSessionLatestWatermarkFunction = @convention(c) (UnsafeMutableRawPointer?) -> UInt64
    private typealias KhalaSessionFreeFunction = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias ControlCoordinatorCreateFunction = @convention(c) () -> UnsafeMutableRawPointer?
    private typealias ControlCoordinatorApplyFunction = @convention(c) (
        UnsafeMutableRawPointer?,
        UnsafePointer<CChar>?
    ) -> UnsafeMutablePointer<CChar>?
    private typealias ControlCoordinatorFreeFunction = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private struct Symbols {
        let normalizeEmail: TransformFunction?
        let normalizeVerificationCode: TransformFunction?
        let normalizeMessageText: TransformFunction?
        let extractDesktopHandshakeAckID: TransformFunction?
        let parseKhalaFrame: TransformFunction?
        let decodeControlReceipt: TransformFunction?
        let extractControlSuccessContext: TransformFunction?
        let khalaSessionCreate: KhalaSessionCreateFunction?
        let khalaSessionStart: KhalaSessionStepFunction?
        let khalaSessionOnFrame: KhalaSessionOnFrameFunction?
        let khalaSessionHeartbeat: KhalaSessionStepFunction?
        let khalaSessionLatestWatermark: KhalaSessionLatestWatermarkFunction?
        let khalaSessionFree: KhalaSessionFreeFunction?
        let controlCoordinatorCreate: ControlCoordinatorCreateFunction?
        let controlCoordinatorApply: ControlCoordinatorApplyFunction?
        let controlCoordinatorFree: ControlCoordinatorFreeFunction?
        let freeString: FreeFunction?
        let ffiContractVersion: ContractVersionFunction?

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
                decodeControlReceipt: loadSymbol(
                    "oa_client_core_decode_control_receipt",
                    as: TransformFunction.self
                ),
                extractControlSuccessContext: loadSymbol(
                    "oa_client_core_extract_control_success_context",
                    as: TransformFunction.self
                ),
                khalaSessionCreate: loadSymbol(
                    "oa_client_core_khala_session_create",
                    as: KhalaSessionCreateFunction.self
                ),
                khalaSessionStart: loadSymbol(
                    "oa_client_core_khala_session_start",
                    as: KhalaSessionStepFunction.self
                ),
                khalaSessionOnFrame: loadSymbol(
                    "oa_client_core_khala_session_on_frame",
                    as: KhalaSessionOnFrameFunction.self
                ),
                khalaSessionHeartbeat: loadSymbol(
                    "oa_client_core_khala_session_heartbeat",
                    as: KhalaSessionStepFunction.self
                ),
                khalaSessionLatestWatermark: loadSymbol(
                    "oa_client_core_khala_session_latest_watermark",
                    as: KhalaSessionLatestWatermarkFunction.self
                ),
                khalaSessionFree: loadSymbol(
                    "oa_client_core_khala_session_free",
                    as: KhalaSessionFreeFunction.self
                ),
                controlCoordinatorCreate: loadSymbol(
                    "oa_client_core_control_coordinator_create",
                    as: ControlCoordinatorCreateFunction.self
                ),
                controlCoordinatorApply: loadSymbol(
                    "oa_client_core_control_coordinator_apply",
                    as: ControlCoordinatorApplyFunction.self
                ),
                controlCoordinatorFree: loadSymbol(
                    "oa_client_core_control_coordinator_free",
                    as: ControlCoordinatorFreeFunction.self
                ),
                freeString: loadSymbol("oa_client_core_free_string", as: FreeFunction.self),
                ffiContractVersion: loadSymbol(
                    "oa_client_core_ffi_contract_version",
                    as: ContractVersionFunction.self
                )
            )
        }
    }

    private static let symbols = Symbols.load()

    static var expectedContractVersion: UInt32 {
        expectedFFIContractVersion
    }

    static var ffiContractVersion: UInt32? {
        symbols.ffiContractVersion?()
    }

    static var isContractVersionCompatible: Bool {
        guard let version = ffiContractVersion else {
            return false
        }

        return version == expectedFFIContractVersion
    }

    static var isAvailable: Bool {
        symbols.normalizeEmail != nil
            && symbols.normalizeVerificationCode != nil
            && symbols.normalizeMessageText != nil
            && symbols.extractDesktopHandshakeAckID != nil
            && symbols.parseKhalaFrame != nil
            && symbols.decodeControlReceipt != nil
            && symbols.extractControlSuccessContext != nil
            && symbols.khalaSessionCreate != nil
            && symbols.khalaSessionStart != nil
            && symbols.khalaSessionOnFrame != nil
            && symbols.khalaSessionHeartbeat != nil
            && symbols.khalaSessionLatestWatermark != nil
            && symbols.khalaSessionFree != nil
            && symbols.controlCoordinatorCreate != nil
            && symbols.controlCoordinatorApply != nil
            && symbols.controlCoordinatorFree != nil
            && symbols.freeString != nil
            && symbols.ffiContractVersion != nil
            && isContractVersionCompatible
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

    static func createKhalaSession(
        workerID: String,
        workerEventsTopic: String,
        resumeAfter: Int
    ) -> UnsafeMutableRawPointer? {
        guard let create = symbols.khalaSessionCreate else {
            return nil
        }

        return withTwoCStringInputs(workerID, workerEventsTopic) { workerPtr, topicPtr in
            create(workerPtr, topicPtr, UInt64(max(0, resumeAfter)))
        }
    }

    static func freeKhalaSession(_ session: UnsafeMutableRawPointer?) {
        symbols.khalaSessionFree?(session)
    }

    static func khalaSessionStart(_ session: UnsafeMutableRawPointer?) -> KhalaSessionStep? {
        invokeSessionStep(symbols.khalaSessionStart, session: session)
    }

    static func khalaSessionOnFrame(_ session: UnsafeMutableRawPointer?, raw: String) -> KhalaSessionStep? {
        guard let function = symbols.khalaSessionOnFrame,
              let freeString = symbols.freeString else {
            return nil
        }

        return raw.withCString { rawPtr in
            guard let rawStep = function(session, rawPtr) else {
                return nil
            }
            defer {
                freeString(rawStep)
            }
            guard let json = String(cString: rawStep).data(using: .utf8) else {
                return nil
            }
            return try? JSONDecoder().decode(KhalaSessionStep.self, from: json)
        }
    }

    static func khalaSessionHeartbeat(_ session: UnsafeMutableRawPointer?) -> String? {
        guard let step = invokeSessionStep(symbols.khalaSessionHeartbeat, session: session),
              step.kind == "outbound" else {
            return nil
        }
        return step.frame
    }

    static func khalaSessionLatestWatermark(_ session: UnsafeMutableRawPointer?) -> Int {
        Int(symbols.khalaSessionLatestWatermark?(session) ?? 0)
    }

    static func createControlCoordinator() -> UnsafeMutableRawPointer? {
        symbols.controlCoordinatorCreate?()
    }

    static func freeControlCoordinator(_ coordinator: UnsafeMutableRawPointer?) {
        symbols.controlCoordinatorFree?(coordinator)
    }

    static func applyControlCoordinator(
        _ coordinator: UnsafeMutableRawPointer?,
        commandJSON: String
    ) -> String? {
        guard let apply = symbols.controlCoordinatorApply,
              let freeString = symbols.freeString else {
            return nil
        }

        return commandJSON.withCString { commandPtr in
            guard let rawOutput = apply(coordinator, commandPtr) else {
                return nil
            }
            defer {
                freeString(rawOutput)
            }
            return String(cString: rawOutput)
        }
    }

    static func decodeControlReceipt(payloadJSON: String) -> String? {
        invoke(symbols.decodeControlReceipt, with: payloadJSON)
    }

    static func extractControlSuccessContext(inputJSON: String) -> String? {
        invoke(symbols.extractControlSuccessContext, with: inputJSON)
    }

    private static func invokeSessionStep(
        _ function: KhalaSessionStepFunction?,
        session: UnsafeMutableRawPointer?
    ) -> KhalaSessionStep? {
        guard let function,
              let freeString = symbols.freeString,
              let rawOutput = function(session) else {
            return nil
        }

        defer {
            freeString(rawOutput)
        }

        guard let jsonData = String(cString: rawOutput).data(using: .utf8) else {
            return nil
        }
        return try? JSONDecoder().decode(KhalaSessionStep.self, from: jsonData)
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

    private static func withTwoCStringInputs<T>(
        _ lhs: String,
        _ rhs: String,
        _ body: (_ lhs: UnsafePointer<CChar>?, _ rhs: UnsafePointer<CChar>?) -> T
    ) -> T {
        lhs.withCString { lhsPtr in
            rhs.withCString { rhsPtr in
                body(lhsPtr, rhsPtr)
            }
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
