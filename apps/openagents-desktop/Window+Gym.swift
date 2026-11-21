// Firm reason: 6 (AppKit lifecycle and Gym Workbench mode hosting over the compiled page).
// Stage 2 of GitHub #13792 & #13946: Unified Gym mode bar (Benchmarks | PLE 3D | Lean · RH).
import AppKit

extension CoderWindow {
    /// Opens the Gym in the main content area beside the sidebar.
    /// mode: 0 Benchmarks, 1 PLE 3D, 2 Lean · RH (nil keeps the page's current mode).
    func openGym(mode: Int32? = nil) {
        guard Experimental.on else { return }   // an experimental feature (Settings > General)
        cd_gym_set(page, 1)
        if let m = mode { cd_gym_mode_set(page, m) }
        hideScenePanel()
        let curMode = cd_gym_mode(page)
        if curMode == 2 {
            gymView?.isHidden = true
            hydrateLeanWorkbench(force: false)
        } else {
            let workbench = ensureGymView()
            workbench.setModeBenchmarks(curMode == 0)
            workbench.refreshRuns(autoSelectFirst: true)
        }
        wake()
        syncGymPanelFrame()
    }

    /// Leaves Gym mode when the user switches to a chat, Foreman, or Settings.
    func closeGym() {
        guard cd_gym_open(page) != 0 else { return }
        cd_gym_set(page, 0)
        gymView?.isHidden = true
        wake()
    }

    func ensureGymView() -> GymWorkbenchView {
        if let existing = gymView {
            if existing.superview == nil {
                canvas.addSubview(existing)
            }
            return existing
        }
        let view = GymWorkbenchView(frame: .zero)
        view.workingFolderProvider = { [weak self] in
            self?.folder ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        }
        view.isHidden = true
        canvas.addSubview(view)
        gymView = view
        return view
    }

    /// Keeps the GymWorkbenchView aligned to the Coax page's gym rect (right of sidebar, below lifted 36 pt GYM bar).
    func syncGymPanelFrame() {
        guard let view = gymView else { return }
        var rect = [Float](repeating: 0, count: 4)
        let shown = cd_gym_rect(page, &rect)
        guard shown != 0, rect[2] > 1, rect[3] > 1 else {
            if !view.isHidden { view.isHidden = true }
            return
        }
        let canvasH = canvas.bounds.height
        let x = CGFloat(rect[0]).rounded()
        let w = CGFloat(rect[2]).rounded()
        let h = CGFloat(rect[3]).rounded()
        let y = (canvasH - CGFloat(rect[1]) - h).rounded()
        let target = NSRect(x: x, y: max(0, y), width: max(1, w), height: max(1, h))
        if view.frame != target {
            view.frame = target
        }
        if view.isHidden {
            view.isHidden = false
        }
    }

    // MARK: - Lean 4 & Riemann Hypothesis Proof Workbench (#13946)

    static var leanGymRequestFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/gym-request.json") }
    static var leanGymStateFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/gym-state.json") }
    static var rhAtlasFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/index/rh-atlas.json") }

    func leanRunPath() -> String? {
        let fm = FileManager.default
        if let bundled = Bundle.main.resourceURL?.appendingPathComponent("lean-run").path, fm.isExecutableFile(atPath: bundled) {
            return bundled
        }
        let shared = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/lean-run").path
        if fm.isExecutableFile(atPath: shared) { return shared }
        return nil
    }

    func runLeanTool(_ args: [String], done: @escaping (String?) -> Void) {
        guard let tool = leanRunPath() else {
            DispatchQueue.main.async { done(nil) }
            return
        }
        DispatchQueue.global(qos: .userInitiated).async {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: tool)
            p.arguments = args
            var env = ProcessInfo.processInfo.environment
            let home = FileManager.default.homeDirectoryForCurrentUser.path
            let elanBin = home + "/.elan/bin"
            let curPath = env["PATH"] ?? "/usr/bin:/bin"
            if !curPath.contains(elanBin) { env["PATH"] = elanBin + ":" + curPath }
            p.environment = env
            let pipe = Pipe()
            p.standardOutput = pipe
            p.standardError = Pipe()
            do {
                try p.run()
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                let text = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                DispatchQueue.main.async { done(text.isEmpty ? nil : text) }
            } catch {
                DispatchQueue.main.async { done(nil) }
            }
        }
    }

    func hydrateLeanWorkbench(force: Bool = false) {
        if let data = try? Data(contentsOf: CoderWindow.rhAtlasFile),
           let text = String(data: data, encoding: .utf8), !text.isEmpty {
            _ = cd_lean_atlas_set(page, text)
            wake()
        }
        if !force && leanHydrated { return }
        leanHydrated = true
        runLeanTool(["doctor", "--json"]) { [weak self] out in
            guard let self, let out else { return }
            _ = cd_lean_doctor_set(self.page, out)
            self.wake()
        }
        runLeanTool(["rh", "audit", "--json"]) { [weak self] out in
            guard let self, let out else { return }
            _ = cd_lean_audit_set(self.page, out)
            self.wake()
        }
        runLeanTool(["rh", "module", "Main", "--json"]) { [weak self] out in
            guard let self, let out else { return }
            _ = cd_lean_module_set(self.page, out)
            self.wake()
        }
    }

    private func leanReqString() -> String {
        var buf = [CChar](repeating: 0, count: 1024)
        let n = cd_lean_req_str(page, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    /// Dispatches CP_ASK_LEAN_* actions (150..157) raised by page/gym-lean.coax.
    func leanGymAsked(_ what: Int32) {
        switch what {
        case 150: // CP_ASK_LEAN_MODE
            let mode = cd_gym_mode(page)
            if mode == 2 {
                gymView?.isHidden = true
                hydrateLeanWorkbench(force: false)
            } else {
                let wb = ensureGymView()
                wb.setModeBenchmarks(mode == 0)
                if mode == 1 { wb.refreshRuns(autoSelectFirst: false) }
            }
            syncGymPanelFrame()
            wake()
        case 151: // CP_ASK_LEAN_INDEX
            cd_toast(page, "Building Riemann Hypothesis 175k-module index (~28 s)...", 0)
            wake()
            runLeanTool(["rh", "index", "--json"]) { [weak self] out in
                guard let self else { return }
                if out != nil {
                    self.hydrateLeanWorkbench(force: true)
                    cd_toast(self.page, "Riemann Hypothesis index rebuilt", 0)
                } else {
                    cd_toast(self.page, "RH index failed — run `lean-run rh fetch` first if corpus is missing", 1)
                }
                self.wake()
            }
        case 152: // CP_ASK_LEAN_MODULE
            let mod = leanReqString()
            guard !mod.isEmpty else { return }
            runLeanTool(["rh", "module", mod, "--json"]) { [weak self] out in
                guard let self, let out else { return }
                _ = cd_lean_module_set(self.page, out)
                self.wake()
            }
        case 153: // CP_ASK_LEAN_SEARCH
            let query = leanReqString()
            guard !query.isEmpty else { return }
            runLeanTool(["rh", "search", query, "--limit", "24", "--json"]) { [weak self] out in
                guard let self, let out else { return }
                _ = cd_lean_search_set(self.page, out)
                self.wake()
            }
        case 154: // CP_ASK_LEAN_EXPLAIN
            let mod = leanReqString()
            let target = mod.isEmpty ? "Main" : mod
            let prompt = "Explain Lean 4 module `\(target)` from Levent Alpöge's Nov 20, 2026 Lean 4 proof of the Riemann Hypothesis (`~/.openagents/lean/corpora/riemann-hypothesis`). Use `lean-run rh module \(target)` and `lean-run rh path \(target)` to cite exact declaration names, its shortest dependency chain to `Main.riemannHypothesis`, and its mathematical role in the proof."
            closeGym()
            newChat()
            let nextAct = cd_action(page)
            if nextAct != 0 { act(nextAct) }
            if cd_set_text(page, prompt) != 0 {
                _ = cd_submit(page)
                let sendAct = cd_action(page)
                if sendAct != 0 { act(sendAct) }
            }
            wake()
        case 155: // CP_ASK_LEAN_VERIFY
            let idx = Int(leanReqString()) ?? 0
            let sampleDir = Bundle.main.resourceURL?.appendingPathComponent("fixtures/lean-samples/add_zero_right").path
                ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/samples/add_zero_right").path
            let rhMain = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/lean/corpora/riemann-hypothesis/src/Main.lean").path
            let pair: (String, String)
            switch idx {
            case 0: pair = (sampleDir + "/AddZeroRight.lean", "add_zero_right")
            case 1: pair = (sampleDir + "/CheatSorry.lean", "add_zero_right")
            case 2: pair = (sampleDir + "/CheatAxiom.lean", "add_zero_right")
            case 3: pair = (sampleDir + "/CheatNativeDecide.lean", "two_plus_two")
            default: pair = (rhMain, "riemannHypothesis")
            }
            runLeanTool(["verify", pair.0, pair.1, "--json"]) { [weak self] out in
                guard let self, let out else { return }
                _ = cd_lean_verify_set(self.page, out)
                self.wake()
            }
        case 156: // CP_ASK_LEAN_OPEN_FILE
            let path = leanReqString()
            guard !path.isEmpty else { return }
            closeGym()
            openNamedFile(URL(fileURLWithPath: path))
            wake()
        case 157: // CP_ASK_FILE_LEAN_GOAL
            refreshFilePaneLeanGoal()
        default:
            break
        }
    }

    /// Handles a Lean Gym request (lean-gym-request.v1 JSON) via POST /v1/gym/lean or ~/.openagents/lean/gym-request.json.
    func handleLeanGymRequest(_ json: String, done: @escaping ([String: Any]) -> Void) {
        guard Experimental.on else {
            done(["ok": false, "error": "the Gym is an experimental feature: turn on Experimental features in Settings > General"])
            return
        }
        let planText = DesktopCore.withCoaxString(json) { r in
            DesktopCore.callString(8192) { oa_dc_lean_gym_plan(r, $0) }
        }
        guard let planData = planText.data(using: .utf8),
              let planObj = (try? JSONSerialization.jsonObject(with: planData)) as? [String: Any] else {
            done(["ok": false, "error": "the desktop core gave no Lean Gym plan"])
            return
        }
        guard (planObj["ok"] as? Bool) == true else {
            done(["ok": false, "error": (planObj["error"] as? String) ?? "refused"])
            return
        }
        let reqId = (planObj["id"] as? String) ?? ""
        let shouldOpen = (planObj["open"] as? Bool) ?? true
        if shouldOpen {
            openGym(mode: 2)
        }
        _ = cd_lean_apply_plan(page, planText)
        // Ensure atlas is hydrated immediately on main thread if index exists
        if let data = try? Data(contentsOf: CoderWindow.rhAtlasFile),
           let text = String(data: data, encoding: .utf8), !text.isEmpty {
            _ = cd_lean_atlas_set(page, text)
        }
        syncGymPanelFrame()
        wake()

        let mod = (planObj["module"] as? String) ?? ""
        let search = (planObj["search"] as? String) ?? ""
        let group = DispatchGroup()
        if !mod.isEmpty {
            group.enter()
            runLeanTool(["rh", "module", mod, "--json"]) { [weak self] out in
                if let self, let out { _ = cd_lean_module_set(self.page, out); self.wake() }
                group.leave()
            }
        }
        if !search.isEmpty {
            group.enter()
            runLeanTool(["rh", "search", search, "--limit", "24", "--json"]) { [weak self] out in
                if let self, let out { _ = cd_lean_search_set(self.page, out); self.wake() }
                group.leave()
            }
        }
        group.notify(queue: .main) { [weak self] in
            guard let self else { done(["ok": false, "error": "window closed"]); return }
            done(self.leanGymStateForProgram(reqId: reqId))
        }
    }

    /// Exports lean-gym-state.v1 JSON dictionary for GET /v1/gym/lean.
    func leanGymStateForProgram(reqId: String = "") -> [String: Any] {
        guard Experimental.on else {
            return ["ok": false, "error": "the Gym is an experimental feature: turn on Experimental features in Settings > General"]
        }
        var buf = [CChar](repeating: 0, count: 8192)
        let n = cd_lean_state_json(page, reqId, &buf, Int32(buf.count))
        if n > 0, let data = String(cString: buf).data(using: .utf8),
           var obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] {
            obj["ok"] = true
            return obj
        }
        return ["ok": true, "open": cd_gym_open(page) != 0, "mode": "lean"]
    }

    // MARK: - A program's request (GitHub #13812; docs/desktop/steer-ple-from-chat.md, "A Gym route for programs")

    /// Carries out a Gym request (ple-gym-request.v1 JSON): checked and worded by the desktop core
    /// (oa_dc_gym_plan), then the Gym opened unless it says open false, its run, step, camera, layers,
    /// element and Live set. done hears the Gym's state (ple-gym-state.v1) with ok, and error when
    /// it was refused or went wrong.
    func handleGymRequest(_ json: String, done: @escaping ([String: Any]) -> Void) {
        guard Experimental.on else {
            done(["ok": false, "error": "the Gym is an experimental feature: turn on Experimental features in Settings > General"])
            return
        }
        let made = PleGymPlan.make(request: json)
        guard let plan = made.plan else {
            done(["ok": false, "error": made.error ?? "refused"])
            return
        }
        if plan.open {
            openGym(mode: 1)
        }
        let workbench = ensureGymView()
        workbench.applyPlan(plan) { [weak self] error in
            var state = workbench.gymState()
            state["ok"] = error == nil
            state["id"] = plan.id
            state["open"] = self.map { cd_gym_open($0.page) != 0 } ?? false
            if let self {
                let m = cd_gym_mode(self.page)
                state["mode"] = m == 0 ? "benchmarks" : (m == 1 ? "ple" : "lean")
            }
            if let error { state["error"] = error }
            done(state)
        }
    }

    /// The Gym's state for GET /v1/gym.
    func gymStateForProgram() -> [String: Any] {
        guard Experimental.on else { return ["ok": false, "error": "the Gym is an experimental feature: turn on Experimental features in Settings > General"] }
        var state = gymView?.gymState() ?? ["version": "ple-gym-state.v1"]
        state["ok"] = true
        state["open"] = cd_gym_open(page) != 0
        let m = cd_gym_mode(page)
        state["mode"] = m == 0 ? "benchmarks" : (m == 1 ? "ple" : "lean")
        return state
    }

    /// `ple-run gym` and `lean-run gym` hand requests over in ~/.openagents/{ple,lean}/gym-request.json,
    /// taken (and removed) twice a second while Experimental features are on; answers go to gym-state.json beside them.
    static var gymRequestFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/ple/gym-request.json") }
    static var gymStateFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/ple/gym-state.json") }

    func startGymRequests() {
        guard gymRequestTimer == nil else { return }
        gymRequestTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.takeGymRequest()
            self?.takeLeanGymRequest()
        }
    }

    func stopGymRequests() {
        gymRequestTimer?.invalidate()
        gymRequestTimer = nil
    }

    private func takeGymRequest() {
        let file = CoderWindow.gymRequestFile
        guard let data = try? Data(contentsOf: file) else { return }
        try? FileManager.default.removeItem(at: file)
        handleGymRequest(String(decoding: data, as: UTF8.self)) { state in
            guard let out = try? JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]) else { return }
            let tmp = CoderWindow.gymStateFile.appendingPathExtension("tmp")
            do {
                try out.write(to: tmp)
                _ = try FileManager.default.replaceItemAt(CoderWindow.gymStateFile, withItemAt: tmp)
            } catch {
                try? out.write(to: CoderWindow.gymStateFile, options: .atomic)
            }
        }
    }

    private func takeLeanGymRequest() {
        let file = CoderWindow.leanGymRequestFile
        guard let data = try? Data(contentsOf: file) else { return }
        try? FileManager.default.removeItem(at: file)
        handleLeanGymRequest(String(decoding: data, as: UTF8.self)) { state in
            guard let out = try? JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]) else { return }
            let tmp = CoderWindow.leanGymStateFile.appendingPathExtension("tmp")
            do {
                try out.write(to: tmp)
                _ = try FileManager.default.replaceItemAt(CoderWindow.leanGymStateFile, withItemAt: tmp)
            } catch {
                try? out.write(to: CoderWindow.leanGymStateFile, options: .atomic)
            }
        }
    }
}
