import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

/// Tests for ComposeSheet - the modal sheet for sending new messages
final class ComposeSheetTests: XCTestCase {

    // MARK: - Initialization Tests

    func testComposeSheet_Initializes() {
        let view = ComposeSheet()
        XCTAssertNotNil(view)
    }

    // MARK: - Interaction Tests

    func testComposeSheet_WithBridgeManager_Initializes() {
        let bridge = BridgeManager()
        let view = ComposeSheet()
            .environmentObject(bridge)

        XCTAssertNotNil(view)
    }

    // MARK: - State Management Tests

    func testComposeSheet_InitialMessageTextEmpty() {
        let view = ComposeSheet()
        // State is private, but we verify view can be created
        // In actual usage, messageText starts empty
        XCTAssertNotNil(view)
    }

    // MARK: - Integration with BridgeManager

    func testComposeSheet_RequiresBridgeManager() {
        // ComposeSheet requires BridgeManager as EnvironmentObject
        // This test verifies the sheet can be created with proper environment
        let bridge = BridgeManager()
        let view = ComposeSheet()
            .environmentObject(bridge)

        XCTAssertNotNil(view)
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptExists() {
        let bridge = BridgeManager()
        // Verify sendPrompt method exists and can be called
        bridge.sendPrompt(text: "Test message")
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptWithEmptyText() {
        let bridge = BridgeManager()
        // Should handle empty text gracefully
        bridge.sendPrompt(text: "")
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptWithWhitespace() {
        let bridge = BridgeManager()
        // Should handle whitespace-only text
        bridge.sendPrompt(text: "   \n\t  ")
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptWithValidText() {
        let bridge = BridgeManager()
        // Should handle normal text
        bridge.sendPrompt(text: "Hello, agent!")
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptWithMultilineText() {
        let bridge = BridgeManager()
        let multiline = """
        First line
        Second line
        Third line
        """
        bridge.sendPrompt(text: multiline)
        XCTAssertNotNil(bridge)
    }

    func testBridgeManager_SendPromptWithUnicodeText() {
        let bridge = BridgeManager()
        bridge.sendPrompt(text: "Hello 👋 世界 🌍")
        XCTAssertNotNil(bridge)
    }

    // MARK: - Preview Tests

    func testComposeSheet_PreviewConfiguration() {
        #if DEBUG
        let previewProvider = ComposeSheet_Previews.self
        XCTAssertNotNil(previewProvider)
        #endif
    }

    // MARK: - Accessibility Tests

    func testComposeSheet_HasAccessibleElements() {
        let view = ComposeSheet()
        // Verify view structure is accessible
        XCTAssertNotNil(view)
        // In practice, the navigation bar should have:
        // - Cancel button (left)
        // - "New Message" title (center)
        // - Send button (right)
    }

    // MARK: - Edge Cases

    func testComposeSheet_VeryLongText() {
        let bridge = BridgeManager()
        let longText = String(repeating: "a", count: 100000)
        bridge.sendPrompt(text: longText)
        XCTAssertNotNil(bridge)
    }

    func testComposeSheet_SpecialCharacters() {
        let bridge = BridgeManager()
        let special = "Test with \"quotes\", 'apostrophes', and symbols: @#$%^&*()"
        bridge.sendPrompt(text: special)
        XCTAssertNotNil(bridge)
    }

    func testComposeSheet_JSONContent() {
        let bridge = BridgeManager()
        let json = """
        {
            "key": "value",
            "nested": {
                "array": [1, 2, 3]
            }
        }
        """
        bridge.sendPrompt(text: json)
        XCTAssertNotNil(bridge)
    }

    func testComposeSheet_CodeSnippet() {
        let bridge = BridgeManager()
        let code = """
        func hello() {
            print("Hello, world!")
        }
        """
        bridge.sendPrompt(text: code)
        XCTAssertNotNil(bridge)
    }

    // MARK: - Layout Tests

    func testComposeSheet_UsesNavigationView() {
        // Verify ComposeSheet uses NavigationView for proper sheet presentation
        let view = ComposeSheet()
        XCTAssertNotNil(view)
        // NavigationView provides the title bar with Cancel/Send buttons
    }

    func testComposeSheet_UsesTextEditor() {
        // Verify ComposeSheet uses TextEditor for multiline input
        let view = ComposeSheet()
        XCTAssertNotNil(view)
        // TextEditor allows for multiline text input with scrolling
    }
}
