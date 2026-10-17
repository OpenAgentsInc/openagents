import AVFoundation
import UIKit

/// The camera screen that reads a QR code (#12000): a computer's connect
/// code, or a website sign-in code from openagents.com. It only reads text;
/// Rust decides what the text is (`computerCodeProblem`, `isWebsiteLogin`)
/// and does the pairing, linking, and signing. The person can paste a code
/// instead when the camera isn't available.
final class CodeScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    /// A code Rust accepted. The scanner has stopped; the caller dismisses it.
    var onCode: ((String) -> Void)?

    private let session = AVCaptureSession()
    private let sessionQueue = DispatchQueue(label: "com.openagents.app.scanner")
    private var preview: AVCaptureVideoPreviewLayer?
    private let hint = UILabel()
    private let problem = UILabel()
    private var done = false
    private var lastProblem: String?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        title = "Scan a code"
        view.accessibilityIdentifier = "code-scanner"
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Paste", primaryAction: UIAction { [weak self] _ in
            self?.paste()
        })

        hint.text = "Point your camera at the code on your computer's screen."
        hint.font = Fonts.ui(.sansMedium, 16)
        hint.textColor = .white
        hint.numberOfLines = 0
        hint.textAlignment = .center
        problem.font = Fonts.ui(.sans, 15)
        problem.textColor = .white
        problem.backgroundColor = UIColor.black.withAlphaComponent(0.6)
        problem.layer.cornerRadius = 10
        problem.layer.masksToBounds = true
        problem.numberOfLines = 0
        problem.textAlignment = .center
        problem.isHidden = true
        problem.accessibilityIdentifier = "code-scanner-problem"
        let stack = UIStackView(arrangedSubviews: [problem, hint])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        let guide = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -24),
            stack.bottomAnchor.constraint(equalTo: guide.bottomAnchor, constant: -24),
        ])
        startCamera()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        let session = session
        sessionQueue.async { if session.isRunning { session.stopRunning() } }
    }

    private func startCamera() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: configure()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                DispatchQueue.main.async {
                    if granted { self?.configure() } else { self?.cameraOff() }
                }
            }
        default: cameraOff()
        }
    }

    private func cameraOff() {
        hint.text = "The camera is off for OpenAgents. Turn it on in iOS Settings, or copy the code on your computer and tap Paste."
    }

    private func configure() {
        guard let camera = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: camera),
              session.canAddInput(input)
        else {
            hint.text = "This device has no camera. Copy the code on your computer and tap Paste."
            return
        }
        session.addInput(input)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = output.availableMetadataObjectTypes.contains(.qr) ? [.qr] : []
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.insertSublayer(layer, at: 0)
        preview = layer
        let session = session
        sessionQueue.async { session.startRunning() }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        for object in objects {
            guard let code = (object as? AVMetadataMachineReadableCodeObject)?.stringValue else { continue }
            take(code)
            if done { return }
        }
    }

    private func paste() {
        guard let text = UIPasteboard.general.string, !text.isEmpty else {
            show("Copy the code on your computer first, then tap Paste.")
            return
        }
        take(text)
    }

    /// Rust decides whether this is a code worth acting on; nothing is
    /// dialed for anything else.
    private func take(_ text: String) {
        guard !done else { return }
        if isWebsiteLogin(text: text) || computerCodeProblem(text: text) == nil {
            done = true
            UINotificationFeedbackGenerator().notificationOccurred(.success)
            let session = session
            sessionQueue.async { if session.isRunning { session.stopRunning() } }
            onCode?(text)
            return
        }
        show(computerCodeProblem(text: text) ?? "This isn't a code from OpenAgents.")
    }

    private func show(_ message: String) {
        guard message != lastProblem else { return }
        lastProblem = message
        problem.text = "  \(message)  "
        problem.isHidden = false
    }
}

/// What a scanned or opened code does (#12000, #12001): connect a computer
/// and sign it in as this account, or sign in on the website after the
/// person confirms which browser. Rust does the work; this shows progress
/// and the result.
@MainActor
enum CodeAction {
    /// Open the scanner over `presenter`.
    static func scan(from presenter: UIViewController, app: AppModel, done: (() -> Void)? = nil) {
        let scanner = CodeScannerViewController()
        let nav = UINavigationController(rootViewController: scanner)
        nav.modalPresentationStyle = .fullScreen
        scanner.onCode = { [weak nav] code in
            nav?.dismiss(animated: true) {
                run(code, from: presenter, app: app, done: done)
            }
        }
        presenter.present(nav, animated: true)
    }

    /// Act on a code from the scanner or a universal link.
    static func run(_ code: String, from presenter: UIViewController, app: AppModel, done: (() -> Void)? = nil) {
        if isWebsiteLogin(text: code) {
            websiteLogin(code, from: presenter, app: app, done: done)
        } else {
            connect(code, from: presenter, app: app, done: done)
        }
    }

    private static func connect(_ code: String, from presenter: UIViewController, app: AppModel, done: (() -> Void)?) {
        let progress = UIAlertController(title: "Connecting your computer…", message: nil, preferredStyle: .alert)
        presenter.present(progress, animated: true)
        Task { @MainActor in
            let title: String
            let message: String?
            do {
                let connected = try await app.connectComputer(code: code)
                title = "\(connected.name) is connected"
                message = connected.linkNote ?? "It's signed in as you. Its sessions show up here."
            } catch {
                title = "Couldn't connect your computer"
                message = error.localizedDescription
            }
            progress.dismiss(animated: true) {
                let result = UIAlertController(title: title, message: message, preferredStyle: .alert)
                result.addAction(UIAlertAction(title: "OK", style: .default) { _ in done?() })
                presenter.present(result, animated: true)
            }
        }
    }

    private static func websiteLogin(_ code: String, from presenter: UIViewController, app: AppModel, done: (() -> Void)?) {
        Task { @MainActor in
            let browser: String
            do {
                browser = try await app.scannedLogin(code: code)
            } catch {
                return fail("Couldn't sign in on the website", error, from: presenter, done: done)
            }
            let ask = UIAlertController(title: "Sign in to openagents.com on \(browser)?", message: "That browser will use your account.", preferredStyle: .alert)
            ask.addAction(UIAlertAction(title: "Not now", style: .cancel) { _ in done?() })
            ask.addAction(UIAlertAction(title: "Sign in", style: .default) { _ in
                Task { @MainActor in
                    do {
                        let signedIn = try await app.approveScannedLogin(code: code)
                        let result = UIAlertController(title: "Signed in on \(signedIn)", message: nil, preferredStyle: .alert)
                        result.addAction(UIAlertAction(title: "OK", style: .default) { _ in done?() })
                        presenter.present(result, animated: true)
                    } catch {
                        fail("Couldn't sign in on the website", error, from: presenter, done: done)
                    }
                }
            })
            presenter.present(ask, animated: true)
        }
    }

    private static func fail(_ title: String, _ error: Error, from presenter: UIViewController, done: (() -> Void)?) {
        let alert = UIAlertController(title: title, message: error.localizedDescription, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default) { _ in done?() })
        presenter.present(alert, animated: true)
    }
}
