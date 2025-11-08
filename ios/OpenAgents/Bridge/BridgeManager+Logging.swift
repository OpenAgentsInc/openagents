import Foundation
import OSLog
import OpenAgentsCore

extension BridgeManager {
    func log(_ tag: String, _ message: String) {
        let ts = ISO8601DateFormatter().string(from: Date())
        let line = "[\(ts)] [\(tag)] \(message)"
        OpenAgentsLog.bridge.debug("[Bridge] \(line)")
        if Thread.isMainThread {
            lastLog = line
            logs.append(line)
            if logs.count > 200 { logs.removeFirst(logs.count - 200) }
        } else {
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.lastLog = line
                self.logs.append(line)
                if self.logs.count > 200 { self.logs.removeFirst(self.logs.count - 200) }
            }
        }
    }
}
