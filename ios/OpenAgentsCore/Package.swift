// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "OpenAgentsCore",
    platforms: [
        .macOS(.v13), .iOS(.v16)
    ],
    products: [
        .library(name: "OpenAgentsCore", targets: ["OpenAgentsCore"]),
    ],
    targets: [
        .target(name: "OpenAgentsCore"),
        .testTarget(name: "OpenAgentsCoreTests", dependencies: ["OpenAgentsCore"]),
    ]
)
