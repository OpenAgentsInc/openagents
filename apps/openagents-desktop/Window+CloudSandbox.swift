// Firm reason: 6 (macOS URLSession HTTPS/SSE transport for dispatching Desktop turns to GCP Cloud Sandbox oa-boat via oa-api).
import AppKit
import Foundation

extension CoderWindow {
    /// Dispatches the active prompt on `r` directly to OpenAgents Cloud Sandbox (GCE VM via oa-boat + Neon Core)
    /// without starting any local worker on this Mac, streaming live CcEvent frames back into `r.page`.
    func sendCloudSandboxTask(_ r: CoderRun) {
        guard MacAccount.shared.loggedIn else {
            cd_failed(r.page, "Sign in to OpenAgents Cloud (OpenAgents ▸ Log In…) to run Cloud Sandbox turns.")
            wake()
            return
        }
        let shown = r === run
        let cwd = (shown ? folder : r.folder).path
        var out = [CChar](repeating: 0, count: 262144)
        let n = cd_task(r.page, r.session, cwd, &out, Int32(out.count))
        let rawTask = n > 0 ? String(cString: out) : ""
        let prompt: String
        if let data = rawTask.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let t = (obj["task"] as? String) ?? (obj["prompt"] as? String), !t.isEmpty {
            prompt = t
        } else {
            prompt = rawTask.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        guard !prompt.isEmpty else {
            cd_failed(r.page, "Could not read prompt for Cloud Sandbox turn.")
            wake()
            return
        }

        let convID = r.cloudSandboxConvID ?? ""
        let expectedRev: Int64 = convID.isEmpty ? 0 : max(1, r.cloudSandboxRevision)
        let model = "google/gemini-4-neon"
        let opID = "sub-\(UUID().uuidString.lowercased())"
        let submitObj: [String: Any] = [
            "schema": "openagents.chat.submit.v2alpha1",
            "operation_id": opID,
            "conversation_id": convID,
            "expected_revision": expectedRev,
            "project_id": NSNull(),
            "expected_project_revision": NSNull(),
            "legacy_thread_id": NSNull(),
            "legacy_thread_revision": NSNull(),
            "content": [["type": "text", "text": prompt]],
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: submitObj),
              let submitURL = URL(string: MacAccount.shared.auth.origin + "/api/chat/submit/v2alpha1?target=sandbox&size=small&model=\(model)") else {
            cd_failed(r.page, "Could not encode Cloud Sandbox request.")
            wake()
            return
        }

        r.cloudSandboxTask?.cancel()
        r.cloudSandboxTask = Task { [weak self, weak r] in
            guard let self, let r else { return }
            await self.runCloudSandboxTurn(r: r, submitURL: submitURL, body: body, fallbackRev: expectedRev)
        }
    }

    func stopCloudSandboxRun(_ r: CoderRun) {
        let turnID = r.cloudSandboxTurnID
        r.cloudSandboxTask?.cancel()
        r.cloudSandboxTask = nil
        r.cloudSandboxTurnID = nil
        r.stopRequestedAt = 0
        r.stopGen &+= 1
        if let turnID, !turnID.isEmpty,
           let url = URL(string: MacAccount.shared.auth.origin + "/api/chat/stop/v2alpha1") {
            let stopObj: [String: Any] = [
                "schema": "openagents.chat.stop.v2alpha1",
                "operation_id": "stop-\(UUID().uuidString.lowercased())",
                "turn_id": turnID,
            ]
            if let body = try? JSONSerialization.data(withJSONObject: stopObj) {
                Task.detached {
                    var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
                    req.httpMethod = "POST"
                    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
                    req.httpBody = body
                    if let authed = try? await MacAccount.shared.auth.authorize(req) {
                        _ = try? await URLSession.shared.data(for: authed)
                    }
                }
            }
        }
        if cd_running(r.page) != 0 {
            cd_line(r.page, "{\"kind\":\"ended\",\"ending\":\"stopped\",\"text\":\"\"}")
        }
        r.endTurn()
        if r === run { keep() } else { keep(r) }
        pump(r)
        wake()
    }

    private static func mwwJSON(_ obj: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: obj) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func emitSandboxLine(_ r: CoderRun, _ line: String) async {
        await MainActor.run {
            guard cd_running(r.page) != 0 else { return }
            cd_line(r.page, line)
            self.pump(r)
            self.wake()
        }
    }

    private func finishSandboxError(_ r: CoderRun, _ message: String) async {
        await MainActor.run {
            r.cloudSandboxTask = nil
            r.cloudSandboxTurnID = nil
            if cd_running(r.page) != 0 {
                cd_failed(r.page, message)
            }
            r.endTurn()
            if r === self.run { self.keep() } else { self.keep(r) }
            self.pump(r)
            self.wake()
        }
    }

    private func runCloudSandboxTurn(r: CoderRun, submitURL: URL, body: Data, fallbackRev: Int64) async {
        let auth = MacAccount.shared.auth
        var req = URLRequest(url: submitURL, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 30)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body

        let retConvID: String
        let retTurnID: String
        let retRev: Int64
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await URLSession.shared.data(for: authed)
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
            guard (200..<300).contains(code),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let cid = obj["conversation_id"] as? String, !cid.isEmpty,
                  let tid = obj["turn_id"] as? String, !tid.isEmpty else {
                let err = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
                await finishSandboxError(r, err ?? "Cloud Sandbox submit failed (HTTP \(code)).")
                return
            }
            retConvID = cid
            retTurnID = tid
            retRev = (obj["conversation_revision"] as? NSNumber)?.int64Value ?? (fallbackRev + 1)
        } catch {
            if Task.isCancelled { return }
            await finishSandboxError(r, "Cloud Sandbox submit error: \(error.localizedDescription)")
            return
        }

        await MainActor.run {
            r.cloudSandboxConvID = retConvID
            r.cloudSandboxRevision = retRev
            r.cloudSandboxTurnID = retTurnID
            if let th = Self.mwwJSON(["kind": "thought", "text": "Dispatched to Google Cloud Sandbox (\(retTurnID.prefix(12)))…\n"]) {
                cd_line(r.page, th)
                self.wake()
            }
        }

        var cursor: Int64 = 0
        var fullReply = ""
        var toolArgsByID: [String: (name: String, args: String)] = [:]
        var finished = false
        var reconnects = 0

        while !finished && !Task.isCancelled && reconnects < 12 {
            guard let streamURL = URL(string: auth.origin + "/api/chat/v2/conversations/\(retConvID)/stream?after=\(cursor)") else {
                await finishSandboxError(r, "Invalid Cloud Sandbox stream URL.")
                return
            }
            var sreq = URLRequest(url: streamURL, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 180)
            sreq.httpMethod = "GET"
            sreq.setValue("text/event-stream", forHTTPHeaderField: "Accept")

            do {
                let authed = try await auth.authorize(sreq)
                let (bytes, resp) = try await URLSession.shared.bytes(for: authed)
                let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
                if code != 200 {
                    reconnects += 1
                    try? await Task.sleep(nanoseconds: 1_500_000_000)
                    continue
                }
                var pendingID: Int64 = 0
                var pendingData = ""
                for try await rawLine in bytes.lines {
                    if Task.isCancelled { return }
                    if rawLine.isEmpty {
                        if !pendingData.isEmpty {
                            if pendingID > cursor { cursor = pendingID }
                            if let evData = pendingData.data(using: .utf8),
                               let ev = try? JSONSerialization.jsonObject(with: evData) as? [String: Any],
                               (ev["turn_id"] as? String) == retTurnID {
                                let kind = (ev["type"] as? String) ?? ""
                                let text = (ev["text"] as? String) ?? (ev["delta"] as? String) ?? ""
                                switch kind {
                                case "thinking_delta":
                                    if !text.isEmpty, let line = Self.mwwJSON(["kind": "thought", "text": text]) {
                                        await emitSandboxLine(r, line)
                                    }
                                case "tool_call":
                                    let tid = (ev["tool_id"] as? String) ?? ""
                                    let tname = (ev["tool_name"] as? String) ?? "Tool"
                                    let targs = (ev["tool_args"] as? String) ?? ""
                                    if !tid.isEmpty { toolArgsByID[tid] = (tname, targs) }
                                    let desc = targs.isEmpty ? "Running \(tname)…\n" : "Running \(tname): \(targs)\n"
                                    if let line = Self.mwwJSON(["kind": "thought", "text": desc]) {
                                        await emitSandboxLine(r, line)
                                    }
                                case "tool_result":
                                    let tid = (ev["tool_id"] as? String) ?? ""
                                    let info = toolArgsByID[tid]
                                    let tname = (ev["tool_name"] as? String).flatMap({ $0.isEmpty ? nil : $0 }) ?? info?.name ?? "Tool"
                                    let targs = info?.args ?? ""
                                    let failed = (ev["failed"] as? Bool) ?? false
                                    let diff = (ev["diff"] as? String) ?? ""
                                    let combined = diff.isEmpty ? text : (text.isEmpty ? diff : text + "\n" + diff)
                                    let cmdLabel = targs.isEmpty ? tname : "\(tname) \(targs)"
                                    if let line = Self.mwwJSON([
                                        "kind": "command",
                                        "command": cmdLabel,
                                        "exit": failed ? 1 : 0,
                                        "timed_out": false,
                                        "output": combined,
                                    ]) {
                                        await emitSandboxLine(r, line)
                                    }
                                case "text_delta":
                                    if !text.isEmpty {
                                        fullReply.append(text)
                                        if let line = Self.mwwJSON(["kind": "reply", "text": text]) {
                                            await emitSandboxLine(r, line)
                                        }
                                    }
                                case "completed":
                                    finished = true
                                    let endLine = Self.mwwJSON(["kind": "ended", "ending": "finished", "text": fullReply]) ?? "{\"kind\":\"ended\",\"ending\":\"finished\",\"text\":\"\"}"
                                    await MainActor.run {
                                        r.cloudSandboxTask = nil
                                        r.cloudSandboxTurnID = nil
                                        cd_line(r.page, endLine)
                                        r.endTurn()
                                        if r === self.run { self.keep() } else { self.keep(r) }
                                        self.pump(r)
                                        self.wake()
                                    }
                                case "cancelled":
                                    finished = true
                                    await MainActor.run {
                                        r.cloudSandboxTask = nil
                                        r.cloudSandboxTurnID = nil
                                        cd_line(r.page, "{\"kind\":\"ended\",\"ending\":\"stopped\",\"text\":\"\"}")
                                        r.endTurn()
                                        if r === self.run { self.keep() } else { self.keep(r) }
                                        self.pump(r)
                                        self.wake()
                                    }
                                case "failed":
                                    finished = true
                                    await finishSandboxError(r, text.isEmpty ? "Cloud Sandbox turn failed." : text)
                                default:
                                    break
                                }
                            }
                            pendingData = ""
                            pendingID = 0
                            if finished { break }
                        }
                    } else if rawLine.hasPrefix("id:") {
                        let s = rawLine.dropFirst(3).trimmingCharacters(in: .whitespaces)
                        if let v = Int64(s) { pendingID = v }
                    } else if rawLine.hasPrefix("data:") {
                        let chunk = String(rawLine.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                        if pendingData.isEmpty { pendingData = chunk } else { pendingData += "\n" + chunk }
                    }
                }
                reconnects += 1
            } catch {
                if Task.isCancelled { return }
                reconnects += 1
                try? await Task.sleep(nanoseconds: 1_500_000_000)
            }
        }

        if !finished && !Task.isCancelled {
            await finishSandboxError(r, "Cloud Sandbox stream disconnected before completion.")
        }
    }
}
