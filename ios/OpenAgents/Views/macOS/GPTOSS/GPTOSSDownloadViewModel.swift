import Foundation
import Combine
import OpenAgentsCore
import SwiftUI

@MainActor
final class GPTOSSDownloadViewModel: ObservableObject {
    enum Status { case notDownloaded, downloading, ready, error(String) }

    @Published var status: Status = .notDownloaded
    @Published var progress: Double = 0.0
    @Published var downloadedBytes: Int64 = 0
    @Published var totalBytes: Int64 = 0

    private var task: Task<Void, Never>?
    private let manager = GPTOSSModelManager()
    private let requiredFreeBytes: Int64 = 25 * 1024 * 1024 * 1024 // 25 GB

    var statusText: String {
        switch status {
        case .notDownloaded: return "Not Downloaded"
        case .downloading: return "Downloading…"
        case .ready: return "Ready"
        case .error(let msg): return "Error: \(msg)"
        }
    }

    var statusColor: Color {
        switch status {
        case .notDownloaded: return .gray
        case .downloading: return .blue
        case .ready: return .green
        case .error: return .red
        }
    }

    var isDownloading: Bool { if case .downloading = status { return true } else { return false } }
    var isDownloaded: Bool { if case .ready = status { return true } else { return false } }

    var downloadedGB: String { String(format: "%.1f", Double(downloadedBytes) / 1_073_741_824.0) }
    var totalGB: String { totalBytes > 0 ? String(format: "%.1f", Double(totalBytes) / 1_073_741_824.0) : "12.1" }

    var hasSufficientSpace: Bool { (freeDiskBytes() ?? 0) >= requiredFreeBytes }

    func startDownload() async {
        guard !isDownloading else { return }
        status = .downloading
        progress = 0
        downloadedBytes = 0
        totalBytes = 0

        task = Task { [weak self] in
            guard let self = self else { return }
            do {
                try await self.manager.downloadModel { prog in
                    Task { @MainActor in
                        self.progress = prog.fractionCompleted
                        self.downloadedBytes = prog.bytesDownloaded
                        self.totalBytes = prog.totalBytes
                    }
                }
                await MainActor.run { self.status = .ready }
            } catch {
                await MainActor.run { self.status = .error(error.localizedDescription) }
            }
        }
    }

    func pauseDownload() {
        task?.cancel()
        status = .notDownloaded
    }

    func cancelDownload() {
        task?.cancel()
        status = .notDownloaded
        progress = 0
        downloadedBytes = 0
        totalBytes = 0
    }

    private func freeDiskBytes() -> Int64? {
        let url = URL(fileURLWithPath: NSHomeDirectory())
        if let values = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey]),
           let free = values.volumeAvailableCapacityForImportantUsage {
            return free
        }
        return nil
    }
}

