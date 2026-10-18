import Foundation

public protocol AADDefinitionLoading: Sendable {
    func loadDefinition() async throws -> Data
}

public struct InMemoryAADDefinitionLoader: AADDefinitionLoading {
    private let data: Data

    public init(data: Data) {
        self.data = data
    }

    public func loadDefinition() async throws -> Data { data }
}

public struct RemoteAADDefinitionLoader: AADDefinitionLoading {
    private let client: AADRuntimeAPIClient

    public init(client: AADRuntimeAPIClient) {
        self.client = client
    }

    public func loadDefinition() async throws -> Data {
        try await client.fetchDefinition().definitionData
    }
}
