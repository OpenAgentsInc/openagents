// Firm reason: 6 (cross-Apple CoreImage QR pairing generator + iOS AVCaptureSession camera QR scanner for zero-typing openagents://pair Tailscale host pairing, #14019).
import CoreImage
import CoreImage.CIFilterBuiltins
import SwiftUI

#if os(iOS)
import AVFoundation
import UIKit
#endif

/// Generates an `openagents://pair?...` deep-link URL and crisp CoreImage QR code for zero-typing iOS pairing.
struct PairingQRCodeView: View {
    let address: String
    let port: UInt16
    let name: String
    @State private var copied = false

    var pairingURLString: String {
        var comps = URLComponents()
        comps.scheme = "openagents"
        comps.host = "pair"
        comps.queryItems = [
            URLQueryItem(name: "address", value: address),
            URLQueryItem(name: "port", value: String(port)),
            URLQueryItem(name: "name", value: name)
        ]
        return comps.string ?? "openagents://pair?address=\(address)&port=\(port)"
    }

    private func makeQRImage(from text: String) -> CGImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        filter.setValue(Data(text.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        return context.createCGImage(scaled, from: scaled.extent)
    }

    var body: some View {
        HStack(spacing: 14) {
            if let cg = makeQRImage(from: pairingURLString) {
                Image(decorative: cg, scale: 1.0)
                    .interpolation(.none)
                    .resizable()
                    .frame(width: 112, height: 112)
                    .padding(8)
                    .background(Color.white, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            }
            VStack(alignment: .leading, spacing: 6) {
                Text("Scan from OpenAgents iOS")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Text("Pairs directly over your private Tailscale network on port \(port) with zero typing.")
                    .font(.system(size: 11.5))
                    .foregroundStyle(OATheme.textSecondary)
                Text(pairingURLString)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(OATheme.textMuted)
                    .lineLimit(2)
                    .textSelection(.enabled)
                Button {
                    OAClipboard.copy(pairingURLString)
                    copied = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { copied = false }
                } label: {
                    Label(copied ? "Copied Link" : "Copy Pairing Link", systemImage: copied ? "checkmark" : "link")
                        .font(.system(size: 11.5, weight: .medium))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(OATheme.elevated, in: Capsule())
                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
            }
            Spacer()
        }
        .padding(12)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }
}

#if os(iOS)
/// Camera-based QR code scanner sheet on iOS for `openagents://pair?...` deep links (#14019).
struct QRScannerSheetView: View {
    let onScannedURL: (URL) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var cameraError: String?
    @State private var pastedLink: String = ""

    var body: some View {
        NavigationStack {
            VStack(spacing: 14) {
                if let cameraError {
                    VStack(spacing: 10) {
                        Image(systemName: "camera.fill")
                            .font(.system(size: 28))
                            .foregroundStyle(OATheme.textMuted)
                        Text(cameraError)
                            .font(.system(size: 13))
                            .foregroundStyle(OATheme.textSecondary)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 24)
                    }
                    .frame(maxWidth: .infinity, minHeight: 220)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                } else {
                    ZStack {
                        CameraQRPreview(
                            onCodeFound: { raw in
                                if let url = URL(string: raw), url.scheme?.lowercased() == "openagents" {
                                    UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                                    onScannedURL(url)
                                    dismiss()
                                }
                            },
                            onError: { err in
                                cameraError = err
                            }
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .stroke(Color.white.opacity(0.8), lineWidth: 2)
                            .frame(width: 200, height: 200)
                    }
                    .frame(height: 300)
                }

                Text("Point your iPhone camera at the QR code in OpenAgents macOS Host Settings, or paste an `openagents://pair?...` link below.")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textSecondary)
                    .multilineTextAlignment(.center)

                HStack(spacing: 8) {
                    TextField("openagents://pair?address=…", text: $pastedLink)
                        .font(.system(size: 12.5, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .padding(10)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                        .accessibilityIdentifier("qr.pasteField")

                    Button("Pair") {
                        let trimmed = pastedLink.trimmingCharacters(in: .whitespacesAndNewlines)
                        if let url = URL(string: trimmed) {
                            onScannedURL(url)
                            dismiss()
                        }
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 9)
                    .background(Color.white, in: Capsule())
                    .disabled(pastedLink.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("qr.pasteSubmit")
                }

                Spacer()
            }
            .padding(16)
            .background(OATheme.background)
            .navigationTitle("Scan Mac QR Code")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
        .preferredColorScheme(.dark)
    }
}

private struct CameraQRPreview: UIViewRepresentable {
    let onCodeFound: (String) -> Void
    let onError: (String) -> Void

    final class Coordinator: NSObject, AVCaptureMetadataOutputObjectsDelegate {
        let onCodeFound: (String) -> Void
        var session: AVCaptureSession?
        var delivered = false

        init(onCodeFound: @escaping (String) -> Void) {
            self.onCodeFound = onCodeFound
        }

        func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
            guard !delivered,
                  let obj = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
                  obj.type == .qr,
                  let val = obj.stringValue else { return }
            delivered = true
            session?.stopRunning()
            DispatchQueue.main.async { self.onCodeFound(val) }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(onCodeFound: onCodeFound) }

    func makeUIView(context: Context) -> UIView {
        let container = UIView()
        container.backgroundColor = .black

        let session = AVCaptureSession()
        context.coordinator.session = session

        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else {
            DispatchQueue.main.async {
                onError("Camera unavailable on this device (e.g. Simulator). Paste the openagents://pair link below instead.")
            }
            return container
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            DispatchQueue.main.async { onError("Unable to initialize QR metadata scanner.") }
            return container
        }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(context.coordinator, queue: DispatchQueue.main)
        output.metadataObjectTypes = [.qr]

        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = CGRect(x: 0, y: 0, width: 340, height: 300)
        container.layer.addSublayer(preview)

        DispatchQueue.global(qos: .userInitiated).async {
            session.startRunning()
        }
        return container
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        if let preview = uiView.layer.sublayers?.first as? AVCaptureVideoPreviewLayer {
            preview.frame = uiView.bounds
        }
    }

    static func dismantleUIView(_ uiView: UIView, coordinator: Coordinator) {
        coordinator.session?.stopRunning()
    }
}
#endif
