import Foundation
import Security
import XCTest
@testable import AvilaAADRuntime

final class AADCredentialStoreTests: XCTestCase {
    func testInMemoryStoreSavesReplacesReadsAndDeletesCredential() throws {
        let store = InMemoryCredentialStore()
        XCTAssertNil(try store.read())
        try store.save(Data("primeira".utf8))
        XCTAssertEqual(try store.read(), Data("primeira".utf8))
        try store.save(Data("segunda".utf8))
        XCTAssertEqual(try store.read(), Data("segunda".utf8))
        try store.delete()
        XCTAssertNil(try store.read())
        XCTAssertNoThrow(try store.delete())
    }

    func testKeychainErrorNeverIncludesCredentialBytes() {
        let error = AADCredentialStoreError.unexpectedStatus(-50)
        XCTAssertEqual(error.description, "Keychain operation failed (-50)")
        XCTAssertFalse(error.description.contains("credential"))
    }

    func testKeychainSaveUpdatesExistingItemWithDeviceOnlyProtection() throws {
        let access = RecordingKeychain(updateStatus: errSecSuccess)
        let store = KeychainCredentialStore(service: "com.avila.runtime", account: "session", access: access)
        let secret = Data("segredo".utf8)

        try store.save(secret)

        XCTAssertEqual(access.updatedData, secret)
        XCTAssertTrue(access.updatedAccessibilityIsDeviceOnly)
        XCTAssertEqual(access.addCount, 0)
    }

    func testKeychainSaveAddsOneProtectedItemWhenMissing() throws {
        let access = RecordingKeychain(updateStatus: errSecItemNotFound)
        let store = KeychainCredentialStore(service: "com.avila.runtime", account: "session", access: access)
        let secret = Data("segredo".utf8)

        try store.save(secret)

        XCTAssertEqual(access.addedData, secret)
        XCTAssertTrue(access.addedAccessibilityIsDeviceOnly)
        XCTAssertEqual(access.addCount, 1)
    }
}

private final class RecordingKeychain: KeychainAccessing {
    private let updateStatus: OSStatus
    private(set) var updatedData: Data?
    private(set) var updatedAccessibilityIsDeviceOnly = false
    private(set) var addedData: Data?
    private(set) var addedAccessibilityIsDeviceOnly = false
    private(set) var addCount = 0

    init(updateStatus: OSStatus) {
        self.updateStatus = updateStatus
    }

    func update(query _: [CFString: Any], attributes: [CFString: Any]) -> OSStatus {
        updatedData = attributes[kSecValueData] as? Data
        if let value = attributes[kSecAttrAccessible] as CFTypeRef? {
            updatedAccessibilityIsDeviceOnly = CFEqual(value, kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
        }
        return updateStatus
    }

    func add(item: [CFString: Any]) -> OSStatus {
        addCount += 1
        addedData = item[kSecValueData] as? Data
        if let value = item[kSecAttrAccessible] as CFTypeRef? {
            addedAccessibilityIsDeviceOnly = CFEqual(value, kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
        }
        return errSecSuccess
    }

    func copy(query _: [CFString: Any], result _: UnsafeMutablePointer<CFTypeRef?>?) -> OSStatus {
        errSecItemNotFound
    }

    func delete(query _: [CFString: Any]) -> OSStatus { errSecItemNotFound }
}
