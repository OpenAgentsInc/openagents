// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "lev-bridge",
    platforms: [.macOS(.v26)],
    targets: [
        .executableTarget(name: "lev-bridge", path: "Sources/lev-bridge")
    ]
)
