// swift-tools-version: 6.2
import PackageDescription

// The bridge to Apple's on-device foundation model (FoundationModels). It is
// Swift because the framework has only a Swift API; everything that uses it
// is Coax (psionic/lib/apple-fm.coax).
let package = Package(
    name: "apple-fm-bridge",
    platforms: [.macOS(.v26)],
    targets: [
        .executableTarget(name: "apple-fm-bridge", path: "Sources/apple-fm-bridge")
    ]
)
