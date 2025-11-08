// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "OpenAgentsCore",
    platforms: [
        .macOS(.v13), .iOS(.v16)
    ],
    products: [
        .library(name: "OpenAgentsCore", targets: ["OpenAgentsCore"]),
        // macOS-only wrapper around Nostr SDK (transitively used by OpenAgentsCore on macOS only)
        .library(name: "OpenAgentsNostr", targets: ["OpenAgentsNostr"]),
    ],
    dependencies: [
        // Local path to nostr-sdk-ios (editable and updatable without re-importing)
        .package(path: "/Users/christopherdavid/code/nostr-sdk-ios"),
    ],
    targets: [
        .target(
            name: "OpenAgentsCore",
            dependencies: [
                // Only pull in the Nostr wrapper when building for macOS
                .target(name: "OpenAgentsNostr", condition: .when(platforms: [.macOS]))
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
