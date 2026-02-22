import Foundation
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

enum WgpuiCodexRole: UInt8 {
    case user = 0
    case assistant = 1
    case reasoning = 2
    case tool = 3
    case system = 4
    case error = 5
}

enum WgpuiInputTarget: UInt8 {
    case none = 0
    case composer = 1
    case authEmail = 2
    case authCode = 3
}

// Direct symbol bindings to avoid runtime dlsym/dead-strip issues.
@_silgen_name("wgpui_ios_background_create")
private func wgpui_ios_background_create(
    _ layerPtr: UnsafeMutableRawPointer?,
    _ width: UInt32,
    _ height: UInt32,
    _ scale: Float
) -> UnsafeMutableRawPointer?

@_silgen_name("wgpui_ios_background_render")
private func wgpui_ios_background_render(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_resize")
private func wgpui_ios_background_resize(_ state: UnsafeMutableRawPointer?, _ width: UInt32, _ height: UInt32)

@_silgen_name("wgpui_ios_background_destroy")
private func wgpui_ios_background_destroy(_ state: UnsafeMutableRawPointer?)

@_silgen_name("wgpui_ios_background_handle_tap")
private func wgpui_ios_background_handle_tap(_ state: UnsafeMutableRawPointer?, _ x: Float, _ y: Float)

@_silgen_name("wgpui_ios_background_clear_codex_messages")
private func wgpui_ios_background_clear_codex_messages(_ state: UnsafeMutableRawPointer?)

@_silgen_name("wgpui_ios_background_push_codex_message")
private func wgpui_ios_background_push_codex_message(
    _ state: UnsafeMutableRawPointer?,
    _ role: UInt8,
    _ string: UnsafePointer<CChar>?,
    _ length: Int,
    _ streaming: Int32
)

@_silgen_name("wgpui_ios_background_set_codex_context")
private func wgpui_ios_background_set_codex_context(
    _ state: UnsafeMutableRawPointer?,
    _ threadPtr: UnsafePointer<CChar>?,
    _ threadLen: Int,
    _ turnPtr: UnsafePointer<CChar>?,
    _ turnLen: Int,
    _ modelPtr: UnsafePointer<CChar>?,
    _ modelLen: Int,
    _ reasoningPtr: UnsafePointer<CChar>?,
    _ reasoningLen: Int
)

@_silgen_name("wgpui_ios_background_set_empty_state")
private func wgpui_ios_background_set_empty_state(
    _ state: UnsafeMutableRawPointer?,
    _ titlePtr: UnsafePointer<CChar>?,
    _ titleLen: Int,
    _ detailPtr: UnsafePointer<CChar>?,
    _ detailLen: Int
)

@_silgen_name("wgpui_ios_background_set_auth_fields")
private func wgpui_ios_background_set_auth_fields(
    _ state: UnsafeMutableRawPointer?,
    _ emailPtr: UnsafePointer<CChar>?,
    _ emailLen: Int,
    _ codePtr: UnsafePointer<CChar>?,
    _ codeLen: Int,
    _ authStatusPtr: UnsafePointer<CChar>?,
    _ authStatusLen: Int
)

@_silgen_name("wgpui_ios_background_set_operator_status")
private func wgpui_ios_background_set_operator_status(
    _ state: UnsafeMutableRawPointer?,
    _ workerStatusPtr: UnsafePointer<CChar>?,
    _ workerStatusLen: Int,
    _ streamStatusPtr: UnsafePointer<CChar>?,
    _ streamStatusLen: Int,
    _ handshakeStatusPtr: UnsafePointer<CChar>?,
    _ handshakeStatusLen: Int,
    _ deviceStatusPtr: UnsafePointer<CChar>?,
    _ deviceStatusLen: Int,
    _ telemetryPtr: UnsafePointer<CChar>?,
    _ telemetryLen: Int,
    _ eventsPtr: UnsafePointer<CChar>?,
    _ eventsLen: Int,
    _ controlPtr: UnsafePointer<CChar>?,
    _ controlLen: Int
)

@_silgen_name("wgpui_ios_background_clear_mission_data")
private func wgpui_ios_background_clear_mission_data(_ state: UnsafeMutableRawPointer?)

@_silgen_name("wgpui_ios_background_push_mission_worker")
private func wgpui_ios_background_push_mission_worker(
    _ state: UnsafeMutableRawPointer?,
    _ workerIDPtr: UnsafePointer<CChar>?,
    _ workerIDLen: Int,
    _ statusPtr: UnsafePointer<CChar>?,
    _ statusLen: Int,
    _ heartbeatStatePtr: UnsafePointer<CChar>?,
    _ heartbeatStateLen: Int,
    _ latestSeq: Int64,
    _ lagEvents: Int64,
    _ reconnectStatePtr: UnsafePointer<CChar>?,
    _ reconnectStateLen: Int,
    _ lastEventAtPtr: UnsafePointer<CChar>?,
    _ lastEventAtLen: Int,
    _ runningTurns: UInt64,
    _ queuedRequests: UInt64,
    _ failedRequests: UInt64
)

@_silgen_name("wgpui_ios_background_push_mission_thread")
private func wgpui_ios_background_push_mission_thread(
    _ state: UnsafeMutableRawPointer?,
    _ workerIDPtr: UnsafePointer<CChar>?,
    _ workerIDLen: Int,
    _ threadIDPtr: UnsafePointer<CChar>?,
    _ threadIDLen: Int,
    _ activeTurnIDPtr: UnsafePointer<CChar>?,
    _ activeTurnIDLen: Int,
    _ lastSummaryPtr: UnsafePointer<CChar>?,
    _ lastSummaryLen: Int,
    _ lastEventAtPtr: UnsafePointer<CChar>?,
    _ lastEventAtLen: Int,
    _ freshnessSeq: Int64,
    _ unreadCount: UInt64,
    _ muted: Int32
)

@_silgen_name("wgpui_ios_background_push_mission_timeline_entry")
private func wgpui_ios_background_push_mission_timeline_entry(
    _ state: UnsafeMutableRawPointer?,
    _ workerIDPtr: UnsafePointer<CChar>?,
    _ workerIDLen: Int,
    _ threadIDPtr: UnsafePointer<CChar>?,
    _ threadIDLen: Int,
    _ rolePtr: UnsafePointer<CChar>?,
    _ roleLen: Int,
    _ textPtr: UnsafePointer<CChar>?,
    _ textLen: Int,
    _ isStreaming: Int32,
    _ turnIDPtr: UnsafePointer<CChar>?,
    _ turnIDLen: Int,
    _ itemIDPtr: UnsafePointer<CChar>?,
    _ itemIDLen: Int,
    _ occurredAtPtr: UnsafePointer<CChar>?,
    _ occurredAtLen: Int
)

@_silgen_name("wgpui_ios_background_push_mission_event")
private func wgpui_ios_background_push_mission_event(
    _ state: UnsafeMutableRawPointer?,
    _ id: UInt64,
    _ topicPtr: UnsafePointer<CChar>?,
    _ topicLen: Int,
    _ seq: Int64,
    _ workerIDPtr: UnsafePointer<CChar>?,
    _ workerIDLen: Int,
    _ threadIDPtr: UnsafePointer<CChar>?,
    _ threadIDLen: Int,
    _ turnIDPtr: UnsafePointer<CChar>?,
    _ turnIDLen: Int,
    _ requestIDPtr: UnsafePointer<CChar>?,
    _ requestIDLen: Int,
    _ eventTypePtr: UnsafePointer<CChar>?,
    _ eventTypeLen: Int,
    _ methodPtr: UnsafePointer<CChar>?,
    _ methodLen: Int,
    _ summaryPtr: UnsafePointer<CChar>?,
    _ summaryLen: Int,
    _ severity: UInt8,
    _ occurredAtPtr: UnsafePointer<CChar>?,
    _ occurredAtLen: Int,
    _ payloadPtr: UnsafePointer<CChar>?,
    _ payloadLen: Int,
    _ resyncMarker: Int32
)

@_silgen_name("wgpui_ios_background_push_mission_request")
private func wgpui_ios_background_push_mission_request(
    _ state: UnsafeMutableRawPointer?,
    _ requestIDPtr: UnsafePointer<CChar>?,
    _ requestIDLen: Int,
    _ workerIDPtr: UnsafePointer<CChar>?,
    _ workerIDLen: Int,
    _ threadIDPtr: UnsafePointer<CChar>?,
    _ threadIDLen: Int,
    _ methodPtr: UnsafePointer<CChar>?,
    _ methodLen: Int,
    _ statePtr: UnsafePointer<CChar>?,
    _ stateLen: Int,
    _ occurredAtPtr: UnsafePointer<CChar>?,
    _ occurredAtLen: Int,
    _ errorCodePtr: UnsafePointer<CChar>?,
    _ errorCodeLen: Int,
    _ errorMessagePtr: UnsafePointer<CChar>?,
    _ errorMessageLen: Int,
    _ retryable: Int32,
    _ responsePtr: UnsafePointer<CChar>?,
    _ responseLen: Int
)

@_silgen_name("wgpui_ios_background_set_composer_text")
private func wgpui_ios_background_set_composer_text(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

@_silgen_name("wgpui_ios_background_set_auth_email")
private func wgpui_ios_background_set_auth_email(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

@_silgen_name("wgpui_ios_background_set_auth_code")
private func wgpui_ios_background_set_auth_code(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

@_silgen_name("wgpui_ios_background_composer_focused")
private func wgpui_ios_background_composer_focused(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_set_composer_focused")
private func wgpui_ios_background_set_composer_focused(_ state: UnsafeMutableRawPointer?, _ focused: Int32)

@_silgen_name("wgpui_ios_background_active_input_target")
private func wgpui_ios_background_active_input_target(_ state: UnsafeMutableRawPointer?) -> UInt8

@_silgen_name("wgpui_ios_background_set_active_input_target")
private func wgpui_ios_background_set_active_input_target(_ state: UnsafeMutableRawPointer?, _ target: UInt8)

@_silgen_name("wgpui_ios_background_consume_send_requested")
private func wgpui_ios_background_consume_send_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_new_thread_requested")
private func wgpui_ios_background_consume_new_thread_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_interrupt_requested")
private func wgpui_ios_background_consume_interrupt_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_model_cycle_requested")
private func wgpui_ios_background_consume_model_cycle_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_reasoning_cycle_requested")
private func wgpui_ios_background_consume_reasoning_cycle_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_send_code_requested")
private func wgpui_ios_background_consume_send_code_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_verify_code_requested")
private func wgpui_ios_background_consume_verify_code_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_sign_out_requested")
private func wgpui_ios_background_consume_sign_out_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_refresh_workers_requested")
private func wgpui_ios_background_consume_refresh_workers_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_connect_stream_requested")
private func wgpui_ios_background_consume_connect_stream_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_disconnect_stream_requested")
private func wgpui_ios_background_consume_disconnect_stream_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_send_handshake_requested")
private func wgpui_ios_background_consume_send_handshake_requested(_ state: UnsafeMutableRawPointer?) -> Int32

// Backward-compatible alias symbols.
@_silgen_name("wgpui_ios_background_login_submit_requested")
private func wgpui_ios_background_login_submit_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_consume_submit_requested")
private func wgpui_ios_background_consume_submit_requested(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_email_focused")
private func wgpui_ios_background_email_focused(_ state: UnsafeMutableRawPointer?) -> Int32

@_silgen_name("wgpui_ios_background_set_email_focused")
private func wgpui_ios_background_set_email_focused(_ state: UnsafeMutableRawPointer?, _ focused: Int32)

@_silgen_name("wgpui_ios_background_set_login_email")
private func wgpui_ios_background_set_login_email(
    _ state: UnsafeMutableRawPointer?,
    _ string: UnsafePointer<CChar>?,
    _ length: Int
)

/// Bridge to WGPUI iOS Codex renderer.
enum WgpuiBackgroundBridge {
    static func logAvailability() -> Bool {
        let msg = "[WGPUI] bridge symbols linked directly (no runtime dlsym)"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return true
    }

    static var isAvailable: Bool { true }

    static func create(layerPtr: UnsafeMutableRawPointer, width: UInt32, height: UInt32, scale: Float) -> UnsafeMutableRawPointer? {
        let result = wgpui_ios_background_create(layerPtr, width, height, scale)
        let msg = "[WGPUI] create width=\(width) height=\(height) scale=\(scale) -> \(result != nil ? "OK" : "NULL (Rust failed)")"
        os_log("%{public}@", log: wgpuiLog, type: .default, msg)
        print(msg)
        return result
    }

    static func render(state: UnsafeMutableRawPointer?, logFirstFrame: Bool = false) -> Bool {
        guard let state else { return false }
        let ok = wgpui_ios_background_render(state) != 0
        if logFirstFrame {
            let msg = "[WGPUI] first render -> \(ok ? "OK" : "FAIL")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
        return ok
    }

    static func resize(state: UnsafeMutableRawPointer?, width: UInt32, height: UInt32) {
        wgpui_ios_background_resize(state, width, height)
    }

    static func destroy(state: UnsafeMutableRawPointer?) {
        print("[WGPUI] destroy state=\(state != nil ? "non-nil" : "nil")")
        wgpui_ios_background_destroy(state)
    }

    static func handleTap(state: UnsafeMutableRawPointer?, x: Float, y: Float) {
        wgpui_ios_background_handle_tap(state, x, y)
    }

    static func clearCodexMessages(state: UnsafeMutableRawPointer?) {
        wgpui_ios_background_clear_codex_messages(state)
    }

    static func pushCodexMessage(
        state: UnsafeMutableRawPointer?,
        role: WgpuiCodexRole,
        text: String,
        streaming: Bool
    ) {
        let utf8 = Array(text.utf8)
        utf8.withUnsafeBufferPointer { buf in
            buf.baseAddress?.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                wgpui_ios_background_push_codex_message(state, role.rawValue, ptr, buf.count, streaming ? 1 : 0)
            }
        }
    }

    static func setCodexContext(
        state: UnsafeMutableRawPointer?,
        thread: String,
        turn: String,
        model: String,
        reasoning: String
    ) {
        withCStringBytes(thread) { threadPtr, threadLen in
            withCStringBytes(turn) { turnPtr, turnLen in
                withCStringBytes(model) { modelPtr, modelLen in
                    withCStringBytes(reasoning) { reasoningPtr, reasoningLen in
                        wgpui_ios_background_set_codex_context(
                            state,
                            threadPtr,
                            threadLen,
                            turnPtr,
                            turnLen,
                            modelPtr,
                            modelLen,
                            reasoningPtr,
                            reasoningLen
                        )
                    }
                }
            }
        }
    }

    static func setComposerText(state: UnsafeMutableRawPointer?, _ text: String) {
        withCStringBytes(text) { ptr, len in
            wgpui_ios_background_set_composer_text(state, ptr, len)
        }
    }

    static func setEmptyState(state: UnsafeMutableRawPointer?, title: String, detail: String) {
        withCStringBytes(title) { titlePtr, titleLen in
            withCStringBytes(detail) { detailPtr, detailLen in
                wgpui_ios_background_set_empty_state(state, titlePtr, titleLen, detailPtr, detailLen)
            }
        }
    }

    static func setAuthFields(
        state: UnsafeMutableRawPointer?,
        email: String,
        code: String,
        authStatus: String
    ) {
        withCStringBytes(email) { emailPtr, emailLen in
            withCStringBytes(code) { codePtr, codeLen in
                withCStringBytes(authStatus) { authStatusPtr, authStatusLen in
                    wgpui_ios_background_set_auth_fields(
                        state,
                        emailPtr,
                        emailLen,
                        codePtr,
                        codeLen,
                        authStatusPtr,
                        authStatusLen
                    )
                }
            }
        }
    }

    static func setOperatorStatus(
        state: UnsafeMutableRawPointer?,
        workerStatus: String,
        streamStatus: String,
        handshakeStatus: String,
        deviceStatus: String,
        telemetry: String,
        events: String,
        control: String
    ) {
        withCStringBytes(workerStatus) { workerPtr, workerLen in
            withCStringBytes(streamStatus) { streamPtr, streamLen in
                withCStringBytes(handshakeStatus) { handshakePtr, handshakeLen in
                    withCStringBytes(deviceStatus) { devicePtr, deviceLen in
                        withCStringBytes(telemetry) { telemetryPtr, telemetryLen in
                            withCStringBytes(events) { eventsPtr, eventsLen in
                                withCStringBytes(control) { controlPtr, controlLen in
                                    wgpui_ios_background_set_operator_status(
                                        state,
                                        workerPtr,
                                        workerLen,
                                        streamPtr,
                                        streamLen,
                                        handshakePtr,
                                        handshakeLen,
                                        devicePtr,
                                        deviceLen,
                                        telemetryPtr,
                                        telemetryLen,
                                        eventsPtr,
                                        eventsLen,
                                        controlPtr,
                                        controlLen
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    static func clearMissionData(state: UnsafeMutableRawPointer?) {
        wgpui_ios_background_clear_mission_data(state)
    }

    static func pushMissionWorker(
        state: UnsafeMutableRawPointer?,
        workerID: String,
        status: String,
        heartbeatState: String?,
        latestSeq: Int?,
        lagEvents: Int?,
        reconnectState: String?,
        lastEventAt: String?,
        runningTurns: UInt64,
        queuedRequests: UInt64,
        failedRequests: UInt64
    ) {
        withCStringBytes(workerID) { workerIDPtr, workerIDLen in
            withCStringBytes(status) { statusPtr, statusLen in
                withCStringBytes(heartbeatState ?? "") { heartbeatPtr, heartbeatLen in
                    withCStringBytes(reconnectState ?? "") { reconnectPtr, reconnectLen in
                        withCStringBytes(lastEventAt ?? "") { lastEventPtr, lastEventLen in
                            wgpui_ios_background_push_mission_worker(
                                state,
                                workerIDPtr,
                                workerIDLen,
                                statusPtr,
                                statusLen,
                                heartbeatPtr,
                                heartbeatLen,
                                Int64(latestSeq ?? -1),
                                Int64(lagEvents ?? -1),
                                reconnectPtr,
                                reconnectLen,
                                lastEventPtr,
                                lastEventLen,
                                runningTurns,
                                queuedRequests,
                                failedRequests
                            )
                        }
                    }
                }
            }
        }
    }

    static func pushMissionThread(
        state: UnsafeMutableRawPointer?,
        workerID: String,
        threadID: String,
        activeTurnID: String?,
        lastSummary: String,
        lastEventAt: String?,
        freshnessSeq: Int?,
        unreadCount: UInt64,
        muted: Bool
    ) {
        withCStringBytes(workerID) { workerIDPtr, workerIDLen in
            withCStringBytes(threadID) { threadIDPtr, threadIDLen in
                withCStringBytes(activeTurnID ?? "") { activeTurnPtr, activeTurnLen in
                    withCStringBytes(lastSummary) { summaryPtr, summaryLen in
                        withCStringBytes(lastEventAt ?? "") { lastEventPtr, lastEventLen in
                            wgpui_ios_background_push_mission_thread(
                                state,
                                workerIDPtr,
                                workerIDLen,
                                threadIDPtr,
                                threadIDLen,
                                activeTurnPtr,
                                activeTurnLen,
                                summaryPtr,
                                summaryLen,
                                lastEventPtr,
                                lastEventLen,
                                Int64(freshnessSeq ?? -1),
                                unreadCount,
                                muted ? 1 : 0
                            )
                        }
                    }
                }
            }
        }
    }

    static func pushMissionTimelineEntry(
        state: UnsafeMutableRawPointer?,
        workerID: String,
        threadID: String,
        role: String,
        text: String,
        isStreaming: Bool,
        turnID: String?,
        itemID: String?,
        occurredAt: String?
    ) {
        withCStringBytes(workerID) { workerIDPtr, workerIDLen in
            withCStringBytes(threadID) { threadIDPtr, threadIDLen in
                withCStringBytes(role) { rolePtr, roleLen in
                    withCStringBytes(text) { textPtr, textLen in
                        withCStringBytes(turnID ?? "") { turnPtr, turnLen in
                            withCStringBytes(itemID ?? "") { itemPtr, itemLen in
                                withCStringBytes(occurredAt ?? "") { occurredPtr, occurredLen in
                                    wgpui_ios_background_push_mission_timeline_entry(
                                        state,
                                        workerIDPtr,
                                        workerIDLen,
                                        threadIDPtr,
                                        threadIDLen,
                                        rolePtr,
                                        roleLen,
                                        textPtr,
                                        textLen,
                                        isStreaming ? 1 : 0,
                                        turnPtr,
                                        turnLen,
                                        itemPtr,
                                        itemLen,
                                        occurredPtr,
                                        occurredLen
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    static func pushMissionEvent(
        state: UnsafeMutableRawPointer?,
        id: UInt64,
        topic: String,
        seq: Int?,
        workerID: String?,
        threadID: String?,
        turnID: String?,
        requestID: String?,
        eventType: String?,
        method: String?,
        summary: String,
        severity: UInt8,
        occurredAt: String?,
        payloadJSON: String,
        resyncMarker: Bool
    ) {
        withCStringBytes(topic) { topicPtr, topicLen in
            withCStringBytes(workerID ?? "") { workerPtr, workerLen in
                withCStringBytes(threadID ?? "") { threadPtr, threadLen in
                    withCStringBytes(turnID ?? "") { turnPtr, turnLen in
                        withCStringBytes(requestID ?? "") { requestPtr, requestLen in
                            withCStringBytes(eventType ?? "") { eventTypePtr, eventTypeLen in
                                withCStringBytes(method ?? "") { methodPtr, methodLen in
                                    withCStringBytes(summary) { summaryPtr, summaryLen in
                                        withCStringBytes(occurredAt ?? "") { occurredPtr, occurredLen in
                                            withCStringBytes(payloadJSON) { payloadPtr, payloadLen in
                                                wgpui_ios_background_push_mission_event(
                                                    state,
                                                    id,
                                                    topicPtr,
                                                    topicLen,
                                                    Int64(seq ?? -1),
                                                    workerPtr,
                                                    workerLen,
                                                    threadPtr,
                                                    threadLen,
                                                    turnPtr,
                                                    turnLen,
                                                    requestPtr,
                                                    requestLen,
                                                    eventTypePtr,
                                                    eventTypeLen,
                                                    methodPtr,
                                                    methodLen,
                                                    summaryPtr,
                                                    summaryLen,
                                                    severity,
                                                    occurredPtr,
                                                    occurredLen,
                                                    payloadPtr,
                                                    payloadLen,
                                                    resyncMarker ? 1 : 0
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    static func pushMissionRequest(
        state: UnsafeMutableRawPointer?,
        requestID: String,
        workerID: String,
        threadID: String?,
        method: String,
        requestState: String,
        occurredAt: String?,
        errorCode: String?,
        errorMessage: String?,
        retryable: Bool,
        responseJSON: String?
    ) {
        withCStringBytes(requestID) { requestPtr, requestLen in
            withCStringBytes(workerID) { workerPtr, workerLen in
                withCStringBytes(threadID ?? "") { threadPtr, threadLen in
                    withCStringBytes(method) { methodPtr, methodLen in
                        withCStringBytes(requestState) { statePtr, stateLen in
                            withCStringBytes(occurredAt ?? "") { occurredPtr, occurredLen in
                                withCStringBytes(errorCode ?? "") { codePtr, codeLen in
                                    withCStringBytes(errorMessage ?? "") { messagePtr, messageLen in
                                        withCStringBytes(responseJSON ?? "") { responsePtr, responseLen in
                                            wgpui_ios_background_push_mission_request(
                                                state,
                                                requestPtr,
                                                requestLen,
                                                workerPtr,
                                                workerLen,
                                                threadPtr,
                                                threadLen,
                                                methodPtr,
                                                methodLen,
                                                statePtr,
                                                stateLen,
                                                occurredPtr,
                                                occurredLen,
                                                codePtr,
                                                codeLen,
                                                messagePtr,
                                                messageLen,
                                                retryable ? 1 : 0,
                                                responsePtr,
                                                responseLen
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    static func composerFocused(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_composer_focused(state) != 0
    }

    static func setComposerFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        wgpui_ios_background_set_composer_focused(state, focused ? 1 : 0)
    }

    static func setAuthEmail(state: UnsafeMutableRawPointer?, _ text: String) {
        withCStringBytes(text) { ptr, len in
            wgpui_ios_background_set_auth_email(state, ptr, len)
        }
    }

    static func setAuthCode(state: UnsafeMutableRawPointer?, _ text: String) {
        withCStringBytes(text) { ptr, len in
            wgpui_ios_background_set_auth_code(state, ptr, len)
        }
    }

    static func activeInputTarget(state: UnsafeMutableRawPointer?) -> WgpuiInputTarget {
        WgpuiInputTarget(rawValue: wgpui_ios_background_active_input_target(state)) ?? .none
    }

    static func setActiveInputTarget(state: UnsafeMutableRawPointer?, _ target: WgpuiInputTarget) {
        wgpui_ios_background_set_active_input_target(state, target.rawValue)
    }

    static func consumeSendRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_send_requested(state) != 0
    }

    static func consumeNewThreadRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_new_thread_requested(state) != 0
    }

    static func consumeInterruptRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_interrupt_requested(state) != 0
    }

    static func consumeModelCycleRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_model_cycle_requested(state) != 0
    }

    static func consumeReasoningCycleRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_reasoning_cycle_requested(state) != 0
    }

    static func consumeSendCodeRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_send_code_requested(state) != 0
    }

    static func consumeVerifyCodeRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_verify_code_requested(state) != 0
    }

    static func consumeSignOutRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_sign_out_requested(state) != 0
    }

    static func consumeRefreshWorkersRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_refresh_workers_requested(state) != 0
    }

    static func consumeConnectStreamRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_connect_stream_requested(state) != 0
    }

    static func consumeDisconnectStreamRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_disconnect_stream_requested(state) != 0
    }

    static func consumeSendHandshakeRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_send_handshake_requested(state) != 0
    }

    // Compatibility wrappers (legacy login naming).
    static func loginSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_login_submit_requested(state) != 0
    }

    static func consumeSubmitRequested(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_consume_submit_requested(state) != 0
    }

    static func emailFocused(state: UnsafeMutableRawPointer?) -> Bool {
        wgpui_ios_background_email_focused(state) != 0
    }

    static func setEmailFocused(state: UnsafeMutableRawPointer?, focused: Bool) {
        wgpui_ios_background_set_email_focused(state, focused ? 1 : 0)
    }

    static func setLoginEmail(state: UnsafeMutableRawPointer?, _ string: String) {
        withCStringBytes(string) { ptr, len in
            wgpui_ios_background_set_login_email(state, ptr, len)
        }
    }

    private static func withCStringBytes(
        _ string: String,
        _ body: (_ ptr: UnsafePointer<CChar>?, _ len: Int) -> Void
    ) {
        let utf8 = Array(string.utf8)
        utf8.withUnsafeBufferPointer { buf in
            if let baseAddress = buf.baseAddress {
                baseAddress.withMemoryRebound(to: CChar.self, capacity: buf.count) { ptr in
                    body(ptr, buf.count)
                }
            } else {
                body(nil, 0)
            }
        }
    }
}
