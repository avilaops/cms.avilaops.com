import Foundation

public struct AADApp: Equatable, Sendable {
    public let schemaVersion: String
    public let appId: String
    public let minimumRuntimeVersion: String
    public let navigation: AADNavigation
    public let screens: [AADScreen]

    public init(data: Data) throws {
        self = try AADDefinitionDecoder().decode(data)
    }

    init(
        schemaVersion: String,
        appId: String,
        minimumRuntimeVersion: String,
        navigation: AADNavigation,
        screens: [AADScreen]
    ) {
        self.schemaVersion = schemaVersion
        self.appId = appId
        self.minimumRuntimeVersion = minimumRuntimeVersion
        self.navigation = navigation
        self.screens = screens
    }

    private static func validId(_ value: String) -> Bool {
        let bytes = value.utf8
        return (1...64).contains(bytes.count)
            && bytes.first.map { (97...122).contains(Int($0)) } == true
            && bytes.allSatisfy {
                (97...122).contains(Int($0)) || (48...57).contains(Int($0)) || $0 == 45
            }
    }

    public func validate() throws {
        guard schemaVersion == "1.0.0", minimumRuntimeVersion == "1.0.0" else {
            throw AADError.version
        }
        guard Self.validId(appId), Self.validId(navigation.initialRoute) else {
            throw AADError.identifier
        }
        guard (1...100).contains(screens.count) else { throw AADError.limit }

        var ids = Set<String>()
        var routes = Set<String>()
        for screen in screens {
            guard Self.validId(screen.id), Self.validId(screen.route) else {
                throw AADError.identifier
            }
            guard ids.insert(screen.id).inserted, routes.insert(screen.route).inserted else {
                throw AADError.duplicate
            }
            guard screen.components.count <= 200 else { throw AADError.limit }

            var componentIds = Set<String>()
            for component in screen.components {
                guard Self.validId(component.id) else { throw AADError.identifier }
                guard componentIds.insert(component.id).inserted else { throw AADError.duplicate }
            }
        }

        guard routes.contains(navigation.initialRoute) else { throw AADError.route }
        for screen in screens {
            for component in screen.components {
                if case .button(_, _, let target) = component, !routes.contains(target) {
                    throw AADError.route
                }
            }
        }
    }
}

struct AADAppWire: Decodable {
    let schemaVersion: String
    let appId: String
    let minimumRuntimeVersion: String
    let navigation: AADNavigation
    let screens: [AADScreen]

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case schemaVersion, appId, minimumRuntimeVersion, navigation, screens
    }

    init(from decoder: Decoder) throws {
        try rejectUnknownKeys(in: decoder, allowed: CodingKeys.self)
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try container.decode(String.self, forKey: .schemaVersion)
        appId = try container.decode(String.self, forKey: .appId)
        minimumRuntimeVersion = try container.decode(String.self, forKey: .minimumRuntimeVersion)
        navigation = try container.decode(AADNavigation.self, forKey: .navigation)
        screens = try container.decode([AADScreen].self, forKey: .screens)
    }

    func makeApp() -> AADApp {
        AADApp(
            schemaVersion: schemaVersion,
            appId: appId,
            minimumRuntimeVersion: minimumRuntimeVersion,
            navigation: navigation,
            screens: screens
        )
    }
}

public struct AADNavigation: Decodable, Equatable, Sendable {
    public let initialRoute: String

    private enum CodingKeys: String, CodingKey, CaseIterable { case initialRoute }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(in: decoder, allowed: CodingKeys.self)
        let container = try decoder.container(keyedBy: CodingKeys.self)
        initialRoute = try container.decode(String.self, forKey: .initialRoute)
    }
}

public struct AADScreen: Decodable, Equatable, Identifiable, Sendable {
    public let id: String
    public let route: String
    public let title: String
    public let components: [AADComponent]

    private enum CodingKeys: String, CodingKey, CaseIterable { case id, route, title, components }

    public init(from decoder: Decoder) throws {
        try rejectUnknownKeys(in: decoder, allowed: CodingKeys.self)
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        route = try container.decode(String.self, forKey: .route)
        title = try container.decode(String.self, forKey: .title)
        components = try container.decode([AADComponent].self, forKey: .components)
    }
}

public enum AADError: Error, Equatable, Sendable {
    case payloadTooLarge
    case duplicateJSONKey
    case version
    case identifier
    case limit
    case duplicate
    case route
}

public enum AADComponent: Decodable, Equatable, Identifiable, Sendable {
    case text(String, String)
    case button(String, String, String)
    case spacer(String)

    public var id: String {
        switch self {
        case .text(let id, _), .button(let id, _, _), .spacer(let id): id
        }
    }

    private enum CodingKeys: String, CodingKey { case type, id, value, label, action }

    private struct NavigateAction: Decodable {
        let type: String
        let target: String

        private enum CodingKeys: String, CodingKey, CaseIterable { case type, target }

        init(from decoder: Decoder) throws {
            try rejectUnknownKeys(in: decoder, allowed: CodingKeys.self)
            let container = try decoder.container(keyedBy: CodingKeys.self)
            type = try container.decode(String.self, forKey: .type)
            target = try container.decode(String.self, forKey: .target)
        }
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: JSONValue].self)
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        let id = try container.decode(String.self, forKey: .id)

        let allowed: Set<String>
        switch type {
        case "text":
            allowed = ["type", "id", "value"]
            self = .text(id, try container.decode(String.self, forKey: .value))
        case "button":
            allowed = ["type", "id", "label", "action"]
            let action = try container.decode(NavigateAction.self, forKey: .action)
            guard action.type == "navigate" else {
                throw DecodingError.dataCorruptedError(
                    forKey: .action,
                    in: container,
                    debugDescription: "Unsupported action"
                )
            }
            self = .button(id, try container.decode(String.self, forKey: .label), action.target)
        case "spacer":
            allowed = ["type", "id"]
            self = .spacer(id)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unsupported component"
            )
        }

        guard Set(raw.keys).isSubset(of: allowed) else {
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown component fields"
            )
        }
    }
}

private func rejectUnknownKeys<Keys>(in decoder: Decoder, allowed: Keys.Type) throws
where Keys: CodingKey & CaseIterable, Keys.AllCases.Element == Keys {
    let raw = try decoder.singleValueContainer().decode([String: JSONValue].self)
    let allowedKeys = Set(Keys.allCases.map(\.stringValue))
    guard Set(raw.keys).isSubset(of: allowedKeys) else {
        throw DecodingError.dataCorrupted(
            .init(codingPath: decoder.codingPath, debugDescription: "Unknown fields")
        )
    }
}

private enum JSONValue: Decodable {
    case string, number, bool, null, array, object

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if (try? container.decode(Bool.self)) != nil { self = .bool }
        else if (try? container.decode(Double.self)) != nil { self = .number }
        else if (try? container.decode(String.self)) != nil { self = .string }
        else if (try? container.decode([JSONValue].self)) != nil { self = .array }
        else {
            _ = try container.decode([String: JSONValue].self)
            self = .object
        }
    }
}
