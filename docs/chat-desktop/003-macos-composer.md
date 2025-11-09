# Issue #3: Build macOS Composer Component (NSTextView-based Input)

## Phase
Phase 1: Core Infrastructure

## Priority
Critical - Required for chat interaction

## Description
Create a macOS-native text input component for sending messages to agents, adapting the iOS `Composer` component for AppKit/NSTextView.

## Status
Completed (implemented on main)

Addendum
- BridgeManager chat-state unification (Issue #2) is complete, so the composer calls the shared `PromptDispatcher` on mac.
- Visual: OATheme dark surfaces only (no Liquid Glass) per current directive; glass can return later.
- Layout: Composer overlays bottom of content so centered content remains fixed while input grows.

## Current State
- iOS has `Composer.swift` using UIKit's `UITextView`
- No text input component exists for macOS
- macOS needs AppKit `NSTextView` or native SwiftUI `TextField`/`TextEditor`

## Target State
- Native macOS text input component with:
  - Berkeley Mono font
  - Dark theme styling matching iOS composer
  - Return/Enter key sends message
  - Shift+Return for new line
  - Placeholder text when empty
  - Auto-growing height (up to ~6 lines)
  - Send button with hover states
  - Disabled state during agent processing

## Acceptance Criteria
- [x] Create `ComposerMac.swift` component
- [x] Use `NSViewRepresentable` wrapping `NSTextView`
- [x] Apply Berkeley Mono font (AppKit via `BerkeleyFont.defaultName()`; see AGENTS.md)
- [x] Implement return key = send, shift+return = newline
- [x] Add placeholder overlay when empty ("Ask OpenAgents")
- [x] Auto-grow height with max of ~6 lines
- [x] Add send button (dark OATheme capsule variant; glass optional later)
- [x] Disable input when `isSending` is true
- [x] Use OATheme dark gray background (no glass)
- [x] Integrate with `BridgeManager` shared dispatcher
- [x] Basic styling aligns with iOS composer

## Technical Details

### Approach A: NSViewRepresentable + NSTextView (Recommended)
More control over keyboard behavior and text rendering.

```swift
// ios/OpenAgents/Views/macOS/ComposerMac.swift
struct ComposerMac: View {
    @Binding var text: String
    let placeholder: String
    let isSending: Bool
    let onSend: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            ZStack(alignment: .topLeading) {
                // NSTextView wrapper
                NSTextViewWrapper(
                    text: $text,
                    font: OAFonts.mono(size: 14),
                    onReturn: handleReturn
                )
                .frame(minHeight: 36, maxHeight: 144) // ~6 lines

                // Placeholder
                if text.isEmpty {
                    Text(placeholder)
                        .font(OAFonts.mono(size: 14))
                        .foregroundColor(OATheme.Colors.textSecondary)
                        .padding(.leading, 4)
                        .padding(.top, 8)
                        .allowsHitTesting(false)
                }
            }

            // Floating send button (Liquid Glass pattern)
            FloatingSendButton(
                disabled: text.isEmpty || isSending,
                action: onSend
            )
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(OATheme.Colors.background)
        )
    }

    private func handleReturn() {
        guard !text.isEmpty && !isSending else { return }
        onSend()
    }
}

struct NSTextViewWrapper: NSViewRepresentable {
    @Binding var text: String
    let font: NSFont
    let onReturn: () -> Void

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let textView = scrollView.documentView as! NSTextView

        textView.delegate = context.coordinator
        textView.font = font
        textView.textColor = NSColor(OATheme.Colors.textPrimary)
        textView.backgroundColor = .clear
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false

        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        let textView = scrollView.documentView as! NSTextView
        if textView.string != text {
            textView.string = text
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text, onReturn: onReturn)
    }

    class Coordinator: NSObject, NSTextViewDelegate {
        @Binding var text: String
        let onReturn: () -> Void

        init(text: Binding<String>, onReturn: @escaping () -> Void) {
            _text = text
            self.onReturn = onReturn
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            text = textView.string
        }

        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                // Check if Shift is held
                if NSEvent.modifierFlags.contains(.shift) {
                    return false // Allow newline
                } else {
                    onReturn() // Send message
                    return true
                }
            }
            return false
        }
    }
}
```

### Approach B: Pure SwiftUI TextEditor (Simpler, Less Control)
```swift
struct ComposerMac: View {
    @Binding var text: String
    let placeholder: String
    let isSending: Bool
    let onSend: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            ZStack(alignment: .topLeading) {
                TextEditor(text: $text)
                    .font(OAFonts.mono(size: 14))
                    .scrollContentBackground(.hidden)
                    .background(Color.clear)
                    .frame(minHeight: 36, maxHeight: 144)
                    .onSubmit(onSend) // Limited keyboard control

                if text.isEmpty {
                    Text(placeholder)
                        .font(OAFonts.mono(size: 14))
                        .foregroundColor(OATheme.Colors.textSecondary)
                        .padding(.leading, 4)
                        .padding(.top, 8)
                        .allowsHitTesting(false)
                }
            }

            Button(action: onSend) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 24))
                    .foregroundColor(text.isEmpty ? OATheme.Colors.textSecondary : OATheme.Colors.accent)
            }
            .buttonStyle(.plain)
            .disabled(text.isEmpty || isSending)
        }
        .padding(12)
        .background(OATheme.Colors.background)
        .cornerRadius(12)
    }
}
```

**Recommendation: Use Approach A (NSTextView)** for better keyboard control and consistency with iOS.

### Floating Send Button (Dark theme variant)

Create a capsule button consistent with the all-black OATheme directive:

```swift
struct FloatingSendButton: View {
    let disabled: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            // Foreground (icon + padding) defines size
            Image(systemName: "arrow.up")
                .renderingMode(.template)
                .symbolRenderingMode(.monochrome)
                .foregroundStyle(.white)  // Explicit color, not environment tint
                .font(.system(size: 16, weight: .semibold))
                .frame(width: 36, height: 36)  // Square hit area
        }
        .buttonStyle(.plain)
        .background(Capsule().fill(OATheme.Colors.background))
        .overlay {
            // Subtle border for separation
            Capsule()
                .strokeBorder(
                    OATheme.Colors.textSecondary.opacity(0.3),
                    lineWidth: 0.5
                )
        }
        .clipShape(Capsule())
        .shadow(
            color: Color.black.opacity(0.25),
            radius: 8,
            x: 0,
            y: 4
        )
        .opacity(disabled ? 0.5 : 1.0)
        .disabled(disabled)
    }
}
```

**Why this works:**
- Matches all-black OATheme (no glass/vibrancy).
- Template symbol with explicit color avoids environment tint darkening.
- Subtle border and shadow lift the button on dark surfaces.

Optional later: reintroduce a `GlassEffectContainer` if/when Liquid Glass returns.

### Files to Create
- `ios/OpenAgents/Views/macOS/ComposerMac.swift`

### Integration Example
```swift
// In ChatAreaPlaceholderView or ChatMacOSView
struct ChatAreaView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var messageText = ""
    @State private var isSending = false

    var body: some View {
        VStack(spacing: 0) {
            // Messages area (from Issue #5)
            ScrollView {
                // ...
            }

            // Composer at bottom
            ComposerMac(
                text: $messageText,
                placeholder: "Ask an agent...",
                isSending: isSending,
                onSend: sendMessage
            )
            .padding()
        }
    }

    private func sendMessage() {
        bridgeManager.sendPrompt(text: messageText)
        messageText = ""
    }
}
```

## Dependencies
- Issue #2 (BridgeManager chat state / dispatcher wiring)

## Blocked By
- Issue #2

## Blocks
- Issue #5 (Main chat area - needs composer for sending)
- Issue #11 (Chat integration - needs working input)

## Estimated Complexity
Medium (3-4 hours)

## Testing Requirements
- [ ] Build succeeds for macOS target
- [ ] Return key sends message
- [ ] Shift+Return inserts newline
- [ ] Berkeley Mono font renders correctly
- [ ] Placeholder appears when empty
- [ ] Auto-grows up to max height
- [ ] Send button disabled when empty or sending
- [ ] Integration with BridgeManager works
- [ ] Manual testing: type, send, verify message appears in chat

## References
- iOS Composer: `ios/OpenAgents/Views/Composer.swift`
- Liquid Glass floating buttons: `docs/liquid-glass/floating-buttons.md`
- Liquid Glass APIs: `docs/liquid-glass/apis-and-implementation.md`
- OAFonts: `ios/OpenAgents/Theme/Fonts.swift`
- OATheme: `ios/OpenAgents/Theme/Theme.swift`
- NSTextView docs: https://developer.apple.com/documentation/appkit/nstextview
- FloatingToolbar example: `ios/OpenAgents/FloatingToolbar.swift`
