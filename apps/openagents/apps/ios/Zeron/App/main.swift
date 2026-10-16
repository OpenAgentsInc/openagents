import UIKit

// Before UIKit starts: this bundle ID (com.openagents.app) used to belong to a
// SwiftUI app. iOS keeps a window-scene session per app across updates, and
// it reconnects a saved session with the delegate class it archived, without
// asking `configurationForConnecting`. The old app's archived delegate is
// SwiftUI's AppSceneDelegate, which this UIKit app can't drive: a black screen,
// then a crash in `stateRestorationActivity(for:)` (TestFlight 1.0.0 (60)).
// Discard the inherited scene state once, so the first scene is created with
// `AppDelegate.windowConfiguration(for:)`.
SceneStateMigration.discardInheritedState()

UIApplicationMain(
    CommandLine.argc,
    CommandLine.unsafeArgv,
    nil,
    NSStringFromClass(AppDelegate.self)
)
