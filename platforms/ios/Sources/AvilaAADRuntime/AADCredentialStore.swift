import Foundation
import Security

public protocol AADCredentialStoring {
    func save(_ credential: Data) throws
    func read() throws -> Data?
    func delete() throws
}

public enum AADCredentialStoreError: Error, Equatable, CustomStringConvertible {
    case unexpectedStatus(OSStatus)

    public var description: String {
        switch self {
        case .unexpectedStatus(let status): "Keychain operation failed (\(status))"
        }
    }
}

protocol KeychainAccessing: AnyObject {
    func update(query: [CFString: Any], attributes: [CFString: Any]) -> OSStatus
    func add(item: [CFString: Any]) -> OSStatus
    func copy(query: [CFString: Any], result: UnsafeMutablePointer<CFTypeRef?>?) -> OSStatus
    func delete(query: [CFString: Any]) -> OSStatus
}

private final class SystemKeychain: KeychainAccessing {
    func update(query: [CFString: Any], attributes: [CFString: Any]) -> OSStatus {
        SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    }

    func add(item: [CFString: Any]) -> OSStatus {
        SecItemAdd(item as CFDictionary, nil)
    }

    func copy(query: [CFString: Any], result: UnsafeMutablePointer<CFTypeRef?>?) -> OSStatus {
        SecItemCopyMatching(query as CFDictionary, result)
    }

    func delete(query: [CFString: Any]) -> OSStatus {
        SecItemDelete(query as CFDictionary)
    }
}

public final class InMemoryCredentialStore: AADCredentialStoring {
    private let lock = NSLock()
    private var credential: Data?

    public init() {}

    public func save(_ credential: Data) throws {
        lock.lock()
        defer { lock.unlock() }
        self.credential = credential
    }

    public func read() throws -> Data? {
        lock.lock()
        defer { lock.unlock() }
        return credential
    }

    public func delete() throws {
        lock.lock()
        defer { lock.unlock() }
        credential = nil
    }
}

public struct KeychainCredentialStore: AADCredentialStoring {
    private let service: String
    private let account: String
    private let access: any KeychainAccessing

    public init(service: String, account: String) {
        self.init(service: service, account: account, access: SystemKeychain())
    }

    init(service: String, account: String, access: any KeychainAccessing) {
        self.service = service
        self.account = account
        self.access = access
    }

    public func save(_ credential: Data) throws {
        let status = access.update(query: baseQuery, attributes: [
            kSecValueData: credential,
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        ])

        if status == errSecSuccess { return }
        if status != errSecItemNotFound { throw AADCredentialStoreError.unexpectedStatus(status) }

        var item = baseQuery
        item[kSecValueData] = credential
        item[kSecAttrAccessible] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let addStatus = access.add(item: item)
        guard addStatus == errSecSuccess else {
            throw AADCredentialStoreError.unexpectedStatus(addStatus)
        }
    }

    public func read() throws -> Data? {
        var query = baseQuery
        query[kSecReturnData] = true
        query[kSecMatchLimit] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = access.copy(query: query, result: &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else {
            throw AADCredentialStoreError.unexpectedStatus(status)
        }
        return data
    }

    public func delete() throws {
        let status = access.delete(query: baseQuery)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw AADCredentialStoreError.unexpectedStatus(status)
        }
    }

    private var baseQuery: [CFString: Any] {
        [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account
        ]
    }
}
