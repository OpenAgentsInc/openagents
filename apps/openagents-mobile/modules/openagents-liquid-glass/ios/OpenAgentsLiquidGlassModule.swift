import ExpoModulesCore

// OpenAgents mobile (GL-2 #8648, #8597) — SwiftUI Liquid Glass chrome module.
//
// Effect Native SwiftUI seam per
// docs/effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md and
// the hybrid decision (docs/fable/2026-07-09-swiftui-expo-ui-and-the-effect-
// native-stdlib.md): SwiftUI mounted inside the React Native shell at
// per-component UIHostingController boundaries; events OUT as single named
// events the shell converts into typed Effect Native intents; state IN as
// serializable props. Three chrome views:
//
// - GlassIconButton: circular Liquid Glass icon button (SF symbol).
// - GlassPill: capsule Liquid Glass pill with a label.
// - GlassComposer: the floating composer bar (plus, placeholder, mic).
//
// The former single test island (builds 105/106) grew into this real product
// chrome; the intent/props discipline is unchanged.
public class OpenAgentsLiquidGlassModule: Module {
  public func definition() -> ModuleDefinition {
    Name("OpenAgentsLiquidGlass")

    View(GlassIconButtonView.self) {
      ViewName("GlassIconButton")

      Events("onTap")

      Prop("symbol") { (view: GlassIconButtonView, symbol: String) in
        view.state.symbol = symbol
      }

      Prop("accessibilityLabelText") { (view: GlassIconButtonView, label: String) in
        view.state.accessibilityLabelText = label
      }
    }

    View(GlassPillView.self) {
      ViewName("GlassPill")

      Events("onTap")

      Prop("label") { (view: GlassPillView, label: String) in
        view.state.label = label
      }

      Prop("symbol") { (view: GlassPillView, symbol: String) in
        view.state.symbol = symbol
      }
    }

    View(GlassComposerView.self) {
      ViewName("GlassComposer")

      Events("onTapComposer", "onTapMic", "onTapPlus")

      Prop("placeholder") { (view: GlassComposerView, placeholder: String) in
        view.state.placeholder = placeholder
      }
    }
  }
}
