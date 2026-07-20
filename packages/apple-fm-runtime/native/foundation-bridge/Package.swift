// swift-tools-version:6.2

import PackageDescription

let package = Package(
    name: "foundation-bridge",
    platforms: [
        .macOS(.v26)
    ],
    products: [
        .executable(name: "foundation-bridge", targets: ["foundation-bridge"])
    ],
    targets: [
        .executableTarget(
            name: "foundation-bridge",
            path: "Sources/foundation-bridge",
            swiftSettings: [
                .unsafeFlags(["-parse-as-library"])
            ]
        )
    ]
)
