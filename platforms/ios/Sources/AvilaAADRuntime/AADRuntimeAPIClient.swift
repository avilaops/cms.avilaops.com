import Foundation

public protocol AADRuntimeTransport: Sendable {
    func data(for request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionAADRuntimeTransport: AADRuntimeTransport {
    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func data(for request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse else {
            throw AADRuntimeAPIError.invalidResponse
        }
        return (data, response)
    }
}

public struct AADRuntimeCredential: Codable, Equatable, Sendable {
    public let accessToken: String

    public init(accessToken: String) {
        self.accessToken = accessToken
    }
}

public enum AADRuntimeAPIError: Error, Equatable, Sendable {
    case unauthenticated
    case notModified
    case invalidResponse
}

public struct AADRuntimeFetchResult: Equatable, Sendable {
    public let app: AADApp
    public let etag: String?
    let definitionData: Data
}

public struct AADRuntimeAPIClient: @unchecked Sendable {
    private let baseURL: URL
    private let appId: String
    private let environment: String
    private let transport: any AADRuntimeTransport
    private let credentialStore: any AADCredentialStoring
    private let decoder: AADDefinitionDecoder

    public init(
        baseURL: URL,
        appId: String,
        environment: String,
        transport: any AADRuntimeTransport = URLSessionAADRuntimeTransport(),
        credentialStore: any AADCredentialStoring,
        decoder: AADDefinitionDecoder = AADDefinitionDecoder()
    ) {
        self.baseURL = baseURL
        self.appId = appId
        self.environment = environment
        self.transport = transport
        self.credentialStore = credentialStore
        self.decoder = decoder
    }

    public func fetchDefinition(etag: String? = nil) async throws -> AADRuntimeFetchResult {
        guard
            let credentialData = try credentialStore.read(),
            let credential = try? JSONDecoder().decode(AADRuntimeCredential.self, from: credentialData),
            !credential.accessToken.isEmpty
        else {
            throw AADRuntimeAPIError.unauthenticated
        }

        var request = URLRequest(url: endpoint)
        request.httpMethod = "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(credential.accessToken)", forHTTPHeaderField: "Authorization")
        if let etag {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        }

        let (data, response) = try await transport.data(for: request)
        switch response.statusCode {
        case 200:
            return try decodeResponse(data, etag: response.value(forHTTPHeaderField: "ETag"))
        case 304:
            throw AADRuntimeAPIError.notModified
        case 401:
            throw AADRuntimeAPIError.unauthenticated
        default:
            throw AADRuntimeAPIError.invalidResponse
        }
    }

    private var endpoint: URL {
        baseURL
            .appendingPathComponent("api")
            .appendingPathComponent("runtime")
            .appendingPathComponent("v1")
            .appendingPathComponent("apps")
            .appendingPathComponent(appId)
            .appendingPathComponent("environments")
            .appendingPathComponent(environment)
    }

    private func decodeResponse(_ data: Data, etag: String?) throws -> AADRuntimeFetchResult {
        do {
            try JSONDuplicateKeyValidator.validate(data)
            guard
                let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                Set(envelope.keys) == ["appId", "environment", "version", "definition"],
                envelope["appId"] as? String == appId,
                envelope["environment"] as? String == environment,
                let version = envelope["version"] as? NSNumber,
                CFGetTypeID(version) != CFBooleanGetTypeID(),
                version.intValue > 0,
                version.doubleValue == Double(version.intValue),
                let definition = envelope["definition"],
                JSONSerialization.isValidJSONObject(definition)
            else {
                throw AADRuntimeAPIError.invalidResponse
            }
            let definitionData = try JSONSerialization.data(
                withJSONObject: definition,
                options: [.sortedKeys]
            )
            let app = try decoder.decode(definitionData)
            guard app.appId == appId else { throw AADRuntimeAPIError.invalidResponse }
            return AADRuntimeFetchResult(app: app, etag: etag, definitionData: definitionData)
        } catch {
            throw AADRuntimeAPIError.invalidResponse
        }
    }
}
