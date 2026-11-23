// Firm reason: 6 (cross-Apple 16 kHz mono Int16 PCM microphone bridge over authenticated Tailscale TCP feeding Antigravity host `agy mic-serve` / `ANTIGRAVITY_MIC`, #14018).
import AVFoundation
import Foundation
import Network

/// Streams 16 kHz mono Int16 PCM (~32 kB/s) over Tailscale TCP to host `agy mic-serve` (default port 4713).
/// Enforces Antigravity's strict safety rule: voice cannot approve destructive commands (`canApproveDestructive = false`).
@MainActor
final class AntigravityMicBridge: ObservableObject {
    @Published var isStreaming: Bool = false
    @Published var audioLevel: Double = 0.0
    @Published var statusLabel: String = "Idle"
    @Published var lastError: String?

    private var engine: AVAudioEngine?
    private var connection: NWConnection?
    private let sendQueue = DispatchQueue(label: "com.openagents.agy.micbridge")

    func toggle(host: String, port: UInt16 = 4713, hasPendingApproval: Bool) {
        if isStreaming {
            stop()
            return
        }
        if hasPendingApproval {
            lastError = "Voice bridge paused while an approval prompt is pending. Destructive approvals require an explicit tap."
            return
        }
        start(host: host, port: port)
    }

    func start(host: String, port: UInt16 = 4713) {
        stop()
        lastError = nil
        let targetHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !targetHost.isEmpty, let nwPort = NWEndpoint.Port(rawValue: port) else {
            lastError = "No reachable Tailscale host for agy mic-serve."
            return
        }

        let conn = NWConnection(host: NWEndpoint.Host(targetHost), port: nwPort, using: .tcp)
        self.connection = conn
        statusLabel = "Connecting to \(targetHost):\(port)…"

        conn.stateUpdateHandler = { [weak self] state in
            Task { @MainActor [weak self] in
                guard let self else { return }
                switch state {
                case .ready:
                    self.statusLabel = "Streaming 16 kHz PCM → \(targetHost):\(port)"
                    self.startAudioCapture()
                case .failed(let err):
                    self.lastError = "Mic bridge failed: \(err.localizedDescription)"
                    self.stop()
                case .cancelled:
                    self.statusLabel = "Stopped"
                default:
                    break
                }
            }
        }
        conn.start(queue: sendQueue)
    }

    private func startAudioCapture() {
        #if os(iOS)
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.defaultToSpeaker, .allowBluetooth])
            try session.setActive(true)
        } catch {
            lastError = "Audio session unavailable: \(error.localizedDescription)"
            stop()
            return
        }
        #endif

        let eng = AVAudioEngine()
        let input = eng.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard let targetFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000, channels: 1, interleaved: true),
              let converter = AVAudioConverter(from: inputFormat, to: targetFormat) else {
            lastError = "Could not configure 16 kHz mono Int16 PCM converter."
            stop()
            return
        }

        let conn = self.connection
        input.installTap(onBus: 0, bufferSize: 1024, format: inputFormat) { [weak self] buffer, _ in
            guard let self, let conn else { return }
            let ratio = 16_000.0 / max(1.0, inputFormat.sampleRate)
            let frameCap = AVAudioFrameCount(max(256, Int(Double(buffer.frameLength) * ratio) + 64))
            guard let converted = AVAudioPCMBuffer(pcmFormat: targetFormat, frameCapacity: frameCap) else { return }

            var error: NSError?
            var consumed = false
            converter.convert(to: converted, error: &error) { _, outStatus in
                if consumed {
                    outStatus.pointee = .noDataNow
                    return nil
                }
                consumed = true
                outStatus.pointee = .haveData
                return buffer
            }

            let frames = Int(converted.frameLength)
            guard frames > 0, let ptr = converted.int16ChannelData?[0] else { return }
            let pcmData = Data(bytes: ptr, count: frames * MemoryLayout<Int16>.size)

            // Compute normalized RMS audio level for meter feedback
            var sumSq: Double = 0
            for i in 0..<frames {
                let s = Double(ptr[i]) / 32768.0
                sumSq += s * s
            }
            let rms = min(1.0, sqrt(sumSq / Double(frames)) * 4.0)
            Task { @MainActor [weak self] in
                self?.audioLevel = rms
            }

            conn.send(content: pcmData, completion: .idempotent)
        }

        do {
            eng.prepare()
            try eng.start()
            self.engine = eng
            self.isStreaming = true
        } catch {
            lastError = "Microphone capture failed: \(error.localizedDescription)"
            stop()
        }
    }

    func stop() {
        if let eng = engine {
            eng.inputNode.removeTap(onBus: 0)
            eng.stop()
        }
        engine = nil
        connection?.cancel()
        connection = nil
        isStreaming = false
        audioLevel = 0
        if lastError == nil {
            statusLabel = "Idle"
        }
    }
}
