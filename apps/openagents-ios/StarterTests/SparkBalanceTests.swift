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
