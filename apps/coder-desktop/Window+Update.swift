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
            let alert = NSAlert()
            alert.messageText = "Coder Updates"
            switch state {
            case .hidden:
                alert.informativeText = "Coder is up to date."
                alert.beginSheetModal(for: self.window)
            case .error(let msg, _):
                alert.alertStyle = .warning
                alert.informativeText = msg
                alert.beginSheetModal(for: self.window)
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
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = "Update Failed"
            alert.informativeText = err.localizedDescription
            alert.beginSheetModal(for: window)
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
                let alert = NSAlert()
                alert.alertStyle = .warning
                alert.messageText = count == 1 ? "An agent task is still running" : "\(count) agent tasks are still running"
                alert.informativeText = "Restarting to install the update will stop all running agent tasks."
                alert.addButton(withTitle: "Cancel")
                alert.addButton(withTitle: "Restart & Update")
                alert.beginSheetModal(for: window) { [weak self] response in
                    guard response == .alertSecondButtonReturn else { return }
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
