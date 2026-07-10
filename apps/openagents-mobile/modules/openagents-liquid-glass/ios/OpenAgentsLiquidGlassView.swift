import ExpoModulesCore
import SwiftUI

// Shared Protoss-blue accent (khalaTheme accent #3b82f6).
private let oaAccent = Color(red: 0x3b / 255.0, green: 0x82 / 255.0, blue: 0xf6 / 255.0)

// Generic ExpoView that owns a UIHostingController — the audit's prescribed
// mount boundary for SwiftUI islands inside a React Native shell.
private func embed<Root: View>(_ root: Root, in view: ExpoView) {
  let controller = UIHostingController(rootView: root)
  controller.view.backgroundColor = .clear
  view.addSubview(controller.view)
  controller.view.translatesAutoresizingMaskIntoConstraints = false
  NSLayoutConstraint.activate([
    controller.view.topAnchor.constraint(equalTo: view.topAnchor),
    controller.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    controller.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
    controller.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
  ])
}

// MARK: - GlassIconButton

final class GlassIconButtonState: ObservableObject {
  @Published var symbol: String = "circle"
  @Published var accessibilityLabelText: String = ""
}

final class GlassIconButtonView: ExpoView {
  let state = GlassIconButtonState()
  let onTap = EventDispatcher()

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    embed(GlassIconButtonRoot(state: state) { [weak self] in self?.onTap([:]) }, in: self)
  }
}

struct GlassIconButtonRoot: View {
  @ObservedObject var state: GlassIconButtonState
  let action: () -> Void

  var body: some View {
    Group {
      if #available(iOS 26.0, *) {
        Button(action: action) {
          Image(systemName: state.symbol)
            .font(.system(size: 17, weight: .medium))
            .foregroundStyle(.white)
            .frame(width: 44, height: 44)
        }
        .glassEffect(.regular.interactive(), in: .circle)
      } else {
        Button(action: action) {
          Image(systemName: state.symbol)
            .font(.system(size: 17, weight: .medium))
            .foregroundStyle(.white)
            .frame(width: 44, height: 44)
            .background(.ultraThinMaterial, in: Circle())
            .overlay(Circle().stroke(oaAccent.opacity(0.35), lineWidth: 1))
        }
      }
    }
    .accessibilityLabel(state.accessibilityLabelText)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

// MARK: - GlassPill (dropdown when options are provided)

struct GlassPillOption: Identifiable {
  let id: String
  let label: String
}

final class GlassPillState: ObservableObject {
  @Published var label: String = ""
  @Published var symbol: String = ""
  @Published var options: [GlassPillOption] = []
  @Published var selectedId: String = ""
}

final class GlassPillView: ExpoView {
  let state = GlassPillState()
  let onTap = EventDispatcher()
  let onSelect = EventDispatcher()

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    embed(
      GlassPillRoot(
        state: state,
        action: { [weak self] in self?.onTap([:]) },
        select: { [weak self] id in self?.onSelect(["id": id]) }
      ),
      in: self
    )
  }
}

struct GlassPillRoot: View {
  @ObservedObject var state: GlassPillState
  let action: () -> Void
  let select: (String) -> Void

  private var content: some View {
    HStack(spacing: 6) {
      if !state.symbol.isEmpty {
        Image(systemName: state.symbol)
          .font(.system(size: 14, weight: .semibold))
      }
      Text(state.label)
        .font(.system(size: 16, weight: .semibold))
      if !state.options.isEmpty {
        Image(systemName: "chevron.down")
          .font(.system(size: 11, weight: .semibold))
          .foregroundStyle(.white.opacity(0.6))
      }
    }
    .foregroundStyle(.white)
    .padding(.horizontal, 16)
    .frame(height: 44)
  }

  @ViewBuilder
  private var menuItems: some View {
    ForEach(state.options) { option in
      Button(action: { select(option.id) }) {
        if option.id == state.selectedId {
          Label(option.label, systemImage: "checkmark")
        } else {
          Text(option.label)
        }
      }
    }
  }

  var body: some View {
    Group {
      if state.options.isEmpty {
        // Plain tap pill (no dropdown).
        if #available(iOS 26.0, *) {
          Button(action: action) { content }
            .glassEffect(.regular.interactive(), in: .capsule)
        } else {
          Button(action: action) {
            content
              .background(.ultraThinMaterial, in: Capsule())
              .overlay(Capsule().stroke(oaAccent.opacity(0.35), lineWidth: 1))
          }
        }
      } else {
        // Dropdown pill: the system Menu (glass-treated by iOS 26 itself).
        if #available(iOS 26.0, *) {
          Menu { menuItems } label: { content }
            .glassEffect(.regular.interactive(), in: .capsule)
        } else {
          Menu { menuItems } label: {
            content
              .background(.ultraThinMaterial, in: Capsule())
              .overlay(Capsule().stroke(oaAccent.opacity(0.35), lineWidth: 1))
          }
        }
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

// MARK: - GlassComposer

final class GlassComposerState: ObservableObject {
  @Published var placeholder: String = "Ask anything"
}

final class GlassComposerView: ExpoView {
  let state = GlassComposerState()
  let onTapComposer = EventDispatcher()
  let onTapMic = EventDispatcher()
  let onTapPlus = EventDispatcher()

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    embed(
      GlassComposerRoot(
        state: state,
        onComposer: { [weak self] in self?.onTapComposer([:]) },
        onMic: { [weak self] in self?.onTapMic([:]) },
        onPlus: { [weak self] in self?.onTapPlus([:]) }
      ),
      in: self
    )
  }
}

struct GlassComposerRoot: View {
  @ObservedObject var state: GlassComposerState
  let onComposer: () -> Void
  let onMic: () -> Void
  let onPlus: () -> Void

  private var bar: some View {
    HStack(spacing: 12) {
      Button(action: onPlus) {
        Image(systemName: "plus")
          .font(.system(size: 17, weight: .medium))
          .foregroundStyle(.white)
          .frame(width: 34, height: 34)
      }
      .accessibilityLabel("Add")

      // Interim (GL-2): the composer is a tap target that dispatches a typed
      // intent; a real bound TextField lands with the Sarah conversation
      // surface. The placeholder is state-projected from the EN program.
      Text(state.placeholder)
        .font(.system(size: 16))
        .foregroundStyle(.white.opacity(0.55))
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .onTapGesture(perform: onComposer)
        .accessibilityLabel("Composer")

      Button(action: onMic) {
        Image(systemName: "mic")
          .font(.system(size: 17, weight: .medium))
          .foregroundStyle(.white)
          .frame(width: 34, height: 34)
      }
      .accessibilityLabel("Voice input")
    }
    .padding(.horizontal, 14)
    .frame(height: 54)
  }

  var body: some View {
    Group {
      if #available(iOS 26.0, *) {
        bar.glassEffect(.regular, in: .capsule)
      } else {
        bar
          .background(.ultraThinMaterial, in: Capsule())
          .overlay(Capsule().stroke(oaAccent.opacity(0.35), lineWidth: 1))
      }
    }
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

// MARK: - GlassOptionSheet (bottom fly-up menu)

struct GlassSheetOption: Identifiable {
  let id: String
  let label: String
  let price: String
}

final class GlassOptionSheetState: ObservableObject {
  @Published var title: String = ""
  @Published var options: [GlassSheetOption] = []
}

final class GlassOptionSheetView: ExpoView {
  let state = GlassOptionSheetState()
  let onSelect = EventDispatcher()
  let onDismiss = EventDispatcher()

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    embed(
      GlassOptionSheetRoot(
        state: state,
        select: { [weak self] id in self?.onSelect(["id": id]) },
        dismiss: { [weak self] in self?.onDismiss([:]) }
      ),
      in: self
    )
  }
}

struct GlassOptionSheetRoot: View {
  @ObservedObject var state: GlassOptionSheetState
  let select: (String) -> Void
  let dismiss: () -> Void

  private func row(_ option: GlassSheetOption) -> some View {
    Button(action: { select(option.id) }) {
      HStack {
        Text(option.label)
          .font(.system(size: 16, weight: .semibold))
          .foregroundStyle(.white)
        Spacer()
        Text(option.price)
          .font(.system(size: 15, weight: .semibold))
          .foregroundStyle(oaAccent)
      }
      .padding(.horizontal, 16)
      .frame(height: 44)
      .background(.white.opacity(0.07), in: RoundedRectangle(cornerRadius: 14))
    }
  }

  private var panel: some View {
    VStack(spacing: 10) {
      Capsule()
        .fill(.white.opacity(0.3))
        .frame(width: 36, height: 5)
        .padding(.top, 10)
      Text(state.title)
        .font(.system(size: 17, weight: .bold))
        .foregroundStyle(.white)
        .padding(.bottom, 2)
      ForEach(state.options) { option in
        row(option)
      }
      Button(action: dismiss) {
        Text("Not now")
          .font(.system(size: 15, weight: .medium))
          .foregroundStyle(.white.opacity(0.6))
          .frame(height: 36)
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
  }

  var body: some View {
    Group {
      if #available(iOS 26.0, *) {
        panel
          .glassEffect(.regular.tint(oaAccent.opacity(0.12)), in: .rect(cornerRadius: 28))
      } else {
        panel
          .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 28))
          .overlay(
            RoundedRectangle(cornerRadius: 28)
              .stroke(oaAccent.opacity(0.3), lineWidth: 1)
          )
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}
