// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Updates: checking, the update pill, install and relaunch.
import AppKit

extension CoderWindow {
    func syncUpdateUI() {
        let st = updater.state
        cd_update(page, st.coaxCode, st.pillLabel, st.pillDetail)
        wake()
    }
    func checkForUpdates() {
        updater.checkNow(userInitiated: true) { [weak self] state in
            guard let self else { return }
            switch state {
            case .hidden:
                self.present(.notice("Coder Updates", "Coder is up to date."))
            case .error(let msg, _):
                self.present(.notice("Coder Updates", msg))
            case .available, .downloading, .ready:
                break
            }
        }
    }
    private func performInstallAndRelaunch() {
        switch updater.applyStagedAndRelaunch() {
        case .success:
            retire()
            NSApp.terminate(nil)
        case .failure(let err):
            present(.notice("Update Failed", err.localizedDescription))
        }
    }
    func handleUpdatePillClick() {
        switch updater.state {
        case .available(_, let source):
            if case .signedRelease(let manifest) = source {
                updater.beginDownload(for: manifest)
            }
        case .ready:
            let count = runningCount
            if count > 0 {
                // it stops the running tasks: Return stays on Cancel
                var d = CoderDialog(title: count == 1 ? "An agent task is still running" : "\(count) agent tasks are still running",
                                    body: "Restarting to install the update will stop all running agent tasks.")
                d.buttons = [.init(title: "Cancel"), .init(title: "Restart & Update", kind: .danger)]
                d.confirm = 0; d.cancel = 0
                present(d) { [weak self] answer, _ in
                    guard answer == 1 else { self?.refocus(); return }
                    self?.performInstallAndRelaunch()
                }
                return
            }
            performInstallAndRelaunch()
        case .error:
            updater.checkNow(userInitiated: true)
        default:
            break
        }
    }
}
