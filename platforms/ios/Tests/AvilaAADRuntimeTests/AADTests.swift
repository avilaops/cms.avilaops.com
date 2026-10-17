import Foundation
import XCTest
@testable import AvilaAADRuntime

final class AADTests: XCTestCase {
    let valid = """
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
        let app = try AADApp(data: Data(valid.utf8))
        XCTAssertEqual(app.screens.count, 2)
        XCTAssertEqual(app.navigation.initialRoute, "home")
    }

    func testRejectsUnknownTarget() {
        let changed = valid.replacingOccurrences(of: "\"target\":\"orders\"", with: "\"target\":\"missing\"")
        XCTAssertThrowsError(try AADApp(data: Data(changed.utf8)))
    }

    func testRejectsUnsupportedComponent() {
        let changed = valid.replacingOccurrences(of: "\"type\":\"text\"", with: "\"type\":\"script\"")
        XCTAssertThrowsError(try AADApp(data: Data(changed.utf8)))
    }
}
