// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "OpenAgentsCore",
    platforms: [
        .macOS(.v14), .iOS(.v16)
    ],
    products: [
        .library(name: "OpenAgentsCore", targets: ["OpenAgentsCore"]),
        .library(name: "OpenAgentsNostr", targets: ["OpenAgentsNostr"]) // relinked
    ],
    dependencies: [
        // Local path to nostr-sdk-ios (editable and updatable without re-importing)
        .package(path: "/Users/christopherdavid/code/nostr-sdk-ios"),

        // MLX Swift Examples for embeddings support (BGE-small model via MLX)
        .package(url: "https://github.com/ml-explore/mlx-swift-examples.git", from: "2.29.0"),
    ],
    targets: [
        .target(
            name: "OpenAgentsCore",
            dependencies: [
                .target(name: "OpenAgentsNostr"),
                // MLX embeddings library (macOS-only code uses #if os(macOS))
                .product(name: "MLXEmbedders", package: "mlx-swift-examples"),
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
