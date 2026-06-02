// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AskHuman",
    platforms: [
        .macOS(.v13)
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-markdown.git", branch: "main")
    ],
    targets: [
        .executableTarget(
            name: "AskHuman",
            dependencies: [
                .product(name: "Markdown", package: "swift-markdown")
            ],
            path: "Sources/AskHuman"
        ),
        .testTarget(
            name: "AskHumanTests",
            dependencies: ["AskHuman"],
            path: "Tests/AskHumanTests"
        )
    ]
)
