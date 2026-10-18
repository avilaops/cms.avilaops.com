import Foundation
import XCTest
@testable import AvilaAADRuntime

@MainActor
final class AADRuntimeStoreTests: XCTestCase {
    func testLoadPublishesValidatedDefinition() async {
        let store = AADRuntimeStore(loader: InMemoryAADDefinitionLoader(data: TestFixture.validData))
        await store.load()
        guard case .ready(let app) = store.state else { return XCTFail("Expected ready state") }
        XCTAssertEqual(app.appId, "app-exemplo")
    }

    func testFailurePublishesSafeMessageAndRetryRecovers() async {
        let loader = SequencedLoader(results: [
            .failure(TestError.secret("token-super-secreto")),
            .success(TestFixture.validData)
        ])
        let store = AADRuntimeStore(loader: loader)

        await store.load()
        guard case .failed(let message) = store.state else { return XCTFail("Expected failed state") }
        XCTAssertEqual(message, AADRuntimeStore.safeFailureMessage)
        XCTAssertFalse(message.contains("token-super-secreto"))

        await store.retry()
        guard case .ready = store.state else { return XCTFail("Expected retry to recover") }
    }

    func testOlderResponseCannotReplaceNewerRetry() async {
        let loader = ControlledLoader()
        let store = AADRuntimeStore(loader: loader)

        let first = Task { await store.load() }
        await loader.waitUntilRequestsEqual(1)
        let second = Task { await store.retry() }
        await loader.waitUntilRequestsEqual(2)

        await loader.complete(request: 1, with: TestFixture.definition(appId: "app-novo"))
        await second.value
        await loader.complete(request: 0, with: TestFixture.definition(appId: "app-antigo"))
        await first.value

        guard case .ready(let app) = store.state else { return XCTFail("Expected ready state") }
        XCTAssertEqual(app.appId, "app-novo")
    }
}

private enum TestError: Error { case secret(String) }

private actor SequencedLoader: AADDefinitionLoading {
    private var results: [Result<Data, Error>]
    init(results: [Result<Data, Error>]) { self.results = results }
    func loadDefinition() async throws -> Data { try results.removeFirst().get() }
}

private actor ControlledLoader: AADDefinitionLoading {
    private var continuations: [CheckedContinuation<Data, Error>] = []

    func loadDefinition() async throws -> Data {
        try await withCheckedThrowingContinuation { continuations.append($0) }
    }

    func waitUntilRequestsEqual(_ count: Int) async {
        while continuations.count < count { await Task.yield() }
    }

    func complete(request: Int, with data: Data) {
        continuations[request].resume(returning: data)
    }
}

enum TestFixture {
    static let validData = definition(appId: "app-exemplo")

    static func definition(appId: String) -> Data {
        Data("""
        {
          "schemaVersion":"1.0.0",
          "appId":"\(appId)",
          "minimumRuntimeVersion":"1.0.0",
          "navigation":{"initialRoute":"home"},
          "screens":[{"id":"home","route":"home","title":"Início","components":[
            {"type":"text","id":"intro","value":"Bem-vindo"},
            {"type":"button","id":"continue","label":"Continuar","action":{"type":"navigate","target":"details"}}
          ]},{"id":"details","route":"details","title":"Detalhes","components":[
            {"type":"text","id":"details-copy","value":"Conteúdo carregado com segurança"}
          ]}]
        }
        """.utf8)
    }
}
