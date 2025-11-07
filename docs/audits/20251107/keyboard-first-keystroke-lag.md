# iOS Keyboard First-Keystroke Lag Analysis

**Date**: 2025-11-07
**Auditor**: Claude Code
**Issue**: 1-2 second lag when typing first character after keyboard opens on new chat screen

## Executive Summary

After opening the keyboard for the first time on the NewChatView/Composer, users experience a 1-2 second lag when typing the first character. Despite multiple optimization attempts (font preloading, haptics warmup, text subsystem priming), the lag persists. This audit analyzes the current implementation and identifies potential root causes.

## Affected Code Paths

### Primary Components
- `ios/OpenAgents/Views/Composer.swift` - UITextView-based input component
- `ios/OpenAgents/PerformanceWarmup.swift` - Various warmup strategies
- `ios/OpenAgents/Views/NewChatView.swift` - Main chat interface using Composer
- `ios/OpenAgents/SimplifiedIOSView.swift` - Navigation container

### Flow Sequence
1. User taps Composer text field
2. `becomeFirstResponder()` called → `textViewDidBeginEditing()` triggered
3. Keyboard animation begins (~250-300ms)
4. Priming code runs: insert "a" + delete after 10ms delay
5. Keyboard animation completes
6. User types first character
7. **1-2 second lag occurs here** ⚠️
8. Character appears

## Current Mitigation Attempts

### 1. Font Warmup (`PerformanceWarmup.preloadMonoFont()`)
**Location**: `PerformanceWarmup.swift:9-28`
**Strategy**: Pre-render full ASCII character set with Berkeley Mono font using both NSAttributedString and UILabel to warm CoreText/TextKit caches.

```swift
let ascii = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789" +
            " ~`!@#$%^&*()-_=+[{]}\\|;:'\",<.>/?\n \t"
let attr = NSAttributedString(string: ascii, attributes: [.font: font, .foregroundColor: UIColor.white])
// Render to offscreen bitmap
```

**Effectiveness**: ✅ Should eliminate font loading lag
**Timing**: Called at app launch (AppDelegate) and onAppear

### 2. Haptics Warmup (`PerformanceWarmup.prewarmHaptics()`)
**Location**: `PerformanceWarmup.swift:31-34`
**Strategy**: Initialize UIFeedbackGenerator and CHHapticEngine early to front-load their initialization cost.

**Effectiveness**: ✅ Should eliminate haptic feedback lag
**Timing**: Called at app launch

### 3. Keyboard & Text Input Warmup (`PerformanceWarmup.prewarmKeyboardAndTextInput()`)
**Location**: `PerformanceWarmup.swift:38-69`
**Strategy**: Create offscreen UITextView, perform layout, touch UITextInputMode and UITextChecker without showing keyboard.

```swift
let tv = UITextView(frame: CGRect(x: -1000, y: -1000, width: 10, height: 10))
// Configure with same settings as Composer
tv.setNeedsLayout(); tv.layoutIfNeeded()
_ = UITextInputMode.activeInputModes
_ = UITextChecker.availableLanguages  // Added in commit 05df3272
tv.removeFromSuperview()  // ⚠️ Never becomes first responder
```

**Effectiveness**: ⚠️ Partial - touches some subsystems but doesn't exercise full keyboard event path
**Timing**: Called at applicationDidBecomeActive and onAppear

### 4. Silent Responder Warmup (`PerformanceWarmup.prewarmResponderSilently()`)
**Location**: `PerformanceWarmup.swift:75-95`
**Strategy**: Become first responder with custom empty inputView to prime responder chain without showing system keyboard.

```swift
let tv = UITextView(...)
tv.inputView = UIView(frame: CGRect(x: 0, y: 0, width: 1, height: 1)) // suppress keyboard
tv.becomeFirstResponder()
// Delay 150ms, then resign
```

**Effectiveness**: ✅ Warms up responder chain
**Runs Once**: Static flag prevents multiple runs
**Timing**: Called at app launch (100ms delay) and onAppear

### 5. In-Focus Priming (`Composer.Coordinator.textViewDidBeginEditing()`)
**Location**: `Composer.swift:125-138`
**Strategy**: After user focuses text field, programmatically insert and delete a character to "prime the candidate/autocorrection pipeline."

```swift
if !didPrime, textView.text.isEmpty {
    didPrime = true
    isPriming = true
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
        textView.insertText("a")
        textView.deleteBackward()
        self.isPriming = false
    }
}
```

**Effectiveness**: ⚠️ **May not be effective** - see analysis below
**Timing**: 10ms after focus (before keyboard animation completes)
**Commit**: da10a9e7 (most recent attempt)

### 6. Disabled Features in Composer
**Location**: `Composer.swift:31-45`

All of the following are explicitly disabled to reduce overhead:
- `autocorrectionType = .no`
- `spellCheckingType = .no`
- `autocapitalizationType = .none`
- `smartDashesType = .no`
- `smartQuotesType = .no`
- `smartInsertDeleteType = .no`
- `dataDetectorTypes = []`
- QuickType bar (inputAssistantItem groups cleared)
- Text drag interaction disabled
- `allowsEditingTextAttributes = false`

**Effectiveness**: ✅ Should eliminate smart feature overhead

## Root Cause Analysis

### Why Existing Mitigations May Not Be Working

#### 1. Programmatic vs. Keyboard Event Input Path Divergence
The in-focus priming uses `textView.insertText("a")` which is **programmatic text insertion**. This may not exercise the same code path as **real keyboard event processing**:

- **Programmatic path**: `insertText()` → Text storage → Layout
- **Keyboard event path**: Keyboard event → UIKeyboardImpl (private) → Input method coordination → Text traits validation → Input delegate callbacks → `shouldChangeTextIn` → Text storage → Layout

**Hypothesis**: The keyboard event processing pipeline has lazy initialization that programmatic insertion doesn't trigger.

#### 2. Timing of Priming
The priming happens at t=0.01s after focus, well before:
- Keyboard animation completes (~0.25-0.3s)
- User can physically type (~0.3s+)

**Hypothesis**: Some subsystems may only initialize when the keyboard is actually visible/interactive, making the early priming ineffective.

#### 3. Undo Manager Not Disabled
`UITextView` has an undo manager by default, and the first text operation initializes the undo stack.

**Evidence**: No `allowsUndo = false` in Composer configuration
**Hypothesis**: First user input registers undo, causing initialization lag

#### 4. Text Kit Layout Deferred Calculation
Even with warmup, the first "real" layout with user-entered text might trigger:
- Typing attributes calculation
- Paragraph style resolution
- Dynamic Type scaling
- Accessibility text size adjustments

**Hypothesis**: First actual text insertion triggers full layout calculation that offscreen warmup didn't exercise

#### 5. Input Method Editor (IME) Coordination
Even though `keyboardType = .asciiCapable`, iOS still has IME framework involvement:
- Input method context creation
- Keyboard layout manager initialization
- Text input traits validation
- Marked text range management (for multi-stage input)

**Hypothesis**: IME framework has first-use initialization on real keyboard input

#### 6. Accessibility Services
First keystroke might trigger:
- VoiceOver announcement preparation
- Dictation service check
- Keyboard accessibility features
- Text-to-speech engine warmup

**Hypothesis**: Accessibility subsystems initialize on first real user input

#### 7. UITextView Internal State Machine
UITextView might have internal state that only transitions on genuine user input:
- Selection management
- Editing state flags
- Text change notification coalescing
- Gesture recognizer coordination

### What We Know Works
✅ Font loading is warmed up (extensive ASCII render)
✅ Haptics are pre-initialized
✅ Responder chain is exercised
✅ Smart features are disabled

### What Remains Untested
❓ Undo manager initialization
❓ Real keyboard event → text insertion pipeline
❓ Input method editor coordination
❓ Accessibility service integration
❓ Text Kit layout manager first-pass calculation
❓ Dynamic Type / text scaling

## Diagnostic Recommendations

### 1. Add Time Profiling
Insert instrumentation to measure where time is spent:

```swift
// In Composer.Coordinator
func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
    let start = CFAbsoluteTimeGetCurrent()
    defer {
        let elapsed = CFAbsoluteTimeGetCurrent() - start
        if elapsed > 0.016 { // More than 1 frame at 60fps
            print("[Composer] shouldChangeTextIn took \(elapsed * 1000)ms")
        }
    }
    // ... existing code
}

func textViewDidChange(_ textView: UITextView) {
    let start = CFAbsoluteTimeGetCurrent()
    defer {
        let elapsed = CFAbsoluteTimeGetCurrent() - start
        if elapsed > 0.016 {
            print("[Composer] textViewDidChange took \(elapsed * 1000)ms")
        }
    }
    // ... existing code
}
```

### 2. Use Instruments Time Profiler
Run the app with Xcode Instruments Time Profiler:
1. Set breakpoint before typing first character
2. Start Time Profiler recording
3. Type first character
4. Stop recording
5. Analyze call tree to find hot path

Look for:
- UIKit private framework calls
- Text framework initialization
- Input method services
- CoreText layout

### 3. Test Undo Manager Hypothesis
Try disabling undo manager in Composer.makeUIView():

```swift
textView.allowsUndo = false
```

### 4. Test Real Keyboard Priming
Instead of programmatic insertText(), try simulating a keyboard event:

```swift
// This is more invasive but closer to real keyboard input
if !didPrime, textView.text.isEmpty {
    didPrime = true
    isPriming = true
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
        // Programmatic insert + delete
        textView.insertText("a")
        textView.deleteBackward()
        self.isPriming = false
    }
}
```

Increase delay to 100ms (after keyboard is fully visible) to see if timing matters.

### 5. Monitor Background Thread Activity
Check if first keystroke triggers background work:

```swift
// Add in AppDelegate
DispatchQueue.global(qos: .default).async {
    Thread.current.name = "Monitor"
    while true {
        Thread.sleep(forTimeInterval: 0.1)
        print("[Monitor] Active threads: \(Thread.current.threadDictionary)")
    }
}
```

### 6. Test with Minimal UITextView
Create a test app with absolutely minimal UITextView configuration to isolate the issue:
- No custom font (system font)
- No SwiftUI UIViewRepresentable wrapper
- Direct UIViewController with UITextView
- No bridge/model binding

If lag persists, it's a framework issue. If not, something in Composer/SwiftUI integration is the cause.

## Potential Solutions (Untested)

### Solution 1: Disable Undo Manager
**File**: `Composer.swift:46` (add after line 45)
```swift
textView.allowsUndo = false
```

### Solution 2: Exercise Full Input Pipeline in Warmup
**File**: `PerformanceWarmup.swift` (modify prewarmResponderSilently)
```swift
tv.becomeFirstResponder()
// Wait for keyboard to be ready, then simulate input
DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
    tv.insertText("a")
    tv.deleteBackward()
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
        tv.resignFirstResponder()
        tv.removeFromSuperview()
    }
}
```

### Solution 3: Delay Priming Until Keyboard Visible
**File**: `Composer.swift:125-138`
```swift
func textViewDidBeginEditing(_ textView: UITextView) {
    placeholder?.isHidden = true
    if !didPrime, textView.text.isEmpty {
        didPrime = true
        isPriming = true
        // Wait for keyboard animation to complete
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            textView.insertText("a")
            textView.deleteBackward()
            self.isPriming = false
        }
    }
}
```

### Solution 4: Pre-Create Persistent UITextView
Keep a hidden UITextView alive throughout the app lifecycle:
```swift
// In AppDelegate or SceneDelegate
private var warmupTextView: UITextView?

func application(_ application: UIApplication, didFinishLaunchingWithOptions...) {
    // Create and keep alive
    warmupTextView = UITextView()
    warmupTextView?.font = UIFont(name: BerkeleyFont.defaultName(), size: 16)
    warmupTextView?.autocorrectionType = .no
    // ... configure like Composer

    // Add to window but keep offscreen/hidden
    if let window = application.windows.first {
        warmupTextView?.frame = CGRect(x: -1000, y: -1000, width: 100, height: 40)
        window.addSubview(warmupTextView!)
        warmupTextView?.becomeFirstResponder()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.warmupTextView?.resignFirstResponder()
        }
    }
}
```

### Solution 5: Accept First-Keystroke Lag and Optimize Perception
If framework lag is unavoidable, optimize perceived performance:
- Show typing indicator/animation during lag
- Optimistically show character immediately (before framework processes it)
- Use haptic feedback to mask delay

## Related Files

### Modified in Recent Commits
- `ios/OpenAgents/Views/Composer.swift` - Priming logic added
- `ios/OpenAgents/PerformanceWarmup.swift` - Multiple warmup strategies
- `ios/OpenAgents/AppDelegate.swift` - Warmup orchestration
- `ios/OpenAgents/OpenAgentsApp.swift` - Warmup orchestration

### Relevant Commit History
```
da10a9e7 Prime candidate pipeline on first focus: insert/delete a char once to eliminate first-type lag (no visible change)
05df3272 Aggressive font/text warmup: render full ASCII sample and touch UITextChecker to remove first-typed-character lag
2f7ef9d4 Fix message flicker: convert pending echo to real session on session/new; Codex args: drop --continue for codex
f8f81fa7 Timeline/session isolation: don't auto-switch session on incoming updates; show only 'pending' echoes when no active session; add unit test
c4389004 Add true placeholder (overlay label) without mutating text; improve warmup
c09138c0 Composer: remove placeholder logic; eliminate typing lag and cursor issues
9d0aae6a Prewarm keyboard show/dismiss with hidden UITextView to remove first-open/close lag
83e8fb33 Prewarm mono font and haptics; reduce keyboard overhead
49f869d7 Fix iOS composer lag; use mono; add tap-to-dismiss
```

## Conclusion

Despite extensive warmup efforts across multiple subsystems (fonts, haptics, text input, responder chain), the 1-2 second first-keystroke lag persists. The root cause is likely one or more of:

1. **Undo manager initialization** (most likely - simple to test)
2. **Keyboard event pipeline divergence** from programmatic insertion
3. **Input method editor coordination** on first real keyboard event
4. **Accessibility services** lazy initialization
5. **Text Kit layout manager** deferred first-pass calculation

**Next Steps**:
1. Add time profiling instrumentation (immediate)
2. Test undo manager disable (quick win if effective)
3. Run Instruments Time Profiler to identify hot path
4. Consider framework bug report to Apple if no user-space solution found

## Appendix: Test Procedure

To reproduce the issue:
1. Clean build and run on iOS simulator or device
2. Kill and relaunch app (fresh start)
3. Navigate to NewChatView (default view if Features.simplifiedIOSUI is true)
4. Tap the Composer text field
5. Wait for keyboard animation to complete
6. Type a single character (e.g., "h")
7. Observe 1-2 second lag before character appears

**Environment**:
- Xcode 16.0+
- iOS 16.0+ simulator or device
- Default configuration (no env vars set)
