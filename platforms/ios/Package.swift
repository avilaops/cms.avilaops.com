// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AvilaAADRuntime",
    platforms: [.iOS(.v16), .macOS(.v13)],
    products: [.library(name: "AvilaAADRuntime", targets: ["AvilaAADRuntime"])],
    targets: [
        .target(name: "AvilaAADRuntime"),
        .testTarget(name: "AvilaAADRuntimeTests", dependencies: ["AvilaAADRuntime"])
    ]
)
