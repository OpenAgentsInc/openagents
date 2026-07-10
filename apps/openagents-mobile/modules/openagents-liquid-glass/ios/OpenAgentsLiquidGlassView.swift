import ExpoModulesCore
import SwiftUI

// Observable prop state pushed from the JS side (which itself is fed by the
// Effect Native view program's state). SwiftUI re-renders when these change.
final class LiquidGlassState: ObservableObject {
  @Published var title: String = ""
  @Published var subtitle: String = ""
  @Published var buttonLabel: String = "Tap"
  @Published var tapCount: Int = 0
}

// ExpoView owning a UIHostingController whose rootView renders the Liquid
// Glass SwiftUI content — the audit's prescribed mount boundary for a
// SwiftUI island inside a React Native shell.
final class OpenAgentsLiquidGlassView: ExpoView {
  let state = LiquidGlassState()
  let onGlassTap = EventDispatcher()

  private var hosting: UIHostingController<LiquidGlassRoot>?

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    let root = LiquidGlassRoot(state: state) { [weak self] in
      self?.onGlassTap([:])
    }
    let controller = UIHostingController(rootView: root)
    controller.view.backgroundColor = .clear
    hosting = controller
    addSubview(controller.view)
    controller.view.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      controller.view.topAnchor.constraint(equalTo: topAnchor),
      controller.view.bottomAnchor.constraint(equalTo: bottomAnchor),
      controller.view.leadingAnchor.constraint(equalTo: leadingAnchor),
      controller.view.trailingAnchor.constraint(equalTo: trailingAnchor),
    ])
  }
}

// The SwiftUI content: a glass card + a glass button on the Protoss-blue
// backdrop. Uses the REAL iOS 26 Liquid Glass APIs (.glassEffect /
// .buttonStyle(.glass)) when the device runs iOS 26+; earlier iOS falls back
// to .ultraThinMaterial glass-morphism so the island still renders honestly.
struct LiquidGlassRoot: View {
  @ObservedObject var state: LiquidGlassState
  let onTap: () -> Void

  private let accent = Color(red: 0x3b / 255.0, green: 0x82 / 255.0, blue: 0xf6 / 255.0)

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      glassCard
      glassButton
    }
    .padding(16)
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
  }

  private var cardContent: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(state.title)
        .font(.headline)
        .foregroundStyle(.white)
      Text(state.subtitle)
        .font(.subheadline)
        .foregroundStyle(.white.opacity(0.7))
      Text("Typed intents from SwiftUI: \(state.tapCount)")
        .font(.caption.weight(.semibold))
        .foregroundStyle(accent)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(16)
  }

  @ViewBuilder
  private var glassCard: some View {
    if #available(iOS 26.0, *) {
      cardContent
        .glassEffect(.regular.tint(accent.opacity(0.2)), in: .rect(cornerRadius: 16))
    } else {
      cardContent
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
        .overlay(
          RoundedRectangle(cornerRadius: 16)
            .stroke(accent.opacity(0.35), lineWidth: 1)
        )
    }
  }

  @ViewBuilder
  private var glassButton: some View {
    if #available(iOS 26.0, *) {
      Button(state.buttonLabel, action: onTap)
        .buttonStyle(.glass)
        .tint(accent)
    } else {
      Button(action: onTap) {
        Text(state.buttonLabel)
          .font(.body.weight(.semibold))
          .foregroundStyle(.white)
          .padding(.horizontal, 16)
          .padding(.vertical, 10)
          .background(.ultraThinMaterial, in: Capsule())
          .overlay(Capsule().stroke(accent.opacity(0.5), lineWidth: 1))
      }
    }
  }
}
