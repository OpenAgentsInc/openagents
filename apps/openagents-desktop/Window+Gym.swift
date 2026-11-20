// Firm reason: 6 (AppKit lifecycle and Gym Workbench mode hosting over the compiled page).
// Stage 2 of GitHub #13792: Gym sidebar mode & 3D PLE Workbench host integration.
import AppKit

extension CoderWindow {
    /// Opens the Gym / 3D PLE Workbench view in the main content area beside the sidebar.
    func openGym() {
        guard Experimental.on else { return }   // an experimental feature (Settings > General)
        cd_gym_set(page, 1)
        hideScenePanel()
        let workbench = ensureGymView()
        workbench.refreshRuns(autoSelectFirst: true)
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

    /// Keeps the GymWorkbenchView aligned to the Coax page's gym rect (right of sidebar, below titlebar).
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
        if plan.open && cd_gym_open(page) == 0 { openGym() }
        let workbench = ensureGymView()
        workbench.applyPlan(plan) { [weak self] error in
            var state = workbench.gymState()
            state["ok"] = error == nil
            state["id"] = plan.id
            state["open"] = self.map { cd_gym_open($0.page) != 0 } ?? false
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
        return state
    }

    /// `ple-run gym` hands its request over in a file: ~/.openagents/ple/gym-request.json, taken
    /// (and removed) twice a second while Experimental features are on; the answer goes to
    /// gym-state.json beside it, with the request's id.
    static var gymRequestFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/ple/gym-request.json") }
    static var gymStateFile: URL { FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/ple/gym-state.json") }

    func startGymRequests() {
        guard gymRequestTimer == nil else { return }
        gymRequestTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in self?.takeGymRequest() }
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
}
