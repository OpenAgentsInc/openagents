// Firm reason: 6 (XCTest presents the installed UIKit editor sheet for a look review; it asserts no layout).
import XCTest
@testable import OpenAgents

/// Shows the world editor's sheet over the app with a small fixed model, for the look walk
/// (StarterUITests/LookUITests.swift, #13580). It writes NAME.want into OA_LOOK_DIR and waits for
/// `xcrun simctl io DEVICE screenshot` to answer with NAME.png. Without OA_LOOK_DIR it does nothing.
@MainActor final class LookTests:XCTestCase {
    private func stop(_ name:String)async {
        guard let directory=ProcessInfo.processInfo.environment["OA_LOOK_DIR"] else {return}
        try? await Task.sleep(for:.seconds(1.2))
        let want=directory+"/"+name+".want";try? FileManager.default.removeItem(atPath:directory+"/"+name+".png")
        FileManager.default.createFile(atPath:want,contents:Data())
        for _ in 0..<300 where FileManager.default.fileExists(atPath:want) {try? await Task.sleep(for:.milliseconds(100))}
    }
    func testEditorSheet()async throws {
        guard ProcessInfo.processInfo.environment["OA_LOOK_DIR"] != nil else {return}
        let directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let scene=try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap {$0 as? UIWindowScene}.first)
        let root=try XCTUnwrap(scene.windows.first {$0.isKeyWindow}?.rootViewController ?? scene.windows.first?.rootViewController)
        let editor=try NativeWorldEditor(directory:directory,network:NativeWorldNetwork(),send:{_,_ in})
        editor.modalPresentationStyle = .pageSheet;editor.sheetPresentationController?.detents=[.medium(),.large()];editor.sheetPresentationController?.prefersGrabberVisible=true
        root.present(editor,animated:false)
        editor.model("editor.model.v1 splice dev 7 2 0 1\ncols x z yaw\nk 1001 Cargo crate\ne 1 1001 350 0 -1200 90 100\n")
        await stop("19-editor-sheet")
        editor.sheetPresentationController?.animateChanges {editor.sheetPresentationController?.selectedDetentIdentifier = .large}
        await stop("20-editor-sheet-large")
        root.dismiss(animated:false)
    }
}
