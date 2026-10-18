import XCTest
@testable import OpenAgents

/// "Run on" with a project picked still offers OpenAgents Cloud (#12020).
final class RunOnCloudHostTests: XCTestCase {
    private let mac = HostOption(id: "mac", name: "Studio Mac", online: true)
    private let cloud = HostOption(id: "cloud", name: AppModel.cloudHostName, online: true)

    private func checkout(on device: String) -> ProjectOption {
        ProjectOption(id: "p-\(device)", name: "openagents", path: "~/work/openagents", device: device, deviceName: device,
                      online: true, git: true, groupKey: "openagents", groupName: "openagents", colorIndex: 0)
    }

    func testCloudIsOfferedBesidesAProjectsCheckouts() {
        let pick = HostOption.cloudBesides([checkout(on: "mac")], in: [mac, cloud], continuationDevice: nil)
        XCTAssertEqual(pick, cloud)
        XCTAssertEqual(pick?.displayName, "OpenAgents Cloud")
    }

    func testOtherComputersAreNotAdded() {
        XCTAssertNil(HostOption.cloudBesides([checkout(on: "mac")], in: [mac], continuationDevice: nil))
    }

    func testCloudWithItsOwnCheckoutIsListedOnce() {
        XCTAssertNil(HostOption.cloudBesides([checkout(on: "mac"), checkout(on: "cloud")], in: [mac, cloud], continuationDevice: nil))
    }

    func testAContinuationStaysOnItsComputer() {
        XCTAssertNil(HostOption.cloudBesides([checkout(on: "mac")], in: [mac, cloud], continuationDevice: "mac"))
        XCTAssertEqual(HostOption.cloudBesides([], in: [mac, cloud], continuationDevice: "cloud"), cloud)
    }
}
