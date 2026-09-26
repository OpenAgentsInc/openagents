// Camera ownership is native. Capture starts only after an explicit Scan action
// and is stopped when its inline panel leaves the foreground or closes.
import AVFoundation
import SwiftUI
import UIKit

private final class CameraWorker: NSObject, AVCaptureMetadataOutputObjectsDelegate {
    let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "com.openagents.coder.invitation-camera")
    private var configured = false
    private var delivered = false
    private var completion: ((Result<String, Error>) -> Void)?

    func start(completion: @escaping (Result<String, Error>) -> Void) {
        queue.async {
            self.completion = completion
            self.delivered = false
            do {
                if !self.configured {
                    guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back) else {
                        throw QRInvitation.Failure.message("A camera is unavailable. Paste the computer's invitation instead.")
                    }
                    let input = try AVCaptureDeviceInput(device: device)
                    let output = AVCaptureMetadataOutput()
                    self.session.beginConfiguration()
                    defer { self.session.commitConfiguration() }
                    guard self.session.canAddInput(input), self.session.canAddOutput(output) else {
                        throw QRInvitation.Failure.message("The camera could not start. Paste the computer's invitation instead.")
                    }
                    self.session.addInput(input)
                    self.session.addOutput(output)
                    guard output.availableMetadataObjectTypes.contains(.qr) else {
                        self.session.removeOutput(output)
                        self.session.removeInput(input)
                        throw QRInvitation.Failure.message("This camera cannot read QR codes. Paste the invitation instead.")
                    }
                    output.setMetadataObjectsDelegate(self, queue: self.queue)
                    output.metadataObjectTypes = [.qr]
                    self.configured = true
                }
                self.session.startRunning()
            } catch { completion(.failure(error)) }
        }
    }

    func stop() {
        queue.async {
            self.completion = nil
            self.delivered = true
            if self.session.isRunning { self.session.stopRunning() }
        }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject],
                        from connection: AVCaptureConnection) {
        guard !delivered, let value = metadataObjects.compactMap({
            ($0 as? AVMetadataMachineReadableCodeObject)?.stringValue
        }).first else { return }
        delivered = true
        let callback = completion
        completion = nil
        let result = Result { try QRInvitation.bounded(value) }
        queue.async {
            if self.session.isRunning { self.session.stopRunning() }
            callback?(result)
        }
    }
}

@MainActor
private final class QRScanner: ObservableObject {
    @Published var message: String?
    @Published var denied = false
    @Published var running = false
    private let worker = CameraWorker()
    private var generation: UInt64 = 0
    private var requesting = false
    var session: AVCaptureSession { worker.session }

    func start(decoded: @escaping (String) -> Void) {
        guard !running, !requesting else { return }
        generation &+= 1
        let current = generation
        message = nil
        denied = false
        #if targetEnvironment(simulator)
        message = "The simulator has no camera. Paste a computer invitation instead."
        return
        #else
        requesting = true
        Task { [weak self] in
            let status = AVCaptureDevice.authorizationStatus(for: .video)
            let allowed: Bool
            if status == .authorized { allowed = true }
            else if status == .notDetermined { allowed = await AVCaptureDevice.requestAccess(for: .video) }
            else { allowed = false }
            guard let self, self.generation == current else { return }
            self.requesting = false
            guard allowed else {
                self.denied = true
                self.message = "Camera access is off. Enable it in Settings, or paste the invitation."
                return
            }
            self.running = true
            self.worker.start { [weak self] result in
                Task { @MainActor in
                    guard let self, self.generation == current else { return }
                    self.running = false
                    switch result {
                    case let .success(value): decoded(value)
                    case let .failure(error): self.message = error.localizedDescription
                    }
                }
            }
        }
        #endif
    }

    func stop() {
        generation &+= 1
        requesting = false
        running = false
        worker.stop()
    }
}

private struct CameraPreview: UIViewRepresentable {
    let session: AVCaptureSession
    func makeUIView(context: Context) -> PreviewView {
        let view = PreviewView()
        view.preview.session = session
        view.preview.videoGravity = .resizeAspectFill
        view.accessibilityLabel = "Camera preview for computer invitation"
        return view
    }
    func updateUIView(_ view: PreviewView, context: Context) { view.orientPortrait() }
    static func dismantleUIView(_ view: PreviewView, coordinator: ()) { view.preview.session = nil }
}

private final class PreviewView: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var preview: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }

    override func layoutSubviews() {
        super.layoutSubviews()
        orientPortrait()
    }

    func orientPortrait() {
        // Coder's phone host is portrait-only; do not assume the sensor's default orientation.
        if let connection = preview.connection, connection.isVideoRotationAngleSupported(90) {
            connection.videoRotationAngle = 90
        }
    }
}

struct InlineQRScanner: View {
    @StateObject private var scanner = QRScanner()
    @Environment(\.scenePhase) private var phase
    @Environment(\.openURL) private var openURL
    let decoded: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if scanner.running {
                CameraPreview(session: scanner.session).frame(height: 190).clipShape(RoundedRectangle(cornerRadius: 12))
                Text("Point the camera at the QR invitation on your computer.").font(.caption)
            }
            if let message = scanner.message {
                Text(message).font(.callout).accessibilityIdentifier("camera-status")
                if scanner.denied {
                    Button("Open camera settings") {
                        if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
                    }
                }
                Button("Try scanning again") { scanner.start(decoded: decoded) }
            }
        }
        .onAppear { scanner.start(decoded: decoded) }
        .onDisappear { scanner.stop() }
        .onChange(of: phase) { _, current in
            if current == .background { scanner.stop() }
            if current == .active { scanner.start(decoded: decoded) }
        }
    }
}
