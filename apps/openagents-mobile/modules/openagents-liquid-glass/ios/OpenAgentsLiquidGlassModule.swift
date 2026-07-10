import ExpoModulesCore

// OpenAgents mobile (#8597) — SwiftUI "Liquid Glass" island Expo module.
//
// Effect Native SwiftUI seam test per
// docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md:
// SwiftUI mounted inside the React Native shell at a per-component boundary
// (audit interop case 2, UIHostingController), with events flowing OUT as a
// single named event that the shell converts into a typed Effect Native
// intent, and state flowing IN as serializable props. No JSX children, no
// open-ended modifier bag — a bounded, typed island.
public class OpenAgentsLiquidGlassModule: Module {
  public func definition() -> ModuleDefinition {
    Name("OpenAgentsLiquidGlass")

    View(OpenAgentsLiquidGlassView.self) {
      Events("onGlassTap")

      Prop("title") { (view: OpenAgentsLiquidGlassView, title: String) in
        view.state.title = title
      }

      Prop("subtitle") { (view: OpenAgentsLiquidGlassView, subtitle: String) in
        view.state.subtitle = subtitle
      }

      Prop("buttonLabel") { (view: OpenAgentsLiquidGlassView, buttonLabel: String) in
        view.state.buttonLabel = buttonLabel
      }

      Prop("tapCount") { (view: OpenAgentsLiquidGlassView, tapCount: Int) in
        view.state.tapCount = tapCount
      }
    }
  }
}
