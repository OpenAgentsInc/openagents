import Foundation
import Combine
import OpenAgentsCore
import SwiftUI

@MainActor
final class GPTOSSDownloadViewModel: ObservableObject {
    enum Status { case notDownloaded, downloading, paused, ready, error(String) }

    @Published var status: Status = .notDownloaded
    @Published var progress: Double = 0.0
    @Published var downloadedBytes: Int64 = 0
    @Published var totalBytes: Int64 = 0

    private var task: Task<Void, Never>?
    private var currentToken: UUID?
    private var paused: Bool = false
    private let manager = GPTOSSModelManager()
    private let requiredFreeBytes: Int64 = 25 * 1024 * 1024 * 1024 // 25 GB

    var statusText: String {
        switch status {
        case .notDownloaded: return "Not Downloaded"
        case .downloading: return "Downloading…"
        case .paused: return "Paused"
        case .ready: return "Ready"
        case .error(let msg): return "Error: \(msg)"
        }
    }

    var statusColor: Color {
        switch status {
        case .notDownloaded: return .gray
        case .downloading: return .blue
        case .paused: return .orange
        case .ready: return .green
        case .error: return .red
        }
    }

    var isDownloading: Bool { if case .downloading = status { return true } else { return false } }
    var isDownloaded: Bool { if case .ready = status { return true } else { return false } }

    // Heuristic: if totalBytes is very small (e.g., 7), Hub.progress is reporting file count, not bytes.
    // In that case, fall back to the known total size for display and estimate downloaded bytes from percent.
    private var fallbackTotalBytes: Int64 { Int64(12.1 * 1_073_741_824.0) } // ~12.1 GiB
    private var looksLikeFileCount: Bool { totalBytes > 0 && totalBytes < (128 * 1024 * 1024) }
    private var effectiveTotalBytes: Int64 { (totalBytes > 0 && !looksLikeFileCount) ? totalBytes : fallbackTotalBytes }

    var downloadedGB: String {
        let bytes: Int64
        if looksLikeFileCount || downloadedBytes == 0 {
            bytes = Int64(Double(effectiveTotalBytes) * progress)
        } else {
            bytes = downloadedBytes
        }
        return String(format: "%.1f", Double(max(bytes, 0)) / 1_073_741_824.0)
    }

    var totalGB: String {
        String(format: "%.1f", Double(effectiveTotalBytes) / 1_073_741_824.0)
    }

    // UI: percentage with one decimal place (e.g., 3.4%)
    var percent1dp: String {
        String(format: "%.1f%%", progress * 100.0)
    }

    // UI: downloaded GB with two decimals (numerator precision)
    var downloadedGB2dp: String {
        let bytes: Int64
        if looksLikeFileCount || downloadedBytes == 0 {
            bytes = Int64(Double(effectiveTotalBytes) * progress)
        } else {
            bytes = downloadedBytes
        }
        return String(format: "%.2f", Double(max(bytes, 0)) / 1_073_741_824.0)
    }

    // UI: total GB with one decimal and no space before unit
    var totalGBNoSpaceUnit: String {
        String(format: "%.1fGB", Double(effectiveTotalBytes) / 1_073_741_824.0)
    }

    func refreshInstalled() async {
        let info = await manager.verifyLocalSnapshot()
        if info.ok {
            await MainActor.run {
                self.totalBytes = info.totalBytes
                self.downloadedBytes = info.totalBytes
                self.progress = 1.0
                self.status = .ready
            }
        } else if info.shardCount > 0 || info.totalBytes > 0 {
            // Partial snapshot — auto-resume download
            await MainActor.run {
                self.totalBytes = self.fallbackTotalBytes
                self.downloadedBytes = info.totalBytes
                self.progress = min(0.99, Double(info.totalBytes) / Double(self.fallbackTotalBytes))
                self.status = .downloading
            }
            print("[GPTOSS UI] Partial snapshot detected; auto-resuming download")
            await startDownload(preserveExisting: true)
        }
    }

    var hasSufficientSpace: Bool { (freeDiskBytes() ?? 0) >= requiredFreeBytes }

    func startDownload(preserveExisting: Bool = false) async {
        guard !isDownloading else { return }
        status = .downloading
        if !preserveExisting {
            progress = 0
            downloadedBytes = 0
            totalBytes = 0
        }
        paused = false
        let token = UUID()
        currentToken = token

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
                await MainActor.run {
                    // Only mark ready if this completion belongs to current token and user didn't pause
                    if self.currentToken == token && !self.paused {
                        // Ensure UI reflects completion even if no byte totals were reported
                        if self.progress < 1.0 { self.progress = 1.0 }
                        if self.totalBytes == 0 { self.totalBytes = self.fallbackTotalBytes }
                        if self.downloadedBytes < self.totalBytes {
                            self.downloadedBytes = self.totalBytes
                        }
                        print("[GPTOSS UI] Download complete; marking Ready. totalBytes=\\(self.totalBytes) downloaded=\\(self.downloadedBytes)")
                        self.status = .ready
                    } else {
                        print("[GPTOSS UI] Download finished but UI token changed or paused; ignoring ready state.")
                    }
                }
            } catch {
                await MainActor.run {
                    if self.paused {
                        self.status = .paused
                    } else {
                        self.status = .error(error.localizedDescription)
                    }
                }
            }
        }
    }

    func pauseDownload() {
        print("[GPTOSS UI] Pause requested")
        paused = true
        task?.cancel()
        status = .paused
    }

    func cancelDownload() {
        print("[GPTOSS UI] Cancel requested")
        paused = false
        task?.cancel()
        currentToken = nil
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
