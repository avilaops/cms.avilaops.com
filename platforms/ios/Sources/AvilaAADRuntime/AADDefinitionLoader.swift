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
