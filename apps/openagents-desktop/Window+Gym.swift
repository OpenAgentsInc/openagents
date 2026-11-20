// Firm reason: 6 (AppKit lifecycle and Gym Workbench mode hosting over the compiled page).
// Stage 2 of GitHub #13792: Gym sidebar mode & 3D PLE Workbench host integration.
import AppKit

extension CoderWindow {
    /// Opens the Gym / 3D PLE Workbench view in the main content area beside the sidebar.
    func openGym() {
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

    private func ensureGymView() -> GymWorkbenchView {
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
}
