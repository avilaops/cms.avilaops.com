import Foundation

public struct AADDefinitionDecoder: Sendable {
    public static let maximumBytes = 512 * 1_024

    public init() {}

    public func decode(_ data: Data) throws -> AADApp {
        guard data.count <= Self.maximumBytes else { throw AADError.payloadTooLarge }
        try JSONDuplicateKeyValidator.validate(data)
        let app = try JSONDecoder().decode(AADAppWire.self, from: data).makeApp()
        try app.validate()
        return app
    }
}

struct JSONDuplicateKeyValidator {
    private let bytes: [UInt8]
    private var index = 0

    static func validate(_ data: Data) throws {
        var validator = Self(bytes: Array(data))
        try validator.parseValue()
        validator.skipWhitespace()
        guard validator.index == validator.bytes.count else { throw ValidationError.malformed }
    }

    private mutating func parseValue() throws {
        skipWhitespace()
        guard let byte = current else { throw ValidationError.malformed }
        switch byte {
        case 0x7B: try parseObject()
        case 0x5B: try parseArray()
        case 0x22: _ = try parseString()
        default: try parsePrimitive()
        }
    }

    private mutating func parseObject() throws {
        index += 1
        skipWhitespace()
        var keys = Set<String>()
        if consume(0x7D) { return }

        while true {
            skipWhitespace()
            let key = try parseString()
            guard keys.insert(key).inserted else { throw AADError.duplicateJSONKey }
            skipWhitespace()
            guard consume(0x3A) else { throw ValidationError.malformed }
            try parseValue()
            skipWhitespace()
            if consume(0x7D) { return }
            guard consume(0x2C) else { throw ValidationError.malformed }
        }
    }

    private mutating func parseArray() throws {
        index += 1
        skipWhitespace()
        if consume(0x5D) { return }

        while true {
            try parseValue()
            skipWhitespace()
            if consume(0x5D) { return }
            guard consume(0x2C) else { throw ValidationError.malformed }
        }
    }

    private mutating func parseString() throws -> String {
        guard current == 0x22 else { throw ValidationError.malformed }
        let start = index
        index += 1
        var escaped = false

        while let byte = current {
            index += 1
            if escaped {
                escaped = false
            } else if byte == 0x5C {
                escaped = true
            } else if byte == 0x22 {
                let token = Data(bytes[start..<index])
                return try JSONDecoder().decode(String.self, from: token)
            } else if byte < 0x20 {
                throw ValidationError.malformed
            }
        }
        throw ValidationError.malformed
    }

    private mutating func parsePrimitive() throws {
        let start = index
        while let byte = current, !Self.delimiters.contains(byte) {
            index += 1
        }
        guard index > start else { throw ValidationError.malformed }
    }

    private mutating func skipWhitespace() {
        while let byte = current, Self.whitespace.contains(byte) { index += 1 }
    }

    private mutating func consume(_ byte: UInt8) -> Bool {
        guard current == byte else { return false }
        index += 1
        return true
    }

    private var current: UInt8? {
        index < bytes.count ? bytes[index] : nil
    }

    private static let whitespace: Set<UInt8> = [0x20, 0x09, 0x0A, 0x0D]
    private static let delimiters: Set<UInt8> = whitespace.union([0x2C, 0x5D, 0x7D])

    private enum ValidationError: Error { case malformed }
}
