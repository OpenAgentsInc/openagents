# Expo UI SwiftUI Audit

> **Honest-scope header.** This is a source-reading audit of Expo's
> `@expo/ui/swift-ui` implementation, written for OpenAgents mobile planning.
> It is not a recommendation to adopt Expo for the current Khala native iOS app.
> The current Khala mobile policy remains native SwiftUI/local Xcode unless the
> owner explicitly changes that direction.

- **Date:** 2026-07-09
- **Reference docs:** <https://docs.expo.dev/versions/latest/sdk/ui/swift-ui/>
- **Guide docs:** <https://docs.expo.dev/guides/expo-ui-swift-ui/>
- **Extension docs:** <https://docs.expo.dev/guides/expo-ui-swift-ui/extending/>
- **Source inspected:** `/Users/christopherdavid/work/projects/repos/expo`
- **Package:** `packages/expo-ui` (`@expo/ui`)
- **Version caveat:** the live docs recommend `~57.0.4`; the local Expo
  checkout inspected here has `packages/expo-ui/package.json` at `56.0.14`.

## Short version

Expo UI is a React Native bridge over native declarative UI systems. Its SwiftUI
entrypoint is `@expo/ui/swift-ui`; the npm package is `@expo/ui`. JavaScript
imports typed React components such as `Host`, `Button`, `Form`, `HStack`, and
`Text`. Those components call Expo's native module loader (`requireNativeView`
or `requireNativeModule`) with the native module name `ExpoUI`. On iOS, the
`ExpoUI` module registers SwiftUI-backed views and exposes them to React Native
Fabric through Expo Modules Core.

The central mental model:

1. React Native owns the outer tree and Yoga layout.
2. `Host` crosses into SwiftUI by creating a UIKit-backed Expo view that embeds
   a `UIHostingController`.
3. SwiftUI children under `Host` are represented as virtual SwiftUI nodes, not
   normal UIKit child views.
4. Props flow from TypeScript objects into Swift `@Field` properties on
   `ExpoSwiftUI.ViewProps` subclasses.
5. Events flow back through Expo `EventDispatcher`s, including a special global
   event path for SwiftUI modifiers.
6. Modifiers are serialized TypeScript records with a `$type` key, then applied
   by a native `ViewModifierRegistry`.

The result is closer to "write SwiftUI from JSX" than to "style React Native to
look like iOS." Expo is deliberately trying to map the native SwiftUI API shape
into TypeScript components.

## Public API from the docs

The official SwiftUI SDK page says the package is installed with:

```sh
npx expo install @expo/ui
```

The public import path for SwiftUI primitives is:

```tsx
import { Host, Button } from '@expo/ui/swift-ui';
```

The important usage rule is that SwiftUI components must be placed inside
`Host`. Expo describes `Host` as the container for SwiftUI views. The guide
adds the implementation detail: `Host` uses `UIHostingController` to render
SwiftUI views inside UIKit.

The guide also spells out the layout boundary: flexbox can style `Host` itself,
but inside `Host` the layout system is SwiftUI. That means authors use
components such as `HStack`, `VStack`, `Form`, `List`, and SwiftUI modifiers
rather than Yoga/flexbox for the native subtree.

The docs currently mark the SwiftUI surface as iOS/tvOS in the SDK reference,
with the guide also mentioning macOS support for the guide page. The package
source has Apple and Android native module registrations, but this audit is
only about the SwiftUI side.

## Package shape

`packages/expo-ui/package.json` defines the npm package and subpath exports:

```json
{
  "name": "@expo/ui",
  "exports": {
    "./swift-ui": {
      "types": "./build/swift-ui/index.d.ts",
      "default": "./src/swift-ui/index.tsx"
    },
    "./swift-ui/modifiers": {
      "types": "./build/swift-ui/modifiers/index.d.ts",
      "default": "./src/swift-ui/modifiers/index.ts"
    }
  }
}
```

The same package also exports Jetpack Compose and universal/community entry
points. Its `expo-module.config.json` names the native module as `expo-ui`,
lists core features `swiftui` and `compose`, and registers the Apple module
class `ExpoUIModule`.

The package layout is:

| Area | Path | Purpose |
| --- | --- | --- |
| TypeScript SwiftUI API | `packages/expo-ui/src/swift-ui/` | React components and exported types |
| TypeScript modifiers | `packages/expo-ui/src/swift-ui/modifiers/` | JS factories that serialize SwiftUI modifiers |
| SwiftUI native views | `packages/expo-ui/ios/` | Swift implementations of SwiftUI components |
| Bridge primitives | `packages/expo-modules-core/ios/Core/Views/SwiftUI/` | Hosting, virtual views, props, child mounting, definition builders |
| Native registration | `packages/expo-ui/ios/ExpoUIModule.swift` | Registers the `ExpoUI` module, views, functions, shared state |

## TypeScript wrapper pattern

Each exported TypeScript component is a thin wrapper around a native view. A
typical example is `Button`:

- `src/swift-ui/Button/index.tsx` defines `ButtonProps`.
- It calls `requireNativeView('ExpoUI', 'Button')`.
- It maps React props into native props and event names.
- It attaches modifier event listeners when `modifiers` are present.

Simplified:

```tsx
const ButtonNativeView = requireNativeView('ExpoUI', 'Button');

export function Button({ onPress, modifiers, ...restProps }: ButtonProps) {
  return (
    <ButtonNativeView
      {...restProps}
      modifiers={modifiers}
      {...(modifiers ? createViewModifierEventListener(modifiers) : undefined)}
      onButtonPress={onPress}
    />
  );
}
```

The TypeScript props intentionally mirror SwiftUI terms: `role`, `label`,
`systemImage`, `HStack`, `VStack`, `Form`, `Section`, `Spacer`, and so on. This
is not a custom design system vocabulary; it is a typed JavaScript facade over
native SwiftUI concepts.

`src/swift-ui/index.tsx` is mostly a barrel export. It exposes components,
`useNativeState`, `withAnimation`, and common types such as
`CommonViewModifierProps`.

## Native registration

`packages/expo-ui/ios/ExpoUIModule.swift` defines:

```swift
public final class ExpoUIModule: Module {
  public func definition() -> ModuleDefinition {
    Name("ExpoUI")
    ...
    View(HostView.self)
    View(TextView.self)
    ...
    ExpoUIView(Button.self)
    ExpoUIView(FormView.self)
    ExpoUIView(HStackView.self)
    ExpoUIView(VStackView.self)
    ...
  }
}
```

There are three important categories in this registration:

- `View(HostView.self)` and `View(TextView.self)` are registered directly
  because they apply common modifiers internally or have special behavior.
- `ExpoUIView(...)` wraps most SwiftUI views in `UIBaseView` so common props and
  modifiers are applied consistently.
- Some views register async functions, such as `TextFieldView` focus/blur and
  `ShareLinkView.setItem`.

The module also registers:

- `ObservableState`, a shared object that lets SwiftUI observe native state
  controlled from JS.
- `completeRefresh`, used by the `refreshable` modifier to finish pull-to-refresh.
- `withAnimation`, which runs SwiftUI animation work on the main queue.
- Namespace cleanup on module destroy.

## How SwiftUI views are modeled

Expo Modules Core defines `ExpoSwiftUI.View` as a Swift protocol:

```swift
public protocol ExpoSwiftUIView<Props>: SwiftUI.View, AnyArgument, ExpoSwiftUI.AnyChild {
  associatedtype Props: ExpoSwiftUI.ViewProps
  var props: Props { get }
  init(props: Props)
}
```

Every native SwiftUI component owns a props object, usually with fields
declared by Expo's `@Field` property wrapper. For example, `ButtonProps` is:

```swift
open class ButtonProps: UIBaseViewProps, Observable {
  @Field public var label: String?
  @Field public var systemImage: String?
  @Field public var role: ButtonRole?
  var onButtonPress = EventDispatcher()
}
```

The Swift `Button` view then reads `props` and returns real SwiftUI:

```swift
public struct Button: ExpoSwiftUI.View {
  @ObservedObject public var props: ButtonProps

  public var body: some View {
    SwiftUI.Button(props.label ?? "", role: props.role?.toNativeRole()) {
      props.onButtonPress()
    }
  }
}
```

That is the core bridge: TypeScript describes a React component, Expo converts
its props into a Swift observable props object, and SwiftUI re-renders when that
object changes.

## Host and the UIKit boundary

`Host` is the boundary between React Native UIKit and SwiftUI.

On the TypeScript side, `src/swift-ui/Host/index.tsx` calls:

```tsx
const HostNativeView = requireNativeView('ExpoUI', 'HostView');
```

Its props include:

- `matchContents`
- `useViewportSizeMeasurement`
- `onLayoutContent`
- `colorScheme`
- `seedColor`
- `layoutDirection`
- `ignoreSafeArea`
- `children`
- normal React Native `style` on the host view itself

On the Swift side, `HostView` conforms to
`ExpoSwiftUI.View, ExpoSwiftUI.WithHostingView`. That marker matters. In
`SwiftUIViewDefinition.createView`, views that conform to `WithHostingView` are
created as `ExpoSwiftUI.HostingView`, which owns a `UIHostingController`.

`HostingView`:

- Subclasses `ExpoView`, so React Native can place it in the UIKit view tree.
- Creates `UIHostingController(rootView: AnyView(contentView))`.
- Adds the hosting controller as a child of the nearest React view controller.
- Sets the hosting controller view to the Expo view's bounds.
- Receives Fabric child mounts and turns child component views into SwiftUI
  child records.
- Updates props with `props.updateRawProps(rawProps, appContext:)`.

`HostView` itself renders `Children()` inside a `ZStackLayout` or
`ViewportSizeMeasurementLayout`, applies environment values such as color
scheme and layout direction, applies modifiers, then emits content layout
events. When `matchContents` is active, it can push measured SwiftUI size back
into the React Native shadow node through `shadowNodeProxy.setStyleSize`.

This is why the docs say flexbox stops at `Host`: React Native sizes and places
the host view, then SwiftUI lays out the subtree inside it.

## Virtual SwiftUI child views

Most SwiftUI components under a `Host` are not standalone UIKit-hosted views.
They are exported as virtual SwiftUI views.

`SwiftUIViewDefinition.createView` chooses between:

- `HostingView` when the SwiftUI type conforms to `WithHostingView`.
- `SwiftUIVirtualViewDev` in dev builds.
- `SwiftUIVirtualView` in production builds.

`SwiftUIVirtualView` is an Objective-C/Swift bridge object that acts as a fake
view for React Native mounting. It stores:

- the `props` object,
- the real SwiftUI `contentView`,
- the view definition,
- event dispatchers,
- a shadow node proxy for size/style updates.

When Fabric mounts a child, Expo either keeps it as an `ExpoSwiftUI.View` child
or wraps a UIKit view as `UIViewHost`. SwiftUI components call `Children()` to
render the current `props.children` array as SwiftUI views. There is also
`UnwrappedChildren()` for cases where nested hosting views need to be stripped
so SwiftUI can consume the inner view directly.

This allows JSX such as:

```tsx
<Host>
  <Button>
    <HStack>
      <Text>Save</Text>
    </HStack>
  </Button>
</Host>
```

to become a SwiftUI tree rather than a stack of UIKit wrappers for every leaf.

## Modifiers

Modifiers are a second bridge layer.

On the TypeScript side, `createModifier(type, params)` returns a plain object:

```ts
{ $type: 'padding', all: 16 }
```

Components accept `modifiers?: ViewModifier[]` through
`CommonViewModifierProps`. Event-like modifiers use
`createModifierWithEventListener`; `createViewModifierEventListener` scans the
modifier array and attaches an `onGlobalEvent` handler to the native view.

On the Swift side:

- `UIBaseViewProps` includes `@Field var modifiers: ModifierArray?`.
- `UIBaseView` calls `.applyModifiers(...)`.
- `View+ModifierArray.swift` reduces the modifier array in order.
- `ViewModifierRegistry.shared.applyModifier(...)` maps the `$type` string to
  a native `ViewModifier`.

The modifier registry has native Swift records for SwiftUI concepts such as
padding, frame, corner radius, shadow, opacity, tint, refreshable, gestures,
foreground style, list style, picker style, dynamic type, symbol effects, and
presentation behavior.

The extension guide shows the intended third-party path:

1. Create a Swift `ViewModifier` that also conforms to `Record`.
2. Register it in `ViewModifierRegistry` on module create.
3. Export a TypeScript helper that calls `createModifier('customType', params)`.

That pattern means the JavaScript API remains typed, but native Swift owns the
actual visual and interaction behavior.

## Native state and worklets

The package also includes a small shared-state bridge for native-controlled
SwiftUI state.

`useNativeState(initialValue)` creates an `ExpoUI.ObservableState` shared object
from JavaScript. On iOS, `ObservableState` is a `SharedObject` and
`ObservableObject` with a `@Published var value`. SwiftUI views can bind to it
with `state.binding(fallback)`.

The JavaScript object exposes:

- `state.value`
- `state.get()`
- `state.set(value)`
- `state.onChange`

`onChange` requires `react-native-worklets` because the callback is meant to run
on the UI runtime when native state changes. JS-thread writes are scheduled to
the UI thread; worklet writes can be synchronous.

This is useful for controls where native interaction should update SwiftUI
state without waiting for a normal React render loop.

## Custom component extension path

The extension docs match the source architecture. A custom SwiftUI component:

1. Adds `ExpoUI` as a pod dependency.
2. Defines props by subclassing `UIBaseViewProps`.
3. Defines a Swift view conforming to `ExpoSwiftUI.View`.
4. Registers it with `ExpoUIView(MyCustomView.self)`.
5. Exposes a TypeScript wrapper using `requireNativeView('MyUi', 'MyCustomView')`.

That is the same path `@expo/ui` itself uses. The value of subclassing
`UIBaseViewProps` is automatic support for the common `modifiers` prop and test
identifier handling.

## Constraints and implications for OpenAgents mobile

This is a real SwiftUI bridge, not a superficial theme layer. The native render
path ends in `UIHostingController`, SwiftUI `View` structs, SwiftUI layout, and
SwiftUI modifiers.

Adopting it would still bring React Native and Expo Modules into the app:

- The app would ship a React Native runtime.
- TypeScript would own UI declaration and prop/event typing.
- Swift would own the native component implementations and modifier registry.
- The layout boundary would be explicit: Yoga outside `Host`, SwiftUI inside it.
- Native extension work would require both Swift module code and TypeScript
  wrapper code.

For the current Khala iOS app, that is a different product architecture from
the native SwiftUI-only path documented in this repo. Expo UI could be useful as
reference material for "how to expose SwiftUI primitives to JS," especially for
a future React Native surface, but it is not a drop-in implementation technique
for a pure SwiftUI app.

## Files worth re-reading

In the local Expo checkout:

- `packages/expo-ui/package.json`
- `packages/expo-ui/expo-module.config.json`
- `packages/expo-ui/src/swift-ui/Host/index.tsx`
- `packages/expo-ui/src/swift-ui/Button/index.tsx`
- `packages/expo-ui/src/swift-ui/modifiers/createModifier.ts`
- `packages/expo-ui/src/swift-ui/modifiers/utils.ts`
- `packages/expo-ui/ios/ExpoUIModule.swift`
- `packages/expo-ui/ios/UIBaseView.swift`
- `packages/expo-ui/ios/UIBaseViewProps.swift`
- `packages/expo-ui/ios/HostView.swift`
- `packages/expo-ui/ios/Button/Button.swift`
- `packages/expo-ui/ios/Button/ButtonProps.swift`
- `packages/expo-ui/ios/Modifiers/View+ModifierArray.swift`
- `packages/expo-ui/ios/Modifiers/ViewModifierRegistry.swift`
- `packages/expo-ui/ios/State/ObservableState.swift`
- `packages/expo-modules-core/ios/Core/Views/SwiftUI/SwiftUIViewDefinition.swift`
- `packages/expo-modules-core/ios/Core/Views/SwiftUI/SwiftUIHostingView.swift`
- `packages/expo-modules-core/ios/Core/Views/SwiftUI/SwiftUIVirtualView.swift`
