import Foundation

public struct AADApp: Decodable {
    public let schemaVersion: String
    public let appId: String
    public let minimumRuntimeVersion: String
    public let navigation: AADNavigation
    public let screens: [AADScreen]

    public init(data: Data) throws {
        self = try JSONDecoder().decode(Self.self, from: data)
        try validate()
    }

    private init(schemaVersion: String, appId: String, minimumRuntimeVersion: String, navigation: AADNavigation, screens: [AADScreen]) {
        self.schemaVersion = schemaVersion
        self.appId = appId
        self.minimumRuntimeVersion = minimumRuntimeVersion
        self.navigation = navigation
        self.screens = screens
    }

    private static func validId(_ value: String) -> Bool {
        value.count >= 1 && value.utf8.count <= 64 &&
        value.utf8.first.map { (97...122).contains(Int($0)) } == true &&
        value.utf8.allSatisfy { (97...122).contains(Int($0)) || (48...57).contains(Int($0)) || $0 == 45 }
    }

    public func validate() throws {
        guard schemaVersion == "1.0.0", minimumRuntimeVersion == "1.0.0" else { throw AADError.version }
        guard Self.validId(appId) else { throw AADError.identifier }
        guard (1...100).contains(screens.count) else { throw AADError.limit }
        var ids = Set<String>()
        var routes = Set<String>()
        for screen in screens {
            guard Self.validId(screen.id), Self.validId(screen.route) else { throw AADError.identifier }
            guard ids.insert(screen.id).inserted, routes.insert(screen.route).inserted else { throw AADError.duplicate }
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
                if case .button(_, _, let target) = component, !routes.contains(target) { throw AADError.route }
            }
        }
    }
}

public struct AADNavigation: Decodable { public let initialRoute: String }

public struct AADScreen: Decodable, Identifiable {
    public let id: String
    public let route: String
    public let title: String
    public let components: [AADComponent]
}

public enum AADError: Error, Equatable {
    case version, identifier, limit, duplicate, route
}

public enum AADComponent: Decodable, Identifiable {
    case text(String, String)
    case button(String, String, String)
    case spacer(String)

    public var id: String {
        switch self {
        case .text(let id, _), .button(let id, _, _), .spacer(let id): return id
        }
    }

    private enum CodingKeys: String, CodingKey { case type, id, value, label, action }
    private struct NavigateAction: Decodable {
        let type: String
        let target: String
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let type = try c.decode(String.self, forKey: .type)
        let id = try c.decode(String.self, forKey: .id)
        // Reject unknown keys to preserve parity with serde deny_unknown_fields.
        let allowed: Set<String>
        switch type {
        case "text":
            allowed = ["type", "id", "value"]
            self = .text(id, try c.decode(String.self, forKey: .value))
        case "button":
            allowed = ["type", "id", "label", "action"]
            let action = try c.decode(NavigateAction.self, forKey: .action)
            guard action.type == "navigate" else { throw DecodingError.dataCorruptedError(forKey: .action, in: c, debugDescription: "Unsupported action") }
            self = .button(id, try c.decode(String.self, forKey: .label), action.target)
        case "spacer":
            allowed = ["type", "id"]
            self = .spacer(id)
        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "Unsupported component")
        }
        let raw = try decoder.singleValueContainer().decode([String: JSONValue].self)
        guard Set(raw.keys).isSubset(of: allowed) else {
            throw DecodingError.dataCorruptedError(forKey: .type, in: c, debugDescription: "Unknown component fields")
        }
    }
}

private enum JSONValue: Decodable {
    case string, number, bool, null, array, object
    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if (try? c.decode(Bool.self)) != nil { self = .bool }
        else if (try? c.decode(Double.self)) != nil { self = .number }
        else if (try? c.decode(String.self)) != nil { self = .string }
        else if (try? c.decode([JSONValue].self)) != nil { self = .array }
        else { _ = try c.decode([String: JSONValue].self); self = .object }
    }
}
