import SwiftUI

/// Renders only the explicitly supported AAD components using local navigation.
/// Networking, authentication and executable content remain outside this view.
@available(iOS 16.0, macOS 13.0, *)
public struct AADAppView: View {
    private let definition: AADApp
    @State private var path: [String] = []

    public init(definition: AADApp) {
        self.definition = definition
    }

    public var body: some View {
        NavigationStack(path: $path) {
            screenView(for: definition.navigation.initialRoute)
                .navigationDestination(for: String.self) { route in
                    screenView(for: route)
                }
        }
    }

    @ViewBuilder
    private func screenView(for route: String) -> some View {
        if let screen = definition.screens.first(where: { $0.route == route }) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(screen.components) { component in
                        componentView(component)
                    }
                }
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .navigationTitle(screen.title)
        } else {
            Text("Tela indisponível")
        }
    }

    @ViewBuilder
    private func componentView(_ component: AADComponent) -> some View {
        switch component {
        case .text(_, let value):
            Text(value)
                .accessibilityLabel(value)
        case .button(_, let label, let target):
            Button(label) { path.append(target) }
                .buttonStyle(.borderedProminent)
                .accessibilityHint("Abre a tela \(target)")
        case .spacer:
            Spacer(minLength: 16)
        }
    }
}
