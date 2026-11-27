// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The Borderless Session Control Bus & Foreman control API (docs/desktop/2026-11-26-unfettered-harness-spec.md §3.4, #14067).
import AppKit

extension CoderWindow {
    // MARK: - Borderless Session Control Bus & Foreman Control Plane

    func handleForemanAsyncRequest(_ request: ForemanRequest, done: @escaping (ForemanResponse) -> Void) {
        // Unified /v1/sessions* routes run without Foreman/Worker tier walls (zero 403s; #14067).
        if request.path == "/v1/sessions" || request.path.hasPrefix("/v1/sessions/") {
            handleSessionBusRequest(request, done: done)
            return
        }
        if let denied = ForemanAPI.authorize(principal: request.principal, method: request.method, path: request.path) {
            foremanAudit.record(tier: foremanTier, method: request.method, path: request.path, status: denied.status, summary: "principal=\(request.principal.rawValue)", error: "operator credential required")
            done(denied)
            return
        }
        if request.method == "POST" && request.path == "/v1/foreman/wait" {
            var payload = request.payload
            payload["chat_ids"] = [Foreman.chatID]; payload["mode"] = "all"
            foremanWait(payload, includeForeman: true, done: done)
            return
        }
        if request.method == "POST" && (request.path == "/v1/wait" || request.path == "/wait") {
            foremanWait(request.payload, done: done)
            return
        }
        if request.path.hasPrefix("/v1/wallet/") {
            #if OA_WALLET
            guard FeatureFlags.isOn(.wallet) else {
                done(ForemanResponse(status: 403, body: ["error": "wallet feature flag is disabled"]))
                return
            }
            handleWalletRPC(request, done: done)
            #else
            done(ForemanResponse(status: 403, body: ["error": "wallet RPC is not included in this build"]))
            #endif
            return
        }
        if request.path == "/v1/gym" && (request.method == "GET" || request.method == "POST") {
            guard FeatureFlags.isOn(.gym) else {
                done(ForemanResponse(status: 403, body: ["error": "gym feature flag is disabled"]))
                return
            }
            if request.method == "GET" {
                done(ForemanResponse(status: 200, body: gymStateForProgram()))
                return
            }
            let body = (try? JSONSerialization.data(withJSONObject: request.payload)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
            handleGymRequest(body) { [weak self] state in
                let ok = (state["ok"] as? Bool) == true
                if let self {
                    self.foremanAudit.record(action: "gym", tier: self.foremanTier, chatID: nil, status: ok ? 200 : 400, detail: ["principal": request.principal.rawValue, "error": state["error"] ?? ""])
                }
                done(ForemanResponse(status: ok ? 200 : 400, body: state))
            }
            return
        }
        if request.path == "/v1/gym/lean" && (request.method == "GET" || request.method == "POST") {
            guard FeatureFlags.isOn(.gym) else {
                done(ForemanResponse(status: 403, body: ["error": "gym feature flag is disabled"]))
                return
            }
            if request.method == "GET" {
                done(ForemanResponse(status: 200, body: leanGymStateForProgram()))
                return
            }
            let body = (try? JSONSerialization.data(withJSONObject: request.payload)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
            handleLeanGymRequest(body) { [weak self] state in
                let ok = (state["ok"] as? Bool) == true
                if let self {
                    self.foremanAudit.record(action: "gym_lean", tier: self.foremanTier, chatID: nil, status: ok ? 200 : 400, detail: ["principal": request.principal.rawValue, "error": state["error"] ?? ""])
                }
                done(ForemanResponse(status: ok ? 200 : 400, body: state))
            }
            return
        }
        if request.path == "/v1/gym/gbench" && (request.method == "GET" || request.method == "POST") {
            guard FeatureFlags.isOn(.gym) else {
                done(ForemanResponse(status: 403, body: ["error": "gym feature flag is disabled"]))
                return
            }
            if request.method == "GET" {
                done(ForemanResponse(status: 200, body: gbenchGymStateForProgram()))
                return
            }
            let body = (try? JSONSerialization.data(withJSONObject: request.payload)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
            handleGBenchGymRequest(body) { [weak self] state in
                let ok = (state["ok"] as? Bool) == true
                if let self {
                    self.foremanAudit.record(action: "gym_gbench", tier: self.foremanTier, chatID: nil, status: ok ? 200 : 400, detail: ["principal": request.principal.rawValue, "error": state["error"] ?? ""])
                }
                done(ForemanResponse(status: ok ? 200 : 400, body: state))
            }
            return
        }
        if let response = handleForemanManagement(request) {
            done(response)
            return
        }
        let response = handleForemanRequest(request)
        done(response)
    }

    // MARK: - Unified Borderless Session Control Bus (/v1/sessions*, #14067)

    private func handleSessionBusRequest(_ request: ForemanRequest, done: @escaping (ForemanResponse) -> Void) {
        let method = request.method
        let path = request.path
        let payload = request.payload
        let query = request.query

        if (method == "GET" || method == "POST") && (path == "/v1/sessions" || path == "/v1/sessions/" || path == "/v1/sessions/list") {
            handleSessionsList(query: query, payload: payload, done: done)
            return
        }
        if method == "POST" && path == "/v1/sessions/read" {
            let sid = (payload["session_id"] as? String) ?? (payload["id"] as? String) ?? ""
            handleSessionsRead(sessionID: sid, payload: payload, query: query, done: done)
            return
        }
        if method == "POST" && path == "/v1/sessions/send" {
            let sid = (payload["session_id"] as? String) ?? (payload["id"] as? String) ?? ""
            handleSessionsSend(sessionID: sid, payload: payload, done: done)
            return
        }
        if method == "POST" && path == "/v1/sessions/spawn" {
            handleSessionsSpawn(payload: payload, done: done)
            return
        }
        if method == "POST" && path == "/v1/sessions/stop" {
            let sid = (payload["session_id"] as? String) ?? (payload["id"] as? String) ?? ""
            handleSessionsStop(sessionID: sid, payload: payload, done: done)
            return
        }
        if method == "POST" && path == "/v1/sessions/wait" {
            handleSessionsWait(payload: payload, done: done)
            return
        }

        // Path-parameterized routes: /v1/sessions/:id/{read,send,stop}
        let prefix = "/v1/sessions/"
        if path.hasPrefix(prefix) {
            let rest = String(path.dropFirst(prefix.count))
            if let slash = rest.lastIndex(of: "/") {
                let rawID = String(rest[..<slash])
                let action = String(rest[rest.index(after: slash)...])
                let sid = rawID.removingPercentEncoding ?? rawID
                if action == "read" && (method == "GET" || method == "POST") {
                    handleSessionsRead(sessionID: sid, payload: payload, query: query, done: done)
                    return
                }
                if action == "send" && method == "POST" {
                    handleSessionsSend(sessionID: sid, payload: payload, done: done)
                    return
                }
                if action == "stop" && method == "POST" {
                    handleSessionsStop(sessionID: sid, payload: payload, done: done)
                    return
                }
            }
        }
        done(ForemanResponse(status: 404, body: ["error": "unknown sessions endpoint: \(method) \(path)"]))
    }

    private func resolveDesktopChatID(from sessionID: String) -> String? {
        if sessionID.hasPrefix("local:chat:") {
            return String(sessionID.dropFirst("local:chat:".count))
        }
        if !sessionID.contains(":"), (sessionID == chat || store.meta(sessionID) != nil || runs[sessionID] != nil) {
            return sessionID
        }
        return nil
    }

    private func desktopSessionRecords() -> [[String: Any]] {
        store.orderedList.map { c in
            [
                "id": "local:chat:\(c.id)",
                "chat_id": c.id,
                "kind": "desktop",
                "harness": "openagents",
                "machine_id": "local",
                "machine_label": "This Mac",
                "title": c.title,
                "cwd": c.folder,
                "branch": "",
                "status": busy(c.id) ? "running" : "idle",
                "updated_ms": Int64(c.updated * 1000.0),
                "pid": Int(getpid()),
                "preview": "",
                "shown": chat == c.id,
                "commandable": true
            ]
        }
    }

    private func handleSessionsList(query: [String: String], payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        let kindFilter = ((query["kind"] ?? (payload["kind"] as? String) ?? "all").lowercased())
        let harnessFilter = ((query["harness"] ?? (payload["harness"] as? String) ?? "").lowercased())
        let machineFilter = ((query["machine"] ?? (payload["machine"] as? String) ?? "").lowercased())
        let limit = max(1, min(200, Int(query["limit"] ?? "") ?? (payload["limit"] as? Int) ?? 100))
        let allowExt = AgentSettings.allowLocalHarnesses
        let desktop = desktopSessionRecords()
        let cachedLocal = introspectLocalSnap
        let cachedRemote = introspectRemoteSnap
        let envHosts = snapshotEnvHostsForIntrospect()

        let finishWith: (IntrospectScanner.LocalScanResult, IntrospectScanner.RemoteScanResult) -> Void = { localRes, remoteRes in
            var all: [[String: Any]] = []
            all.append(contentsOf: desktop)
            all.append(contentsOf: localRes.tuiSessions.map { $0.asDictionary(allowExternal: allowExt) })
            all.append(contentsOf: localRes.harnessSessions.map { $0.asDictionary(allowExternal: allowExt) })
            all.append(contentsOf: remoteRes.remoteSessions.map { $0.asDictionary(allowExternal: allowExt) })

            let filtered = all.filter { item in
                if !kindFilter.isEmpty && kindFilter != "all" {
                    if ((item["kind"] as? String) ?? "").lowercased() != kindFilter { return false }
                }
                if !harnessFilter.isEmpty {
                    if ((item["harness"] as? String) ?? "").lowercased() != harnessFilter { return false }
                }
                if !machineFilter.isEmpty {
                    if ((item["machine_id"] as? String) ?? "").lowercased() != machineFilter { return false }
                }
                return true
            }
            let capped = Array(filtered.prefix(limit))
            done(ForemanResponse(status: 200, body: [
                "ok": true,
                "bus_mode": "unbounded",
                "allow_local_harnesses": allowExt,
                "count": capped.count,
                "sessions": capped,
                "machines": remoteRes.machines.map { $0.asDictionary() }
            ]))
        }

        if let local = cachedLocal, let remote = cachedRemote {
            finishWith(local, remote)
            return
        }
        IntrospectScanner.queue.async { [weak self] in
            let local = cachedLocal ?? IntrospectScanner.scanLocal()
            let remote = cachedRemote ?? IntrospectScanner.scanRemoteMachines(envHosts: envHosts)
            DispatchQueue.main.async {
                self?.introspectLocalSnap = local
                self?.introspectRemoteSnap = remote
                self?.introspectPathIndex = local.pathIndex
                finishWith(local, remote)
            }
        }
    }

    private func handleSessionsRead(sessionID: String, payload: [String: Any], query: [String: String], done: @escaping (ForemanResponse) -> Void) {
        let sid = sessionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sid.isEmpty else {
            done(ForemanResponse(status: 400, body: ["ok": false, "error": "session_id is required"]))
            return
        }
        let tail = max(1, min(200, Int(query["tail"] ?? "") ?? (payload["tail"] as? Int) ?? 40))
        let includeTools = (payload["include_tools"] as? Bool) ?? (query["include_tools"] == "1" || query["include_tools"] == "true")

        if let chatID = resolveDesktopChatID(from: sid) {
            var p = payload
            p["chat_id"] = chatID
            p["tail"] = min(50, tail)
            p["include_tools"] = includeTools
            var resp = foremanReadChat(p)
            var body = resp.body
            body["ok"] = (resp.status == 200)
            body["session_id"] = "local:chat:\(chatID)"
            body["kind"] = "desktop"
            if let turns = body["turns"] as? [[String: Any]] {
                let text = turns.enumerated().map { idx, t in
                    let u = (t["user"] as? String) ?? ""
                    let r = (t["reply"] as? String) ?? ""
                    return "[turn \(idx + 1)] user: \(u)\nassistant: \(r)"
                }.joined(separator: "\n\n")
                body["output"] = text
            }
            done(ForemanResponse(status: resp.status, body: body))
            return
        }

        let paths = introspectPathIndex
        let machines = introspectRemoteSnap?.machines ?? []
        IntrospectScanner.queue.async {
            let res = IntrospectScanner.captureOutput(uri: sid, pathIndex: paths, tailLines: tail, machines: machines)
            DispatchQueue.main.async {
                done(ForemanResponse(status: 200, body: [
                    "ok": true,
                    "session_id": sid,
                    "status": res.status,
                    "output": res.output
                ]))
            }
        }
    }

    private func handleSessionsSend(sessionID: String, payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        let sid = sessionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sid.isEmpty else {
            done(ForemanResponse(status: 400, body: ["ok": false, "error": "session_id is required"]))
            return
        }
        let prompt = (payload["prompt"] as? String) ?? (payload["message"] as? String) ?? (payload["text"] as? String) ?? (payload["keys"] as? String) ?? ""
        guard !prompt.isEmpty else {
            done(ForemanResponse(status: 400, body: ["ok": false, "error": "prompt is required"]))
            return
        }
        let mode = ((payload["mode"] as? String) ?? "send").lowercased()
        let submit = (payload["submit"] as? Bool) ?? true

        if let chatID = resolveDesktopChatID(from: sid) {
            var p = payload
            p["chat_id"] = chatID
            p["prompt"] = prompt
            if mode == "steer" || mode == "interrupt" { p["steer"] = true }
            let resp = foremanSendChat(p, attributePrefix: false)
            foremanAudit.record(action: "session_send", tier: foremanTier, chatID: chatID, status: resp.status, detail: ["session_id": sid, "mode": mode])
            done(resp)
            return
        }

        let allowExt = AgentSettings.allowLocalHarnesses
        let machines = introspectRemoteSnap?.machines ?? []
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.sendCommand(
                uri: sid,
                prompt: prompt,
                mode: mode,
                submit: submit,
                allowLocalHarnesses: allowExt,
                machines: machines
            )
            DispatchQueue.main.async {
                self?.foremanAudit.record(action: "session_send", tier: self?.foremanTier ?? .autonomous, chatID: nil, status: res.status, detail: ["session_id": sid, "mode": mode])
                done(ForemanResponse(status: res.status, body: res.body))
            }
        }
    }

    private func handleSessionsSpawn(payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        let prompt = (payload["prompt"] as? String) ?? (payload["command"] as? String) ?? ""
        let machine = ((payload["machine"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let harness = ((payload["harness"] as? String) ?? "openagents").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let cwd = ((payload["cwd"] as? String) ?? (payload["folder"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let effectiveCwd = cwd.isEmpty ? folder.path : cwd
        let title = ((payload["title"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)

        let isLocalMachine = machine.isEmpty || machine.lowercased() == "local" || machine.lowercased() == "this mac"
        let isDesktopHarness = harness.isEmpty || harness == "openagents" || harness == "desktop" || harness == "flawed" || harness == "inkling" || harness == "oa-worker"

        if isLocalMachine && isDesktopHarness {
            var p = payload
            p["prompt"] = prompt
            p["folder"] = effectiveCwd
            let resp = foremanCreateChat(p, attributePrefix: false)
            foremanAudit.record(action: "session_spawn", tier: foremanTier, chatID: resp.body["chat_id"] as? String, status: resp.status, detail: ["harness": "openagents", "cwd": effectiveCwd])
            done(resp)
            return
        }

        let allowExt = AgentSettings.allowLocalHarnesses
        let machines = introspectRemoteSnap?.machines ?? []
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.spawnExternal(
                machine: isLocalMachine ? "" : machine,
                harness: harness,
                cwd: effectiveCwd,
                prompt: prompt,
                title: title,
                allowLocalHarnesses: allowExt,
                machines: machines
            )
            DispatchQueue.main.async {
                self?.foremanAudit.record(action: "session_spawn", tier: self?.foremanTier ?? .autonomous, chatID: nil, status: res.status, detail: ["machine": machine, "harness": harness, "cwd": effectiveCwd])
                if res.status == 200 { self?.refreshIntrospectLocal() }
                done(ForemanResponse(status: res.status, body: res.body))
            }
        }
    }

    private func handleSessionsStop(sessionID: String, payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        let sid = sessionID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sid.isEmpty else {
            done(ForemanResponse(status: 400, body: ["ok": false, "error": "session_id is required"]))
            return
        }
        if let chatID = resolveDesktopChatID(from: sid) {
            let resp = foremanControlChat(["chat_id": chatID, "action": "stop"])
            var body = resp.body
            body["session_id"] = "local:chat:\(chatID)"
            foremanAudit.record(action: "session_stop", tier: foremanTier, chatID: chatID, status: resp.status, detail: ["session_id": sid])
            done(ForemanResponse(status: resp.status, body: body))
            return
        }
        let allowExt = AgentSettings.allowLocalHarnesses
        let machines = introspectRemoteSnap?.machines ?? []
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.stopSession(uri: sid, allowLocalHarnesses: allowExt, machines: machines)
            DispatchQueue.main.async {
                self?.foremanAudit.record(action: "session_stop", tier: self?.foremanTier ?? .autonomous, chatID: nil, status: res.status, detail: ["session_id": sid])
                done(ForemanResponse(status: res.status, body: res.body))
            }
        }
    }

    private func handleSessionsWait(payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        var mapped = payload
        if let sids = payload["session_ids"] as? [String], !sids.isEmpty {
            mapped["chat_ids"] = sids.compactMap { resolveDesktopChatID(from: $0) }
        } else if let sid = payload["session_id"] as? String, !sid.isEmpty, let cid = resolveDesktopChatID(from: sid) {
            mapped["chat_id"] = cid
        }
        foremanWait(mapped, done: done)
    }

    // MARK: - Foreman Operator API (docs/coder/foreman-control-api.md)

    /// Routes that manage Foreman itself: its description, configuration and lifecycle. Nil for any other route.
    /// ForemanAPI.authorize has already refused the operator-only ones to the Foreman worker.
    private func handleForemanManagement(_ request: ForemanRequest) -> ForemanResponse? {
        let payload = request.payload
        let raiseAllowed = UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey)
        func audited(_ action: String, _ chatID: String? = nil, _ detail: [String: Any] = [:], _ response: ForemanResponse) -> ForemanResponse {
            var d = detail; d["principal"] = request.principal.rawValue
            foremanAudit.record(action: "operator:" + action, tier: foremanTier, chatID: chatID, status: response.status, detail: d)
            return response
        }
        switch (request.method, request.path) {
        case ("GET", "/v1"), ("GET", "/v1/"):
            return ForemanResponse(status: 200, body: ForemanAPI.describe(principal: request.principal))
        case ("GET", "/v1/config"):
            return ForemanResponse(status: 200, body: [
                "ok": true,
                "api_version": ForemanAPI.version,
                "tier": foremanTier.wireName,
                "tier_level": foremanTier.rawValue,
                "tier_label": foremanTier.label,
                "operator_tier_raise": raiseAllowed,
                "workspace": folder.path,
                "audit_log": foremanAudit.fileURL.path,
                "control_file": ForemanControlDescriptor.defaultURL.path
            ])
        case ("POST", "/v1/tier"), ("PUT", "/v1/tier"), ("POST", "/tier"), ("PUT", "/tier"):
            let name = (payload["tier"] as? String) ?? (payload["tier"] as? Int).map(String.init) ?? ""
            guard let tier = ForemanTier.from(name: name) else {
                return ForemanResponse(status: 400, body: ["error": "tier must be observe, managed or autonomous"])
            }
            let previous = foremanTier
            if let denied = ForemanAPI.tierChange(from: previous, to: tier, raiseAllowed: raiseAllowed) {
                return audited("set_tier", nil, ["from": previous.wireName, "to": tier.wireName], denied)
            }
            setForemanTier(tier)
            return audited("set_tier", nil, ["from": previous.wireName, "to": tier.wireName], ForemanResponse(status: 200, body: [
                "ok": true, "tier": tier.wireName, "tier_level": tier.rawValue, "tier_label": tier.label, "previous": previous.wireName
            ]))
        case ("GET", "/v1/foreman"):
            return ForemanResponse(status: 200, body: foremanState())
        case ("POST", "/v1/foreman/open"):
            openForeman()
            guard isForemanShown else {
                return ForemanResponse(status: 409, body: ["error": "Coder could not switch to the Foreman chat"])
            }
            return ForemanResponse(status: 200, body: foremanState())
        case ("POST", "/v1/foreman/prompt"):
            let prompt = ((payload["prompt"] as? String) ?? (payload["message"] as? String) ?? "")
            guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                return ForemanResponse(status: 400, body: ["error": "prompt is required"])
            }
            let mode = ((payload["mode"] as? String) ?? "").lowercased()
            let steer = ((payload["steer"] as? Bool) ?? false) || mode == "steer" || mode == "interrupt"
            if busy(Foreman.chatID) && !steer {
                return ForemanResponse(status: 409, body: ["error": "Foreman is replying. Wait for it (POST /v1/foreman/wait), stop it, or pass steer=true."])
            }
            openForeman()
            guard isForemanShown else {
                return ForemanResponse(status: 409, body: ["error": "Coder could not switch to the Foreman chat"])
            }
            if cd_running(page) != 0 { stopRun(run) }
            guard cd_set_text(page, "[From the operator API]\n" + prompt) != 0, cd_submit(page) != 0 else {
                return audited("prompt", Foreman.chatID, ["steer": steer], ForemanResponse(status: 500, body: ["error": "Coder could not send the message to Foreman"]))
            }
            wake()
            return audited("prompt", Foreman.chatID, ["steer": steer, "bytes": prompt.utf8.count], ForemanResponse(status: 200, body: [
                "ok": true, "chat_id": Foreman.chatID, "status": "running", "event_cursor": foremanEvents.latestID
            ]))
        case ("POST", "/v1/foreman/stop"):
            var stopped = false
            if chat == Foreman.chatID && cd_running(page) != 0 {
                stopRun(run); stopped = true
            } else if let back = runs.removeValue(forKey: Foreman.chatID) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page); stopped = true
            }
            if stopped {
                _ = foremanEvents.emit(type: "chat.stopped", chatID: Foreman.chatID, data: ["title": Foreman.title])
                cd_chats(page, chatLines)
                syncForemanUI()
                wake()
            }
            return audited("stop", Foreman.chatID, [:], ForemanResponse(status: 200, body: ["ok": true, "chat_id": Foreman.chatID, "status": stopped ? "stopped" : "already_idle"]))
        case ("POST", "/v1/foreman/reset"):
            resetForeman()
            _ = foremanEvents.emit(type: "foreman.reset", chatID: Foreman.chatID, data: ["title": Foreman.title])
            return audited("reset", Foreman.chatID, [:], ForemanResponse(status: 200, body: foremanState()))
        default:
            return nil
        }
    }

    /// The Foreman chat as the operator API reports it.
    private func foremanState() -> [String: Any] {
        let managed = store.orderedList.filter { $0.managedBy == Foreman.managedTag }
        return [
            "ok": true,
            "bus_mode": "unbounded",
            "chat_id": Foreman.chatID,
            "shown": chat == Foreman.chatID,
            "busy": busy(Foreman.chatID),
            "status": busy(Foreman.chatID) ? "running" : "idle",
            "has_history": store.text(Foreman.chatID) != nil,
            "folder": store.folder(Foreman.chatID) ?? folder.path,
            "tier": foremanTier.wireName,
            "tier_level": foremanTier.rawValue,
            "operator_tier_raise": UserDefaults.standard.bool(forKey: ForemanAPI.operatorTierRaiseKey),
            "managed_chats": managed.map { $0.id },
            "managed_running": managed.filter { busy($0.id) }.map { $0.id },
            "event_cursor": foremanEvents.latestID
        ]
    }

    private func handleForemanRequest(_ request: ForemanRequest) -> ForemanResponse {
        let method = request.method
        let path = request.path
        let payload = request.payload
        let query = request.query
        switch (method, path) {
        case ("GET", "/v1/status"), ("GET", "/health"):
            let managedRunning = store.orderedList.filter { $0.managedBy == Foreman.managedTag && busy($0.id) }.count
            return ForemanResponse(status: 200, body: [
                "ok": true,
                "tier": foremanTier.wireName,
                "tier_level": foremanTier.rawValue,
                "tier_label": foremanTier.label,
                "workspace": folder.path,
                "chat_count": store.orderedList.count,
                "running_count": runningCount,
                "managed_running_count": managedRunning,
                "shown_chat": chat ?? "",
                "event_cursor": foremanEvents.latestID,
                "audit_log": foremanAudit.fileURL.path
            ])
        case ("GET", "/v1/tier"), ("GET", "/tier"):
            return ForemanResponse(status: 200, body: [
                "tier": foremanTier.wireName,
                "tier_level": foremanTier.rawValue,
                "label": foremanTier.label,
                "short_label": foremanTier.shortLabel
            ])
        case ("GET", "/v1/events"), ("POST", "/v1/events"):
            let since = Int(query["since"] ?? query["since_id"] ?? "") ?? (payload["since"] as? Int) ?? (payload["since_id"] as? Int) ?? 0
            let limit = max(1, min(200, Int(query["limit"] ?? "") ?? (payload["limit"] as? Int) ?? 50))
            let chatFilter = query["chat_id"] ?? (payload["chat_id"] as? String) ?? ""
            let chatIds: Set<String> = chatFilter.isEmpty ? [] : [chatFilter]
            let events = foremanEvents.recent(sinceId: since, chatIds: chatIds, limit: limit).map { $0.toDictionary() }
            return ForemanResponse(status: 200, body: [
                "cursor": foremanEvents.latestID,
                "events": events
            ])
        case ("GET", "/v1/audit"), ("POST", "/v1/audit"):
            let limit = max(1, min(200, Int(query["limit"] ?? "") ?? (payload["limit"] as? Int) ?? 50))
            return ForemanResponse(status: 200, body: [
                "path": foremanAudit.fileURL.path,
                "entries": foremanAudit.tail(limit: limit)
            ])
        case ("POST", "/v1/chats/list"), ("GET", "/v1/chats"), ("GET", "/chats"):
            var merged = payload
            if merged["filter"] == nil, let f = query["filter"] { merged["filter"] = f }
            if merged["limit"] == nil, let l = Int(query["limit"] ?? "") { merged["limit"] = l }
            return foremanListChats(merged)
        case ("POST", "/v1/chats/read"), ("POST", "/chats/read"):
            return foremanReadChat(payload)
        case ("POST", "/v1/chats/create"), ("POST", "/chats/create"):
            let resp = foremanCreateChat(payload)
            foremanAudit.record(action: "create_chat", tier: foremanTier, chatID: resp.body["chat_id"] as? String, status: resp.status, detail: [
                "title": resp.body["title"] ?? payload["title"] ?? "",
                "folder": resp.body["folder"] ?? payload["folder"] ?? ""
            ])
            return resp
        case ("POST", "/v1/chats/send"), ("POST", "/chats/send"):
            let resp = foremanSendChat(payload)
            foremanAudit.record(action: "send_chat", tier: foremanTier, chatID: payload["chat_id"] as? String, status: resp.status, detail: [
                "mode": payload["mode"] ?? "send"
            ])
            return resp
        case ("POST", "/v1/chats/control"), ("POST", "/chats/control"):
            let resp = foremanControlChat(payload)
            let act = (payload["action"] as? String) ?? ""
            if act != "open" {
                foremanAudit.record(action: "control:\(act)", tier: foremanTier, chatID: payload["chat_id"] as? String, status: resp.status, detail: payload)
            }
            return resp
        case ("POST", "/v1/chats/stop-all"), ("POST", "/chats/stop-all"):
            let resp = foremanStopAll(payload)
            foremanAudit.record(action: "stop_all", tier: foremanTier, chatID: nil, status: resp.status, detail: [
                "stopped": resp.body["stopped"] ?? []
            ])
            return resp
        case ("POST", "/v1/search/chats"), ("POST", "/search/chats"):
            return foremanSearchChats(payload)
        case ("POST", "/v1/search/repo"), ("POST", "/search/repo"):
            return foremanSearchRepo(payload)
        default:
            return ForemanResponse(status: 404, body: ["error": "unknown endpoint: \(method) \(path)"])
        }
    }

    private func foremanStopAll(_ payload: [String: Any]) -> ForemanResponse {
        var stopped: [String] = []
        if cd_running(page) != 0, let id = chat, id != Foreman.chatID {
            stopRun(run)
            stopped.append(id)
            _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
        }
        for id in Array(runs.keys) where id != Foreman.chatID {
            if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
                stopped.append(id)
                _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
            }
        }
        cd_chats(page, chatLines)
        syncForemanUI()
        wake()
        return ForemanResponse(status: 200, body: ["ok": true, "stopped": stopped])
    }

    private func foremanWait(_ payload: [String: Any], includeForeman: Bool = false, done: @escaping (ForemanResponse) -> Void) {
        let mode = ((payload["mode"] as? String) ?? "any").lowercased()
        let timeoutSec = max(1.0, min(300.0, (payload["timeout_seconds"] as? Double) ?? Double((payload["timeout_seconds"] as? Int) ?? 60)))
        var targets: [String] = []
        if let ids = payload["chat_ids"] as? [String], !ids.isEmpty {
            targets = ids.filter { !$0.isEmpty && (includeForeman || $0 != Foreman.chatID) }
        } else if let single = payload["chat_id"] as? String, !single.isEmpty {
            targets = [single]
        } else {
            // Default to currently running managed chats, or any running non-Foreman chats.
            let runningManaged = store.orderedList.filter { $0.managedBy == Foreman.managedTag && busy($0.id) }.map { $0.id }
            targets = runningManaged.isEmpty ? store.orderedList.filter { busy($0.id) }.map { $0.id } : runningManaged
        }
        if targets.isEmpty {
            done(ForemanResponse(status: 200, body: [
                "ok": true,
                "timed_out": false,
                "completed": [],
                "still_running": [],
                "message": "no running chats to wait for"
            ]))
            return
        }
        let targetSet = Set(targets)
        let evaluate: () -> (satisfied: Bool, completed: [String], running: [String]) = { [weak self] in
            guard let self else { return (true, [], []) }
            let running = targets.filter { self.busy($0) }
            let completed = targets.filter { !self.busy($0) }
            let ok = mode == "all" ? running.isEmpty : !completed.isEmpty
            return (ok, completed, running)
        }
        let initial = evaluate()
        if initial.satisfied {
            done(ForemanResponse(status: 200, body: [
                "ok": true,
                "timed_out": false,
                "mode": mode,
                "completed": initial.completed,
                "still_running": initial.running
            ]))
            return
        }
        var finished = false
        var token: UUID?
        let finishWait: (Bool, ForemanEvent?) -> Void = { [weak self] timedOut, trigger in
            guard !finished else { return }
            finished = true
            if let t = token { self?.foremanEvents.unsubscribe(t); token = nil }
            let state = evaluate()
            var body: [String: Any] = [
                "ok": true,
                "timed_out": timedOut,
                "mode": mode,
                "completed": state.completed,
                "still_running": state.running
            ]
            if let trigger { body["event"] = trigger.toDictionary() }
            done(ForemanResponse(status: 200, body: body))
        }
        token = foremanEvents.subscribe { ev in
            guard ev.type == "chat.finished" || ev.type == "chat.stopped" || ev.type == "chat.deleted" else { return }
            if let cid = ev.chatID, !targetSet.contains(cid) { return }
            let state = evaluate()
            if state.satisfied {
                finishWait(false, ev)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSec) {
            finishWait(true, nil)
        }
    }

    static func goalStatusName(_ status: Int32) -> String {
        switch status {
        case 1: return "active"
        case 2: return "paused"
        case 3: return "complete"
        case 4: return "blocked"
        case 5: return "budget_limited"
        case 6: return "failed"
        default: return "none"
        }
    }

    private func foremanSearchChats(_ payload: [String: Any]) -> ForemanResponse {
        guard let query = payload["query"] as? String, !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "query is required"])
        }
        let limit = max(1, min(50, (payload["limit"] as? Int) ?? 20))
        let folderFilter = (payload["folder"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let managedOnly = (payload["managed_only"] as? Bool) ?? false
        let df = DateFormatter()
        df.dateFormat = "yyyy-MM-dd"
        df.timeZone = TimeZone(secondsFromGMT: 0)
        let dateFromTs = (payload["date_from"] as? String).flatMap { df.date(from: $0)?.timeIntervalSince1970 } ?? 0
        let dateToTs = (payload["date_to"] as? String).flatMap { df.date(from: $0)?.timeIntervalSince1970 }.map { $0 + 86400 } ?? 0
        let since = max((payload["since"] as? Double) ?? (payload["since"] as? Int).map(Double.init) ?? 0, dateFromTs)
        let needle = query.lowercased()
        var results: [[String: Any]] = []
        for meta in store.orderedList {
            if results.count >= limit { break }
            if managedOnly && meta.managedBy != Foreman.managedTag { continue }
            if !folderFilter.isEmpty && !meta.folder.localizedCaseInsensitiveContains(folderFilter) { continue }
            if since > 0 && meta.updated < since { continue }
            if dateToTs > 0 && meta.updated > dateToTs { continue }
            var snippets: [String] = []
            if meta.title.lowercased().contains(needle) {
                snippets.append("title: " + meta.title)
            }
            if let raw = snapshotText(for: meta.id), let data = raw.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let turns = obj["turns"] as? [[String: Any]] {
                for (idx, t) in turns.enumerated() {
                    if snippets.count >= 3 { break }
                    let pairs: [(String, String?)] = [
                        ("user", (t["ask"] as? String) ?? (t["u"] as? String)),
                        ("reply", (t["reply"] as? String) ?? (t["r"] as? String))
                    ]
                    for (role, maybeText) in pairs {
                        guard let text = maybeText, let range = text.range(of: query, options: .caseInsensitive) else { continue }
                        let start = text.index(range.lowerBound, offsetBy: -60, limitedBy: text.startIndex) ?? text.startIndex
                        let end = text.index(range.upperBound, offsetBy: 120, limitedBy: text.endIndex) ?? text.endIndex
                        let excerpt = String(text[start..<end]).replacingOccurrences(of: "\n", with: " ")
                        snippets.append("turn \(idx + 1) (\(role)): " + excerpt)
                        if snippets.count >= 3 { break }
                    }
                }
            }
            if !snippets.isEmpty {
                results.append([
                    "chat_id": meta.id,
                    "title": meta.title,
                    "managed_by": meta.managedBy ?? "",
                    "folder": meta.folder,
                    "updated": meta.updated,
                    "snippets": snippets
                ])
            }
        }
        return ForemanResponse(status: 200, body: [
            "query": query,
            "total": results.count,
            "results": results
        ])
    }

    private func foremanSearchRepo(_ payload: [String: Any]) -> ForemanResponse {
        guard let query = payload["query"] as? String, !query.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "query is required"])
        }
        let limit = max(1, min(200, (payload["max"] as? Int) ?? (payload["limit"] as? Int) ?? 50))
        let rootPath: String
        if let sub = ((payload["folder"] as? String) ?? (payload["path"] as? String)), !sub.isEmpty {
            rootPath = sub.hasPrefix("/") ? sub : folder.appendingPathComponent(sub).path
        } else {
            rootPath = folder.path
        }
        let p = Process()
        let isGitRepo = FileManager.default.fileExists(atPath: URL(fileURLWithPath: rootPath).appendingPathComponent(".git").path) ||
                        FileManager.default.fileExists(atPath: folder.appendingPathComponent(".git").path)
        if isGitRepo {
            p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            var args = ["-C", rootPath, "grep", "-n", "-i", "-F", query]
            if let glob = payload["glob"] as? String, !glob.isEmpty { args.append(contentsOf: ["--", glob]) }
            p.arguments = args
        } else {
            p.executableURL = URL(fileURLWithPath: "/usr/bin/grep")
            p.arguments = ["-rnIiF", query, rootPath]
        }
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch {
            return ForemanResponse(status: 500, body: ["error": "search command failed: \(error.localizedDescription)"])
        }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let text = String(data: data, encoding: .utf8) ?? ""
        var matches: [[String: Any]] = []
        for line in text.split(separator: "\n") {
            if matches.count >= limit { break }
            let parts = line.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            if parts.count >= 3 {
                matches.append([
                    "file": String(parts[0]),
                    "line": Int(parts[1]) ?? 0,
                    "text": String(parts[2]).trimmingCharacters(in: .whitespaces)
                ])
            }
        }
        return ForemanResponse(status: 200, body: [
            "workspace": rootPath,
            "query": query,
            "total": matches.count,
            "matches": matches
        ])
    }
}
