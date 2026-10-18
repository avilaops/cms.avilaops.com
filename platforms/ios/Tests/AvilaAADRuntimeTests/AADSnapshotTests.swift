#if os(macOS)
import AppKit
import SwiftUI
import XCTest
@testable import AvilaAADRuntime

@MainActor
final class AADSnapshotTests: XCTestCase {
    func testWritesNativeReadyAndErrorScreenshots() throws {
        let output = try screenshotDirectory()
        let app = try AADDefinitionDecoder().decode(TestFixture.validData)

        try write(AADRuntimeContentView(state: .ready(app), onRetry: {}), named: "runtime-ready", to: output)
        try write(AADRuntimeContentView(state: .failed(AADRuntimeStore.safeFailureMessage), onRetry: {}), named: "runtime-error", to: output)

        for name in ["runtime-ready.png", "runtime-error.png"] {
            let data = try Data(contentsOf: output.appendingPathComponent(name))
            XCTAssertGreaterThan(data.count, 1_000)
        }
    }

    private func screenshotDirectory() throws -> URL {
        let environment = ProcessInfo.processInfo.environment["AAD_SCREENSHOT_OUTPUT"]
        let url = environment.map { URL(fileURLWithPath: $0, isDirectory: true) }
            ?? FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func write<V: View>(_ view: V, named name: String, to directory: URL) throws {
        let content = view.frame(width: 390, height: 844).background(Color(nsColor: .windowBackgroundColor))
        let renderer = ImageRenderer(content: content)
        renderer.scale = 2
        guard let image = renderer.nsImage,
              let tiff = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:]) else {
            throw SnapshotError.renderingFailed
        }
        try png.write(to: directory.appendingPathComponent("\(name).png"), options: .atomic)
    }
}

private enum SnapshotError: Error { case renderingFailed }
#endif
