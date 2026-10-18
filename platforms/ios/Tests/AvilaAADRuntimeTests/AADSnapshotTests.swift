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
        let content = view
            .frame(width: 390, height: 844)
            .background(Color(nsColor: .windowBackgroundColor))
            .environment(\.colorScheme, .light)
        let hostingView = NSHostingView(rootView: content)
        hostingView.frame = NSRect(x: 0, y: 0, width: 390, height: 844)
        hostingView.layoutSubtreeIfNeeded()

        guard let bitmap = hostingView.bitmapImageRepForCachingDisplay(in: hostingView.bounds) else {
            throw SnapshotError.renderingFailed
        }
        hostingView.cacheDisplay(in: hostingView.bounds, to: bitmap)
        guard containsVisibleContent(bitmap),
              let png = bitmap.representation(using: .png, properties: [:]) else {
            throw SnapshotError.blankRendering
        }
        try png.write(to: directory.appendingPathComponent("\(name).png"), options: .atomic)
    }

    private func containsVisibleContent(_ bitmap: NSBitmapImageRep) -> Bool {
        for x in stride(from: 0, to: bitmap.pixelsWide, by: 8) {
            for y in stride(from: 0, to: bitmap.pixelsHigh, by: 8) {
                guard let color = bitmap.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB) else { continue }
                let luminance = 0.2126 * color.redComponent
                    + 0.7152 * color.greenComponent
                    + 0.0722 * color.blueComponent
                if luminance < 0.75 { return true }
            }
        }
        return false
    }
}

private enum SnapshotError: Error { case renderingFailed, blankRendering }
#endif
