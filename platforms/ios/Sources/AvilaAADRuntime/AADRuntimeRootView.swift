import SwiftUI

@available(iOS 16.0, macOS 13.0, *)
public struct AADRuntimeRootView: View {
    @StateObject private var store: AADRuntimeStore

    public init(store: AADRuntimeStore) {
        _store = StateObject(wrappedValue: store)
    }

    public var body: some View {
        AADRuntimeContentView(state: store.state) {
            Task { await store.retry() }
        }
        .task { await store.loadIfNeeded() }
    }
}

@available(iOS 16.0, macOS 13.0, *)
struct AADRuntimeContentView: View {
    let state: AADRuntimeStore.State
    let onRetry: () -> Void

    var body: some View {
        switch state {
        case .idle, .loading:
            VStack(spacing: 16) {
                ProgressView()
                Text("Carregando aplicativo").font(.headline)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .accessibilityElement(children: .combine)
        case .ready(let app):
            AADAppView(definition: app)
        case .failed(let message):
            VStack(spacing: 20) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 42))
                    .foregroundStyle(.orange)
                    .accessibilityHidden(true)
                Text("Aplicativo indisponível").font(.title2.bold())
                Text(message)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                Button("Tentar novamente", action: onRetry)
                    .buttonStyle(.borderedProminent)
            }
            .padding(32)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}
