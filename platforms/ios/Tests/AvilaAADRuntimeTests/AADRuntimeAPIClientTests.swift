import Foundation
import XCTest
@testable import AvilaAADRuntime

final class AADRuntimeAPIClientTests: XCTestCase {
    func test200SendsBearerAndReturnsStrictlyValidatedDefinition() async throws {
        let transport = RecordingTransport(result: .success(response(
            status: 200,
            headers: ["ETag": "\"version-7\""],
            body: envelope(definition: TestFixture.validData)
        )))
        let credentials = InMemoryCredentialStore()
        try credentials.save(try JSONEncoder().encode(AADRuntimeCredential(accessToken: "access-secret")))
        let client = AADRuntimeAPIClient(
            baseURL: URL(string: "https://cms.example")!,
            appId: "app-exemplo",
            environment: "production",
            transport: transport,
            credentialStore: credentials
        )

        let result = try await client.fetchDefinition(etag: "\"previous\"")
        XCTAssertEqual(result.app.appId, "app-exemplo")
        XCTAssertEqual(result.etag, "\"version-7\"")
        let request = await transport.lastRequest
        XCTAssertEqual(request?.url?.absoluteString, "https://cms.example/api/runtime/v1/apps/app-exemplo/environments/production")
        XCTAssertEqual(request?.value(forHTTPHeaderField: "Authorization"), "Bearer access-secret")
        XCTAssertEqual(request?.value(forHTTPHeaderField: "If-None-Match"), "\"previous\"")
        XCTAssertEqual(request?.value(forHTTPHeaderField: "Accept"), "application/json")
    }

    func test304ThrowsNotModifiedOnlyAfterSendingCredential() async throws {
        let transport = RecordingTransport(result: .success(response(status: 304)))
        let credentials = InMemoryCredentialStore()
        try credentials.save(try JSONEncoder().encode(AADRuntimeCredential(accessToken: "access-secret")))
        let client = makeClient(transport: transport, credentials: credentials)

        await XCTAssertThrowsErrorAsync(try await client.fetchDefinition(etag: "\"version-7\"")) {
            XCTAssertEqual($0 as? AADRuntimeAPIError, .notModified)
        }
        let request = await transport.lastRequest
        XCTAssertEqual(request?.value(forHTTPHeaderField: "Authorization"), "Bearer access-secret")
    }

    func test401AndMissingCredentialRequireAuthenticationWithoutLeakingToken() async throws {
        let missing = makeClient(
            transport: RecordingTransport(result: .success(response(status: 200))),
            credentials: InMemoryCredentialStore()
        )
        await XCTAssertThrowsErrorAsync(try await missing.fetchDefinition()) {
            XCTAssertEqual($0 as? AADRuntimeAPIError, .unauthenticated)
        }

        let credentials = InMemoryCredentialStore()
        try credentials.save(try JSONEncoder().encode(AADRuntimeCredential(accessToken: "never-log-this")))
        let unauthorized = makeClient(
            transport: RecordingTransport(result: .success(response(status: 401))),
            credentials: credentials
        )
        await XCTAssertThrowsErrorAsync(try await unauthorized.fetchDefinition()) {
            XCTAssertEqual($0 as? AADRuntimeAPIError, .unauthenticated)
            XCTAssertFalse(String(describing: $0).contains("never-log-this"))
        }
    }

    func testInvalidEnvelopeOrDefinitionFailsClosed() async throws {
        let credentials = InMemoryCredentialStore()
        try credentials.save(try JSONEncoder().encode(AADRuntimeCredential(accessToken: "token")))
        let invalidDefinition = TestFixture.validData.replacingOccurrences(
            of: Data("\"schemaVersion\":\"1.0.0\"".utf8),
            with: Data("\"schemaVersion\":\"2.0.0\"".utf8)
        )
        for body in [
            Data("{}".utf8),
            envelope(definition: invalidDefinition),
            envelope(definition: TestFixture.validData, extraField: true)
        ] {
            let client = makeClient(
                transport: RecordingTransport(result: .success(response(status: 200, body: body))),
                credentials: credentials
            )
            await XCTAssertThrowsErrorAsync(try await client.fetchDefinition()) {
                XCTAssertEqual($0 as? AADRuntimeAPIError, .invalidResponse)
            }
        }
    }

    func testRemoteLoaderFeedsExistingRuntimeBoundary() async throws {
        let credentials = InMemoryCredentialStore()
        try credentials.save(try JSONEncoder().encode(AADRuntimeCredential(accessToken: "token")))
        let client = makeClient(
            transport: RecordingTransport(result: .success(response(
                status: 200,
                body: envelope(definition: TestFixture.validData)
            ))),
            credentials: credentials
        )
        let loader = RemoteAADDefinitionLoader(client: client)
        let data = try await loader.loadDefinition()
        XCTAssertEqual(try AADDefinitionDecoder().decode(data).appId, "app-exemplo")
    }

    private func makeClient(
        transport: any AADRuntimeTransport,
        credentials: any AADCredentialStoring
    ) -> AADRuntimeAPIClient {
        AADRuntimeAPIClient(
            baseURL: URL(string: "https://cms.example")!,
            appId: "app-exemplo",
            environment: "production",
            transport: transport,
            credentialStore: credentials
        )
    }
}

private actor RecordingTransport: AADRuntimeTransport {
    private let result: Result<(Data, HTTPURLResponse), Error>
    private(set) var lastRequest: URLRequest?

    init(result: Result<(Data, HTTPURLResponse), Error>) { self.result = result }

    func data(for request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        lastRequest = request
        return try result.get()
    }
}

private func response(
    status: Int,
    headers: [String: String] = [:],
    body: Data = Data()
) -> (Data, HTTPURLResponse) {
    (
        body,
        HTTPURLResponse(
            url: URL(string: "https://cms.example/runtime")!,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: headers
        )!
    )
}

private func envelope(definition: Data, extraField: Bool = false) -> Data {
    var object: [String: Any] = [
        "appId": "app-exemplo",
        "environment": "production",
        "version": 7,
        "definition": try! JSONSerialization.jsonObject(with: definition)
    ]
    if extraField { object["unexpected"] = true }
    return try! JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
}

private func XCTAssertThrowsErrorAsync<T>(
    _ expression: @autoclosure () async throws -> T,
    _ errorHandler: (Error) -> Void = { _ in },
    file: StaticString = #filePath,
    line: UInt = #line
) async {
    do {
        _ = try await expression()
        XCTFail("Expected error", file: file, line: line)
    } catch {
        errorHandler(error)
    }
}

private extension Data {
    func replacingOccurrences(of target: Data, with replacement: Data) -> Data {
        guard let range = range(of: target) else { return self }
        var copy = self
        copy.replaceSubrange(range, with: replacement)
        return copy
    }
}
