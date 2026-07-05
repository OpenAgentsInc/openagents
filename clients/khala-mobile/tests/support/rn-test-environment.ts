import { plugin } from "bun"
import { transformSync } from "@babel/core"
import { readFileSync } from "node:fs"

/**
 * `bun test` preload (wired via `bunfig.toml`'s `[test] preload`) that makes
 * it possible to `TestRenderer.create(<RealProductionComponent />)` for a
 * React Native component in this package — see
 * `tests/chat-composer.test.tsx` for the first real use.
 *
 * `react-native` ships raw Flow + JSX source with no compiled `dist/`
 * (Metro compiles it at bundle time), and several of its leaf files
 * (`View`, `Text`, `TextInput`, `Pressable`, native platform constants)
 * touch the old Objective-C/Java bridge at import time, which has no
 * meaning without a real device/simulator host. This file makes both
 * problems disappear for `bun test` specifically:
 *
 * 1. A Bun `onLoad` plugin re-transforms every `react-native/**\/*.js` file
 *    with `@react-native/babel-preset` (Flow strip) on the fly.
 * 2. A small, explicit set of native-bridge-touching leaves get swapped for
 *    plain host-string-rendering stand-ins (`RN_LEAF_STUBS` below) — the
 *    same idea as React Native's own official Jest preset
 *    (`@react-native/jest-preset`), reimplemented by hand because that
 *    preset's mock files depend on Jest-only globals (`jest.fn`,
 *    `jest.requireActual`) that don't exist under `bun test`.
 * 3. `react-native/index.js`'s own public-API object (a lazy
 *    getter-per-export idiom) gets rewritten into real static `export`
 *    declarations, restricted to a small allowlist (`RN_EAGER_EXPORT_
 *    ALLOWLIST`) — see the extended comments below for why eagerly
 *    evaluating the other ~90 unrelated properties is unsafe, not just
 *    wasteful.
 *
 * This is a test-only shim. Production builds still go through the real
 * Expo/Metro pipeline, completely untouched by this file.
 *
 * Known limits (see `docs/khala-mobile/2026-07-05-qa-swarm-mobile-adaptation.md`
 * and `docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md` for the wider
 * honesty ledger this feeds): this proves REAL React state/render/effect
 * behavior for a mounted component tree — the actual thing a pure-logic unit
 * test cannot. It does NOT prove real native rendering, gesture/touch
 * physics, Skia drawing, or Reanimated worklet execution — those still need
 * a real device/simulator (Maestro/Detox), tracked separately as
 * `khala_mobile.platform.launched_app_interaction_smoke.v1`.
 */

// React Native's own source (and Metro's bundler output generally) assumes a
// global `__DEV__` boolean exists, gating dev-only warnings/invariants. Under
// plain `bun test` there is no bundler defining it, so any RN internal that
// reads `__DEV__` (react-native/index.js's deprecated-module warnings, RN's
// invariant/warning helpers, etc.) throws `ReferenceError: __DEV__ is not
// defined` before any test code runs. Match Metro's production-mode value.
;(globalThis as Record<string, unknown>).__DEV__ ??= false
// react-test-renderer (React 19) checks this to decide whether it's running
// inside a test harness that wraps updates in `act()`; without it, mounts
// emit "not wrapped in act()" warnings and — worse, empirically — silently
// unmount instead of rendering.
;(globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT ??= true

/**
 * React Native ships raw Flow + JSX source as its published npm package —
 * there is no compiled `dist/`, because Metro normally compiles it at bundle
 * time. `bun test` has no Metro step, so importing `react-native` directly
 * throws a Flow parse error ("Unexpected typeof") before any test code runs.
 *
 * This preload registers a Bun loader plugin that intercepts every `.js` file
 * inside `react-native`'s own package folder and re-transforms it with the
 * SAME `@react-native/babel-preset` Metro itself would use (strips Flow
 * types, compiles JSX). This is a test-only shim — production builds still go
 * through the real Expo/Metro pipeline untouched.
 *
 * One deliberate deviation from Metro's own config:
 * `disableImportExportTransform: true`. Metro normally lets Babel lower real
 * `import`/`export` syntax to CommonJS (`@babel/plugin-transform-modules-
 * commonjs`) because Metro's OWN bundler runtime expects CJS-shaped modules.
 * We are not Metro — content returned from a Bun plugin's `onLoad` hook is
 * only recognized for named exports (by `import {X}` AND by `require()`)
 * when it uses genuine `export` syntax; CJS `module.exports = ...` /
 * `exports.X = ...` output from a plugin is silently exposed as an EMPTY
 * module (verified empirically: neither shape works once routed through
 * `onLoad`, even though both work fine for files Bun reads directly off
 * disk). So we keep real `import`/`export` statements exactly as React
 * Native wrote them and let Bun's native ESM support handle them directly —
 * simpler and more correct than fighting Bun's CJS/ESM interop from the
 * plugin side.
 */

/** The only `react-native/index.js` top-level names this test harness ever
 * actually mounts. Every other property on that object becomes a plain
 * `undefined` export (see the allowlist check below for why eagerly
 * `require()`-ing the other ~90 properties is actively unsafe, not just
 * unnecessary). Extend this list only when a new real (non-mocked)
 * `react-native` named import is needed by a component under test. */
const RN_EAGER_EXPORT_ALLOWLIST = new Set(["Platform", "Pressable", "Text", "TextInput", "View"])

/** Rewrites the one non-standard idiom in this package that ISN'T already
 * real ESM: `module.exports = ({ get X() { ... }, Y(...) {...} }: SomeType)`,
 * the "lazy public API object" used by `react-native/index.js` alone. This
 * converts it into real static `export const X = ...` declarations (see the
 * module doc comment above for why real `export` syntax is required here).
 */
const definePropertyExportsPlugin = ({ types: t }: any) => ({
  visitor: {
    ExpressionStatement(path: any) {
      const expr = path.node.expression
      if (
        !t.isAssignmentExpression(expr, { operator: "=" }) ||
        !t.isMemberExpression(expr.left) ||
        !t.isIdentifier(expr.left.object, { name: "module" }) ||
        !t.isIdentifier(expr.left.property, { name: "exports" })
      ) {
        return
      }
      // Flow annotates this idiom as `module.exports = ({...}: SomeType)` —
      // a `TypeCastExpression` wrapping the real object literal. Unwrap it
      // (dropping the now-pointless runtime type cast) before inspecting.
      let rhs = expr.right
      if (t.isTypeCastExpression(rhs)) rhs = rhs.expression
      if (!t.isObjectExpression(rhs)) return
      const objectExpression = rhs
      const hasAccessor = objectExpression.properties.some(
        (prop: any) => t.isObjectMethod(prop) && (prop.kind === "get" || prop.kind === "set")
      )
      if (!hasAccessor) return

      const wrapInTrySafe = (bodyStatements: Array<any>) =>
        t.callExpression(
          t.arrowFunctionExpression(
            [],
            t.blockStatement([
              t.tryStatement(
                t.blockStatement(bodyStatements),
                t.catchClause(t.identifier("__rnFlowStripError"), t.blockStatement([t.returnStatement(t.identifier("undefined"))]))
              )
            ])
          ),
          []
        )

      const statements: Array<any> = []
      for (const prop of objectExpression.properties) {
        if (t.isObjectMethod(prop) && (prop.kind === "get" || prop.kind === "set")) {
          if (prop.kind !== "get") continue // no setters appear in this idiom in practice
          const exportName = t.isIdentifier(prop.key) ? prop.key.name : null
          if (exportName === null) continue // computed keys never appear in this idiom
          // Only the small set of primitives this test harness actually
          // mounts are worth the (still try/catch-guarded) eager `require()`.
          // Evaluating all ~99 properties eagerly was tried first and
          // rejected: several of react-native's OWN internal submodules
          // circularly `require('react-native')` back while `index.js`
          // itself is still mid-evaluation, and eagerly forcing ALL of them
          // in one synchronous pass tripped Bun's "Requested module is
          // already fetched" reentrant-require guard — which then corrupted
          // unrelated sibling properties in the SAME object (e.g. `View`
          // resolving to `undefined` purely because `Vibration`'s eager
          // eval blew up earlier in the same statement list, with no real
          // relationship between the two). Every un-allowlisted property
          // below is exported as a plain `undefined` — never evaluated,
          // zero risk — since this harness has no use for it.
          if (!RN_EAGER_EXPORT_ALLOWLIST.has(exportName)) {
            statements.push(
              t.exportNamedDeclaration(
                t.variableDeclaration("const", [t.variableDeclarator(t.identifier(exportName), t.identifier("undefined"))]),
                []
              )
            )
            continue
          }
          statements.push(
            t.exportNamedDeclaration(
              t.variableDeclaration("const", [
                t.variableDeclarator(t.identifier(exportName), wrapInTrySafe(prop.body.body))
              ]),
              []
            )
          )
        } else if (t.isObjectMethod(prop)) {
          // Plain method shorthand, e.g. `unstable_batchedUpdates(fn) {...}`
          // — the function ITSELF is the intended export value, not its
          // return value, so no try/catch call-wrapping here.
          const exportName = t.isIdentifier(prop.key) ? prop.key.name : null
          if (exportName === null) continue
          const fn = t.functionExpression(null, prop.params, prop.body, prop.generator, prop.async)
          statements.push(
            t.exportNamedDeclaration(t.variableDeclaration("const", [t.variableDeclarator(t.identifier(exportName), fn)]), [])
          )
        } else if (t.isObjectProperty(prop)) {
          const exportName = t.isIdentifier(prop.key) ? prop.key.name : null
          if (exportName === null) continue
          const valueExpr = RN_EAGER_EXPORT_ALLOWLIST.has(exportName)
            ? wrapInTrySafe([t.returnStatement(prop.value)])
            : t.identifier("undefined")
          statements.push(
            t.exportNamedDeclaration(t.variableDeclaration("const", [t.variableDeclarator(t.identifier(exportName), valueExpr)]), [])
          )
        }
      }
      path.replaceWithMultiple(statements)
    }
  }
})

/**
 * A small set of react-native internals that a real device/simulator would
 * satisfy (native view registration, the old bridge, platform constants) but
 * that have no meaningful behavior at all under plain `bun test` — there is
 * no native host, so `View`/`Text`/`TextInput`'s REAL implementations throw
 * ("__fbBatchedBridgeConfig is not set, cannot invoke native modules") the
 * moment they're evaluated. This mirrors what React Native's own official
 * Jest preset does (`@react-native/jest-preset`'s `jest/mocks/{View,Text,
 * TextInput}.js` + `jest/setup.js`'s `mock(...)` table) — swap ONLY the
 * native-bridge-touching leaves for plain host-string-rendering stand-ins.
 * (`Pressable` turned out to need the same treatment — see below.) We
 * hand-roll simple replacements instead of reusing the jest-preset mock
 * files verbatim because those depend on Jest's `jest.fn()`/
 * `jest.requireActual()` globals, which don't exist under `bun test`.
 */
const RN_LEAF_STUBS: ReadonlyArray<{ readonly test: RegExp; readonly contents: string }> = [
  {
    contents: `
      import * as React from "react"
      const View = React.forwardRef((props, ref) => React.createElement("View", { ...props, ref }))
      View.displayName = "View"
      export default View
    `,
    test: /\/Libraries\/Components\/View\/View\.js$/
  },
  {
    // Real `Pressable.js` composes `Pressability`/`usePressability` (touch
    // responder plumbing) plus `useAndroidRippleForView`, which reach into
    // the same dead native bridge transitively (confirmed empirically: it
    // throws the identical `__fbBatchedBridgeConfig is not set` invariant).
    // `chat-composer.tsx` only needs `onPress`/`disabled`/`children` to
    // behave normally for a mount test — this stand-in forwards exactly
    // those, dropping ripple/haptics/gesture-responder behavior that has no
    // meaningful equivalent without a real touch host anyway.
    contents: `
      import * as React from "react"
      const Pressable = React.forwardRef((props, ref) => {
        const { children, disabled, onPress, onPressIn, onPressOut, ...rest } = props
        const pressableProps = {
          ...rest,
          ref,
          onPress: disabled ? undefined : onPress,
          onPressIn: disabled ? undefined : onPressIn,
          onPressOut: disabled ? undefined : onPressOut
        }
        const resolvedChildren = typeof children === "function" ? children({ pressed: false }) : children
        return React.createElement("Pressable", pressableProps, resolvedChildren)
      })
      Pressable.displayName = "Pressable"
      export default Pressable
    `,
    test: /\/Libraries\/Components\/Pressable\/Pressable\.js$/
  },
  {
    contents: `
      import * as React from "react"
      const Text = React.forwardRef((props, ref) => React.createElement("Text", { ...props, ref }))
      Text.displayName = "Text"
      export default Text
    `,
    test: /\/Libraries\/Text\/Text\.js$/
  },
  {
    contents: `
      import * as React from "react"
      const TextInput = React.forwardRef((props, ref) => React.createElement("TextInput", { ...props, ref }))
      TextInput.displayName = "TextInput"
      TextInput.State = { currentlyFocusedInput: () => null }
      export default TextInput
    `,
    test: /\/Libraries\/Components\/TextInput\/TextInput\.js$/
  },
  {
    // Real `Platform.ios.js`/`Platform.android.js` read device constants
    // (`NativePlatformConstantsIOS`, a TurboModule spec) through the same
    // dead native bridge. `chat-composer.tsx` only needs `Platform.select`
    // at module scope (`chatComposerKeyboardVerticalOffset`), so a static
    // "ios" stand-in is enough — matching this repo's iOS-first posture.
    contents: `
      const Platform = {
        OS: "ios",
        Version: "18.0",
        isPad: false,
        isTV: false,
        isTesting: true,
        select: (spec) => ("ios" in spec ? spec.ios : spec.native ?? spec.default)
      }
      export default Platform
    `,
    // Matches `Platform.ios.js`/`Platform.android.js` AND the bare
    // `Platform.js` back-compat shim (`import Platform from './Platform'` —
    // a self-referencing file that only makes sense under a bundler with
    // platform-extension resolution; under Bun's plain resolver it recurses
    // into itself and throws `Cannot access 'Platform' before
    // initialization`). All three names resolve here identically.
    test: /\/Libraries\/Utilities\/Platform(\.ios|\.android)?\.js$/
  }
]

plugin({
  name: "rn-flow-strip",
  setup(build) {
    for (const stub of RN_LEAF_STUBS) {
      build.onLoad({ filter: stub.test }, () => ({ contents: stub.contents, loader: "js" }))
    }

    // IMPORTANT: this must stay a SYNCHRONOUS onLoad callback. Bun treats any
    // module resolved through an async `onLoad` hook as an "async module" in
    // its graph, and refuses plain `require()` of async modules ("use await
    // import() instead") — but react-native's own internals `require()` each
    // other pervasively and synchronously. A sync callback keeps every
    // Flow-stripped file an ordinary synchronous module so those internal
    // `require()` calls keep working.
    build.onLoad({ filter: /node_modules\/react-native\/.*\.js$/ }, args => {
      const source = readFileSync(args.path, "utf8")
      const result = transformSync(source, {
        filename: args.path,
        presets: [[require.resolve("@react-native/babel-preset"), { disableImportExportTransform: true }]],
        plugins: [definePropertyExportsPlugin],
        babelrc: false,
        configFile: false,
        sourceType: "unambiguous"
      })
      return { contents: result?.code ?? source, loader: "js" }
    })
  }
})
