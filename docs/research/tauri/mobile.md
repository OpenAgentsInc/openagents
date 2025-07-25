# Adding mobile support to your Tauri desktop app

Tauri v2 enables you to extend your existing desktop application to iOS and Android platforms using primarily **web technologies** (JavaScript, HTML, CSS) while leveraging Rust for core logic. You won't need to write Swift or Kotlin for most use cases, as Tauri provides rich JavaScript APIs that handle native functionality. The framework uses a WebView-based architecture on mobile, similar to desktop, with platform-specific WebViews (WKWebView on iOS, Android System WebView on Android).

## Programming languages for Tauri mobile development

The beauty of Tauri's mobile support lies in its developer-friendly language approach. Your **frontend remains entirely in JavaScript/web technologies**, using any framework you prefer (React, Vue, Svelte, etc.). The **core application logic stays in Rust**, handling system interactions and business logic across all platforms. Native mobile languages (Swift for iOS, Kotlin/Java for Android) are **only required when creating custom plugins** that need deep platform integration beyond what Tauri's extensive JavaScript APIs provide.

According to official documentation, **"Developers do not need to have programming skills in Rust, Swift, or Kotlin as Tauri provides rich JavaScript APIs."** This makes mobile development accessible to web developers without requiring native mobile expertise. When you do need platform-specific functionality, Tauri's plugin system allows you to write native code modules that integrate seamlessly with your JavaScript frontend through a unified API.

## Development workflow for adding mobile support

Adding mobile support to your existing desktop app follows a structured migration process. First, ensure you have the latest Tauri CLI installed and run the automated migration command: `npm run tauri migrate`. This handles most v1 to v2 updates and prepares your project for mobile platforms.

The key structural change involves **converting your desktop entry point to a shared library**. Rename `src-tauri/src/main.rs` to `lib.rs` and update it with the mobile entry point attribute. Create a new `main.rs` that simply calls your library's run function. Add the necessary library configuration to your `Cargo.toml` to produce shared libraries for mobile platforms.

Initialize mobile support by running `npm run tauri android init` and `npm run tauri ios init`. These commands generate platform-specific project structures within your existing Tauri app, creating Android and iOS projects in the `src-tauri/gen/` directory. Your project maintains a single codebase while generating native mobile projects that wrap your web frontend and Rust backend.

## Mobile architecture and implementation

Tauri v2's mobile architecture employs a **WebView-based hybrid approach** that feels native while leveraging web technologies. The framework uses each platform's system WebView - WKWebView on iOS and Android System WebView - avoiding the overhead of bundled runtimes. This results in remarkably small app sizes, often under 600KB for the native wrapper.

The architecture consists of three main layers: your web frontend communicates with the Rust core through message passing (IPC), and when needed, the Rust core interfaces with platform-specific native code through Tauri's plugin system. The **WRY (WebView Rendering Library)** provides a unified interface to system WebViews across platforms, while **TAO** handles window management and application lifecycle.

Mobile plugins follow a split architecture with shared Rust logic and platform-specific implementations. When you need camera access, for example, you'd call a JavaScript API that invokes Rust code, which then bridges to Swift or Kotlin implementations that handle the actual camera interaction. This layered approach maintains code reusability while allowing platform-specific optimizations.

## Setting up mobile development step-by-step

The setup process requires platform-specific development environments. For **iOS development**, you need macOS with full Xcode installation (not just command line tools). Install the required Rust targets: `rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim`. An Apple Developer account ($99/year) is required for device testing and App Store distribution.

For **Android development**, install Android Studio with SDK Manager on any platform. Through the SDK Manager, install Android SDK Platform-Tools, Build-Tools, NDK, and your target SDK platforms. Configure environment variables for ANDROID_HOME, NDK_HOME, and JAVA_HOME. Add Android Rust targets: `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`.

Your development server needs configuration for mobile access. Update your Vite config (or equivalent) to handle the TAURI_DEV_HOST environment variable, enabling hot module replacement on mobile devices over your local network. Physical iOS devices require additional network permission setup through the `--force-ip-prompt` flag.

## Essential tools and build processes

Mobile development introduces platform-specific toolchains beyond standard Tauri requirements. **iOS builds require** Xcode for compilation, CocoaPods for dependency management, and Apple's code signing infrastructure. **Android builds need** Android Studio's build system, Gradle for dependency management, and the Android NDK for native code compilation.

Development commands mirror desktop with platform prefixes: `npm run tauri android dev` or `npm run tauri ios dev` launch your app on connected devices or emulators with hot reload support. For production builds, use `npm run tauri android build --aab` for Google Play or `npm run tauri ios build --export-method app-store-connect` for the App Store.

**Debugging leverages platform tools** - Chrome DevTools for Android via `chrome://inspect` and Safari Web Inspector for iOS. Both platforms support standard web debugging techniques, making the development experience familiar for web developers. Build outputs generate platform-specific packages: AAB/APK files for Android in `src-tauri/gen/android/app/build/outputs/` and IPA files for iOS in `src-tauri/gen/apple/build/`.

## Critical differences between desktop and mobile

The transition from desktop to mobile introduces several architectural constraints. **Plugin compatibility** presents the most significant challenge - many desktop-specific plugins like system tray, global shortcuts, and menu bars have no mobile equivalents. Mobile plugins require native language implementations (Swift/Kotlin) rather than pure Rust, increasing complexity for custom functionality.

**Platform limitations** affect development workflow and app capabilities. iOS development requires macOS, limiting team flexibility. Mobile sandboxing restricts file system access compared to desktop's broader permissions. Background processing faces strict limitations due to battery optimization requirements. The single-window constraint eliminates multi-window functionality common in desktop applications.

**Performance considerations** demand different optimization strategies. Mobile devices have limited memory, requiring careful resource management. Battery efficiency becomes crucial, affecting design decisions around background operations and network usage. Mobile WebViews may perform differently than desktop equivalents, particularly on older devices, requiring thorough cross-device testing.

## Adapting your app for mobile success

Successful mobile adaptation requires fundamental UI/UX changes. Touch interfaces demand larger interactive elements, typically 44-48px minimum touch targets. Navigation patterns shift from desktop paradigms (menu bars, hover states) to mobile conventions (bottom tabs, hamburger menus, gesture navigation). Responsive design becomes critical with the vast array of screen sizes and orientations.

**Mobile-specific considerations** include handling intermittent connectivity gracefully, implementing proper offline support, and managing app lifecycle events (background/foreground transitions). Safe area handling for iOS notches and Android navigation bars requires CSS adjustments. Platform-specific features like the Android back button need explicit handling in your application logic.

Code organization benefits from **conditional compilation and progressive enhancement**. Use Rust's `cfg` attributes to separate platform-specific code. Design features as plugins from the start for better mobile compatibility. Implement core functionality first, then layer platform-specific enhancements. This approach maintains a clean codebase while accommodating platform differences.

## Conclusion

Tauri v2's mobile support represents a significant evolution, enabling true cross-platform development from a single codebase. While the framework successfully extends desktop applications to mobile platforms, developers must understand and adapt to mobile-specific constraints, from plugin architecture differences to UI/UX considerations. The ability to use familiar web technologies for mobile development, combined with Rust's performance and security benefits, makes Tauri an compelling choice for teams with existing web expertise looking to expand to mobile platforms. Success requires embracing mobile-first design principles, understanding platform limitations, and leveraging Tauri's plugin ecosystem to bridge the gap between web technologies and native mobile capabilities.
