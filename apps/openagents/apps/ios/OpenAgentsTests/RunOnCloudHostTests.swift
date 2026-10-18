import XCTest
@testable import OpenAgents

/// Picking a project leaves every online computer available (#12133).
final class RunOnComputerTests: XCTestCase {
    private let mac = HostOption(id: "mac", name: "Studio Mac", online: true)
    private let server = HostOption(id: "server", name: "coderos", online: true)
    private let offline = HostOption(id: "offline", name: "MacBook", online: false)
    private let cloud = HostOption(id: "cloud", name: AppModel.cloudHostName, online: true)

    private func checkout(on device: String) -> ProjectOption {
        ProjectOption(id: "p-\(device)", name: "openagents", path: "~/work/openagents", device: device, deviceName: device,
                      online: true, git: true, groupKey: "openagents", groupName: "openagents", colorIndex: 0)
    }

    func testCloudIsOfferedBesidesAProjectsCheckouts() {
        let picks = HostOption.besides([checkout(on: "mac")], in: [mac, cloud], selectedDevice: "mac", continuationDevice: nil)
        XCTAssertEqual(picks, [cloud])
        XCTAssertEqual(picks.first?.displayName, "OpenAgents Cloud")
    }

    func testEveryOnlineComputerWithoutACheckoutIsOffered() {
        XCTAssertEqual(HostOption.besides([checkout(on: "mac")], in: [mac, server, cloud, offline], selectedDevice: "mac", continuationDevice: nil), [server, cloud])
    }

    func testSelectedOfflineComputerStaysVisible() {
        XCTAssertEqual(HostOption.besides([], in: [mac, offline], selectedDevice: "offline", continuationDevice: nil), [mac, offline])
    }

    func testCloudWithItsOwnCheckoutIsListedOnce() {
        XCTAssertTrue(HostOption.besides([checkout(on: "mac"), checkout(on: "cloud")], in: [mac, cloud], selectedDevice: "mac", continuationDevice: nil).isEmpty)
    }

    func testAContinuationStaysOnItsComputer() {
        XCTAssertTrue(HostOption.besides([checkout(on: "mac")], in: [mac, server, cloud], selectedDevice: "mac", continuationDevice: "mac").isEmpty)
        XCTAssertEqual(HostOption.besides([], in: [mac, cloud], selectedDevice: "cloud", continuationDevice: "cloud"), [cloud])
    }
}
