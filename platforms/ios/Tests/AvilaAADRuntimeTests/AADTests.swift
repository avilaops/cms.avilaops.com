import Foundation
import XCTest
@testable import AvilaAADRuntime

final class AADTests: XCTestCase {
    private let valid = """
    {
      "schemaVersion":"1.0.0",
      "appId":"app-exemplo",
      "minimumRuntimeVersion":"1.0.0",
      "navigation":{"initialRoute":"home"},
      "screens":[
        {"id":"home","route":"home","title":"Início","components":[
          {"type":"text","id":"intro","value":"Bem-vindo"},
          {"type":"button","id":"go-orders","label":"Pedidos","action":{"type":"navigate","target":"orders"}}
        ]},
        {"id":"orders","route":"orders","title":"Pedidos","components":[]}
      ]
    }
    """

    func testDecodesMatchingRustFixture() throws {
        let app = try AADDefinitionDecoder().decode(Data(valid.utf8))
        XCTAssertEqual(app.screens.count, 2)
        XCTAssertEqual(app.navigation.initialRoute, "home")
    }

    func testAcceptsPayloadAtByteLimit() throws {
        let source = Data(valid.utf8)
        let padded = source + Data(repeating: 0x20, count: AADDefinitionDecoder.maximumBytes - source.count)
        XCTAssertNoThrow(try AADDefinitionDecoder().decode(padded))
    }

    func testRejectsPayloadOneByteAboveLimitBeforeDecoding() {
        let oversized = Data(repeating: 0x20, count: AADDefinitionDecoder.maximumBytes + 1)
        XCTAssertThrowsError(try AADDefinitionDecoder().decode(oversized)) { error in
            XCTAssertEqual(error as? AADError, .payloadTooLarge)
        }
    }

    func testRejectsUnknownFieldsAtEveryObjectLevel() {
        let mutations = [
            valid.replacingOccurrences(of: "\"screens\":[", with: "\"unknown\":true,\"screens\":["),
            valid.replacingOccurrences(of: "\"initialRoute\":\"home\"", with: "\"initialRoute\":\"home\",\"unknown\":true"),
            valid.replacingOccurrences(of: "\"title\":\"Início\"", with: "\"title\":\"Início\",\"unknown\":true"),
            valid.replacingOccurrences(of: "\"value\":\"Bem-vindo\"", with: "\"value\":\"Bem-vindo\",\"unknown\":true"),
            valid.replacingOccurrences(of: "\"target\":\"orders\"", with: "\"target\":\"orders\",\"unknown\":true")
        ]
        for mutation in mutations {
            XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(mutation.utf8)), mutation)
        }
    }

    func testRejectsDuplicateJSONKeysAtEveryObjectLevel() {
        let mutations = [
            valid.replacingOccurrences(of: "\"appId\":\"app-exemplo\"", with: "\"appId\":\"app-exemplo\",\"appId\":\"outro-app\""),
            valid.replacingOccurrences(of: "\"initialRoute\":\"home\"", with: "\"initialRoute\":\"home\",\"initialRoute\":\"orders\""),
            valid.replacingOccurrences(of: "\"title\":\"Início\"", with: "\"title\":\"Início\",\"title\":\"Outra\""),
            valid.replacingOccurrences(of: "\"value\":\"Bem-vindo\"", with: "\"value\":\"Bem-vindo\",\"value\":\"Outro\""),
            valid.replacingOccurrences(of: "\"target\":\"orders\"", with: "\"target\":\"orders\",\"target\":\"home\"")
        ]
        for mutation in mutations {
            XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(mutation.utf8))) { error in
                XCTAssertEqual(error as? AADError, .duplicateJSONKey)
            }
        }
    }

    func testRejectsUnknownTarget() {
        let changed = valid.replacingOccurrences(of: "\"target\":\"orders\"", with: "\"target\":\"missing\"")
        XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(changed.utf8))) { error in
            XCTAssertEqual(error as? AADError, .route)
        }
    }

    func testRejectsUnsupportedComponent() {
        let changed = valid.replacingOccurrences(of: "\"type\":\"text\"", with: "\"type\":\"script\"")
        XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(changed.utf8)))
    }

    func testRejectsNonASCIIAndOverlongIdentifiers() {
        let nonASCII = valid.replacingOccurrences(of: "\"app-exemplo\"", with: "\"aplicação\"")
        let overlong = valid.replacingOccurrences(of: "\"app-exemplo\"", with: "\"a\(String(repeating: "b", count: 64))\"")
        for mutation in [nonASCII, overlong] {
            XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(mutation.utf8))) { error in
                XCTAssertEqual(error as? AADError, .identifier)
            }
        }
    }

    func testRejectsDuplicateScreenRouteAndComponentIdentifier() {
        let duplicateRoute = valid.replacingOccurrences(of: "\"route\":\"orders\"", with: "\"route\":\"home\"")
        let duplicateComponent = valid.replacingOccurrences(of: "\"id\":\"go-orders\"", with: "\"id\":\"intro\"")
        for mutation in [duplicateRoute, duplicateComponent] {
            XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(mutation.utf8))) { error in
                XCTAssertEqual(error as? AADError, .duplicate)
            }
        }
    }

    func testPublicModelCannotBypassDefinitionValidation() {
        XCTAssertFalse(AADApp.self is Decodable.Type)
    }

    func testScreenCountBoundaries() throws {
        XCTAssertNoThrow(try decodeApp(screens: makeScreens(count: 100)))

        for count in [0, 101] {
            XCTAssertThrowsError(try decodeApp(screens: makeScreens(count: count))) { error in
                XCTAssertEqual(error as? AADError, .limit)
            }
        }
    }

    func testComponentCountBoundaries() throws {
        XCTAssertNoThrow(try decodeApp(screens: [makeScreen(index: 0, componentCount: 200)]))

        XCTAssertThrowsError(
            try decodeApp(screens: [makeScreen(index: 0, componentCount: 201)])
        ) { error in
            XCTAssertEqual(error as? AADError, .limit)
        }
    }

    func testRejectsUnsupportedSchemaAndRuntimeVersions() {
        let mutations = [
            valid.replacingOccurrences(of: "\"schemaVersion\":\"1.0.0\"", with: "\"schemaVersion\":\"2.0.0\""),
            valid.replacingOccurrences(of: "\"minimumRuntimeVersion\":\"1.0.0\"", with: "\"minimumRuntimeVersion\":\"2.0.0\"")
        ]

        for mutation in mutations {
            XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(mutation.utf8))) { error in
                XCTAssertEqual(error as? AADError, .version)
            }
        }
    }

    func testRejectsDuplicateScreenIdentifiersAndUnknownInitialRoute() {
        let duplicateIdentifier = valid.replacingOccurrences(of: "\"id\":\"orders\"", with: "\"id\":\"home\"")
        let unknownInitialRoute = valid.replacingOccurrences(of: "\"initialRoute\":\"home\"", with: "\"initialRoute\":\"missing\"")

        XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(duplicateIdentifier.utf8))) { error in
            XCTAssertEqual(error as? AADError, .duplicate)
        }
        XCTAssertThrowsError(try AADDefinitionDecoder().decode(Data(unknownInitialRoute.utf8))) { error in
            XCTAssertEqual(error as? AADError, .route)
        }
    }

    private func decodeApp(screens: [String]) throws -> AADApp {
        let json = """
        {
          "schemaVersion":"1.0.0",
          "appId":"app-exemplo",
          "minimumRuntimeVersion":"1.0.0",
          "navigation":{"initialRoute":"screen-0"},
          "screens":[\(screens.joined(separator: ","))]
        }
        """
        return try AADDefinitionDecoder().decode(Data(json.utf8))
    }

    private func makeScreens(count: Int) -> [String] {
        (0..<count).map { makeScreen(index: $0, componentCount: 0) }
    }

    private func makeScreen(index: Int, componentCount: Int) -> String {
        let components = (0..<componentCount).map {
            "{\"type\":\"spacer\",\"id\":\"component-\($0)\"}"
        }
        return "{\"id\":\"screen-\(index)\",\"route\":\"screen-\(index)\",\"title\":\"Tela\",\"components\":[\(components.joined(separator: ","))]}"
    }
}
