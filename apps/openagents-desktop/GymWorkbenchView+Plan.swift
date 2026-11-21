// Firm reason: 6 (AppKit/Metal 3D PLE Gym Workbench view over PleScenePanelView and local run discovery).
// Stage 2 of GitHub #13792: 3D PLE Run Visualizer / Gym Workbench in OpenAgents Desktop.
import AppKit
import CryptoKit
import Foundation
import UniformTypeIdentifiers

// Split from GymWorkbenchView.swift (#13848): PleGymPlan and programmatic gym actions.

// MARK: - A program's request (GitHub #13812): `ple-run gym` and POST /v1/gym

/// A request to the Gym as oa_dc_gym_plan (coax/lib/agent/desktop/gym.coax) checked and worded it.
struct PleGymPlan {
    let id: String
    let open: Bool
    let run: String
    let camera: PleCameraPreset?
    let layers: [PleSemanticLayer: Bool]
    var step: Int          // 0 as it is, -1 the last, N from 1
    let select: String
    let live: Int          // 1 on, 0 off, -1 as it is

    /// The plan for a request's JSON text, or why it is refused.
    static func make(request: String) -> (plan: PleGymPlan?, error: String?) {
        let text = DesktopCore.withCoaxString(request) { r in DesktopCore.callString(8192) { oa_dc_gym_plan(r, $0) } }
        guard let data = text.data(using: .utf8),
              let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            return (nil, "the desktop core gave no plan")
        }
        guard (o["ok"] as? Bool) == true else { return (nil, (o["error"] as? String) ?? "refused") }
        var layers: [PleSemanticLayer: Bool] = [:]
        let marks = (o["layers"] as? [Int]) ?? []
        for (i, layer) in PleSemanticLayer.allCases.enumerated() where i < marks.count && marks[i] >= 0 {
            layers[layer] = marks[i] == 1
        }
        let cam = (o["camera"] as? String) ?? ""
        return (PleGymPlan(
            id: (o["id"] as? String) ?? "",
            open: (o["open"] as? Bool) ?? true,
            run: (o["run"] as? String) ?? "",
            camera: cam.isEmpty ? nil : PleCameraPreset(rawValue: cam),
            layers: layers,
            step: (o["step"] as? Int) ?? 0,
            select: (o["select"] as? String) ?? "",
            live: (o["live"] as? Int) ?? -1
        ), nil)
    }
}

extension GymWorkbenchView {
    /// Carries out a plan: the run first (its scene loads off the main thread), then the step, then
    /// the camera, layers, element and Live. done hears nil, or what went wrong, once it is over.
    func applyPlan(_ plan: PleGymPlan, done: @escaping (String?) -> Void) {
        if pendingPlan != nil { planFinished("another request came first") }
        pendingPlan = plan
        pendingPlanDone = done
        guard !plan.run.isEmpty else {
            if activeScene == nil {
                refreshRuns(autoSelectFirst: true)   // nothing shown yet: the first run, then the rest
                if selectedItem == nil { planFinished("the Gym has no run to show") }
                return
            }
            planSceneReady()
            return
        }
        refreshRuns(autoSelectFirst: false)
        guard let item = resolveRun(plan.run) else {
            planFinished("no run \(plan.run) in the Gym (it lists ~/.openagents/ple/runs, ple-kit/runs and the sample)")
            return
        }
        if !items.contains(where: { $0.id == item.id }) {
            items.insert(item, at: min(1, items.count))
            rebuildRows()
            rebuildCompareMenu()
        }
        selectItem(item)
    }

    /// A run a request names: "latest" (the newest run's), a run's name under ~/.openagents/ple/runs,
    /// a run's folder (its run.ple-scene.json or model.ple-scene.json, else its newest scene), or a
    /// scene or IFC file.
    func resolveRun(_ run: String) -> PleGymRunItem? {
        let fm = FileManager.default
        if run == "latest" {
            guard let newest = items.first(where: { !$0.isSample }) else { return nil }
            return bestItem(in: newest.url.deletingLastPathComponent().path) ?? newest
        }
        let path = run.contains("/")
            ? (run as NSString).expandingTildeInPath
            : fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/ple/runs/\(run)").path
        let canon = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: canon, isDirectory: &isDir) else { return nil }
        if isDir.boolValue { return bestItem(in: canon) }
        if let known = items.first(where: { $0.id == canon }) { return known }
        let url = URL(fileURLWithPath: canon)
        guard PleSceneLoader.isSceneFile(url) || PleSceneLoader.isIfcFile(url) else { return nil }
        let isIfc = PleSceneLoader.isIfcFile(url)
        return PleGymRunItem(id: canon, title: url.lastPathComponent, subtitle: "Asked for • \(isIfc ? "IFC" : "PLE Scene")",
                             url: url, isIfc: isIfc, isSample: false, modifiedAt: Date())
    }

    private func bestItem(in folder: String) -> PleGymRunItem? {
        let inside = items.filter { $0.id.hasPrefix(folder + "/") }
        for name in ["run.ple-scene.json", "model.ple-scene.json"] {
            if let hit = inside.first(where: { $0.id == folder + "/" + name }) { return hit }
        }
        return inside.first
    }

    /// A scene is in (a run's, or a step's): the rest of the pending plan.
    func planSceneReady() {
        guard var plan = pendingPlan else { return }
        if plan.step != 0, !snapshots.isEmpty {
            let index = plan.step < 0 ? snapshots.count - 1 : plan.step - 1
            guard index < snapshots.count else {
                planFinished("the run has \(snapshots.count) step\(snapshots.count == 1 ? "" : "s"), not \(plan.step)")
                return
            }
            plan.step = 0
            pendingPlan = plan
            if index != currentSnapshotIndex || snapshots[index].scene == nil {
                stopPlayback()
                scrubToSnapshot(index: index)   // applyScrubbedScene comes back here
                return
            }
        } else if plan.step > 1 {
            planFinished("the run has 1 step, not \(plan.step)")
            return
        }
        if let camera = plan.camera { scenePanel.applyCamera(camera) }
        for (layer, on) in plan.layers { scenePanel.setLayer(layer, visible: on) }
        if !plan.select.isEmpty, !scenePanel.selectElement(id: plan.select) {
            planFinished("no element \(plan.select) in the scene")
            return
        }
        if plan.live >= 0 { setLiveTailingEnabled(plan.live == 1) }
        planFinished(nil)
    }

    func planFinished(_ error: String?) {
        guard let done = pendingPlanDone else { return }
        pendingPlan = nil
        pendingPlanDone = nil
        done(error)
    }

    /// What the Gym shows, for a program (ple-gym-state.v1).
    func gymState() -> [String: Any] {
        var state: [String: Any] = [
            "version": "ple-gym-state.v1",
            "runs": items.count,
            "run": selectedItem?.url.path ?? "",
            "title": selectedItem?.title ?? "",
            "step": snapshots.isEmpty ? 0 : currentSnapshotIndex + 1,
            "steps": snapshots.count,
            "layers": PleSemanticLayer.allCases.filter { scenePanel.metalView.visibleLayers.contains($0) }.map { $0.rawValue },
            "selected": scenePanel.metalView.selectedElementId ?? "",
            "live": liveEnabled,
            "loading": pendingPlan != nil,
            "status": statusLabel.stringValue
        ]
        if let scene = activeScene {
            state["elements"] = scene.allElements.count
            state["clashes"] = scene.totalClashCount
            if let score = scene.run?.score { state["score"] = score }
        }
        return state
    }
}
