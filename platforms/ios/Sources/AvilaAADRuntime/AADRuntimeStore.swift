import Combine
import Foundation

@MainActor
public final class AADRuntimeStore: ObservableObject {
    public enum State: Equatable, Sendable {
        case idle
        case loading
        case ready(AADApp)
        case failed(String)
    }

    public static let safeFailureMessage = "Não foi possível carregar o aplicativo."

    @Published public private(set) var state: State = .idle

    private let loader: any AADDefinitionLoading
    private let decoder: AADDefinitionDecoder
    private var requestGeneration = 0

    public init(
        loader: any AADDefinitionLoading,
        decoder: AADDefinitionDecoder = AADDefinitionDecoder()
    ) {
        self.loader = loader
        self.decoder = decoder
    }

    public func load() async {
        requestGeneration += 1
        let generation = requestGeneration
        state = .loading

        do {
            let data = try await loader.loadDefinition()
            guard generation == requestGeneration else { return }
            let app = try decoder.decode(data)
            state = .ready(app)
        } catch {
            guard generation == requestGeneration else { return }
            state = .failed(Self.safeFailureMessage)
        }
    }

    public func retry() async {
        await load()
    }

    public func loadIfNeeded() async {
        guard state == .idle else { return }
        await load()
    }
}
