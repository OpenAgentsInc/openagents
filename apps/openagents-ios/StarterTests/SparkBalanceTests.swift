import XCTest
import Security
@testable import OpenAgents

final class SparkBalanceTests:XCTestCase {
    func testMissingLockedAndUnreadableNeverOpenSDKOrBecomeZero() async {
        let cases:[(WalletKeychain.Read,Int64)]=[(.notFound,2),(.unreadable,4),(.unavailable(errSecInteractionNotAllowed),3),(.unavailable(errSecAuthFailed),5)]
        for (seed,code) in cases {
            let host=SparkHost(seed:{seed},reader:{_ in XCTFail("The SDK opened without a readable seed");return 0})
            let result=await host.balance();XCTAssertEqual(result.status,code)
        }
    }
    func testExistingEntropyIsReusedAndFailuresStayUnavailable() async {
        let fixture=Data(repeating:7,count:16)
        let host=SparkHost(seed:{.found(fixture)},reader:{data in XCTAssertEqual(data,fixture);return 12345})
        let result=await host.balance();XCTAssertEqual(result.status,1);XCTAssertEqual(result.sats,12345)
        enum Offline:Error {case offline}
        let offline=SparkHost(seed:{.found(fixture)},reader:{_ in throw Offline.offline})
        let unavailable=await offline.balance();XCTAssertEqual(unavailable.status,5)
    }
    func testReadOnlyConfigurationAndHistoricalStorage() throws {
        let c=SparkConnection.configuration(readOnly:true)
        XCTAssertFalse(c.backgroundTasksEnabled);XCTAssertNil(c.maxDepositClaimFee)
        XCTAssertNil(c.realTimeSyncServerUrl);XCTAssertNil(c.stableBalanceConfig)
        XCTAssertFalse(c.leafOptimizationConfig.autoEnabled)
        XCTAssertFalse(c.tokenOptimizationConfig.autoEnabled)
        XCTAssertEqual(try SparkConnection.directory(folder:"spark-mainnet").lastPathComponent,"spark-mainnet")
        XCTAssertNotNil(BIP39.englishWords())
    }
}

/// Opt-in SDK/mainnet check. Never touches the seed Keychain item or money.
final class SparkBalanceLiveTests:XCTestCase {
    @MainActor func testExistingKeychainBalanceBridgeKeepsSeed() async throws {
        // This test runs only in our dedicated simulator. Never replace an
        // existing item or cache, even if someone selects it on another device.
        #if !targetEnvironment(simulator)
        throw XCTSkip("Dedicated simulator only")
        #else
        switch WalletKeychain.readSeed() {
        case .notFound:break
        case .unavailable(let code):throw XCTSkip("Simulator Keychain status \(code)")
        default:throw XCTSkip("An existing wallet is left alone")
        }
        let directory=try SparkConnection.directory(folder:"spark-mainnet")
        guard !FileManager.default.fileExists(atPath:directory.path) else {throw XCTSkip("An existing SDK directory is left alone")}
        var bytes=[UInt8](repeating:0,count:16)
        guard SecRandomCopyBytes(kSecRandomDefault,bytes.count,&bytes)==errSecSuccess else {return XCTFail("Test entropy unavailable")}
        let entropy=Data(bytes)
        let query:[String:Any]=[kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:"com.openagents.app.spark",kSecAttrAccount as String:"seed-v1",kSecAttrSynchronizable as String:false]
        var item=query;item[kSecAttrAccessible as String]=kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;item[kSecValueData as String]=entropy
        guard SecItemAdd(item as CFDictionary,nil)==errSecSuccess else {return XCTFail("Test Keychain setup failed")}
        defer {
            if case .found(let kept)=WalletKeychain.readSeed(),kept==entropy {_ = SecItemDelete(query as CFDictionary)}
            try? FileManager.default.removeItem(at:directory)
        }
        let host=NativeHost(updates:try NativeUpdates())
        let reply=try await host.call("wallet.balance",[:]) as? [String:String]
        let record=try XCTUnwrap(reply?["record"]).data(using:.utf8)!
        let object=try JSONSerialization.jsonObject(with:record) as! [String:Any]
        XCTAssertEqual(object["schema"] as? String,"openagents.wallet.balance.v1")
        XCTAssertEqual(object["status"] as? Int,1);XCTAssertEqual(object["sats"] as? Int,0)
        guard case .found(let kept)=WalletKeychain.readSeed() else {return XCTFail("The saved test wallet disappeared")}
        XCTAssertTrue(kept==entropy,"The seed must remain unchanged")
        #endif
    }
    func testEmptyThrowawayWalletReadOnly() async {
        let folder="spark-readonly-test-"+UUID().uuidString
        guard let dir=try? SparkConnection.directory(folder:folder) else {return XCTFail("Test storage unavailable")}
        defer {try? FileManager.default.removeItem(at:dir)}
        var entropy=[UInt8](repeating:0,count:16)
        guard SecRandomCopyBytes(kSecRandomDefault,entropy.count,&entropy)==errSecSuccess,
              let words=BIP39.englishWords(),let mnemonic=BIP39.mnemonic(entropy:Data(entropy),words:words) else {return XCTFail("Test seed setup unavailable")}
        do {
            let sdk=try await SparkConnection.open(mnemonic:mnemonic,folder:folder,readOnly:true)
            do {
                _ = try await sdk.syncWallet(request:.init())
                let info=try await sdk.getInfo(request:.init(ensureSynced:false))
                XCTAssertEqual(info.balanceSats,0)
                try? await sdk.disconnect()
            } catch {try? await sdk.disconnect();XCTFail("Read-only SDK sync failed; SDK details withheld")}
        } catch {XCTFail("Read-only SDK connect failed; SDK details withheld")}
    }
}
