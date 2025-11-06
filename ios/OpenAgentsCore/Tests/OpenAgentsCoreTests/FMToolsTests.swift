import XCTest
@testable import OpenAgentsCore

final class FMToolsTests: XCTestCase {
    func testFMToolsRegistryIncludesSessionTools() throws {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            let tools = FMToolsRegistry.defaultTools(workspaceRoot: ".")
            let names = tools.map { $0.name }
            XCTAssertTrue(names.contains("session.search"))
            XCTAssertTrue(names.contains("session.read"))
        }
        #endif
    }
}

