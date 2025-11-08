// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "OpenAgentsCore",
    platforms: [
        .macOS(.v13), .iOS(.v16)
    ],
    products: [
        .library(name: "OpenAgentsCore", targets: ["OpenAgentsCore"]),
        .library(name: "OpenAgentsNostr", targets: ["OpenAgentsNostr"]) // relinked
    ],
    dependencies: [
        // Local path to nostr-sdk-ios (editable and updatable without re-importing)
        .package(path: "/Users/christopherdavid/code/nostr-sdk-ios"),
    ],
    targets: [
        .target(
            name: "OpenAgentsCore",
            dependencies: [
                .target(name: "OpenAgentsNostr")
            ]
        ),
        .target(
            name: "OpenAgentsNostr",
            dependencies: [
                .product(name: "NostrSDK", package: "nostr-sdk-ios")
            ]
        ),
        .testTarget(name: "OpenAgentsCoreTests", dependencies: ["OpenAgentsCore"]),
    ]
)
