import { decodeTerminalHostProps, type TerminalHostProps } from "@effect-native/core"
import type { ReactElementLike, ReactNativeHostDriver } from "@effect-native/render-rn"

const clampGrid = (width: number, height: number): Readonly<{ cols: number; rows: number }> => ({
  cols: Math.max(20, Math.min(400, Math.floor(width / 7.2))),
  rows: Math.max(5, Math.min(200, Math.floor(height / 16))),
})

export const mobileTerminalHostDriver: ReactNativeHostDriver = {
  kind: "terminal",
  decodeProps: props => decodeTerminalHostProps(props),
  mount: (_props, context) => {
    let lastGrid = ""
    return {
      render: raw => {
        const props = raw as TerminalHostProps
        const create = context.dependencies.React.createElement
        const native = context.dependencies.ReactNative
        const ScrollView = (native as unknown as { readonly ScrollView: unknown }).ScrollView
        const output = (props.output ?? "").slice(-100_000)
        const emitData = (data: string) => {
          if (props.readOnly !== true && data.length > 0) context.emit({ type: "data", data })
        }
        return create(native.View, {
          testID: "oa-mobile-terminal-host",
          accessibilityLabel: "Interactive terminal",
          style: { flex: 1, minHeight: 280, borderRadius: 12, overflow: "hidden", backgroundColor: "#101114" },
          onLayout: (event: unknown) => {
            if (props.autoFit !== true || typeof event !== "object" || event === null) return
            const layout = (event as { nativeEvent?: { layout?: { width?: unknown; height?: unknown } } }).nativeEvent?.layout
            if (typeof layout?.width !== "number" || typeof layout.height !== "number") return
            const grid = clampGrid(layout.width, layout.height)
            const key = `${grid.cols}:${grid.rows}`
            if (key === lastGrid) return
            lastGrid = key
            context.emit({ type: "resize", ...grid })
          },
        },
        create(ScrollView, {
          key: "output",
          testID: "oa-mobile-terminal-output",
          style: { flex: 1 },
          contentContainerStyle: { paddingHorizontal: 10, paddingVertical: 10 },
          showsVerticalScrollIndicator: false,
        }, create(native.Text, {
          selectable: true,
          accessibilityLabel: "Terminal output",
          style: { color: "#f5f5f7", fontFamily: "Menlo", fontSize: 11, lineHeight: 16 },
        }, output === "" ? "$ " : output)),
        create(native.View, {
          key: "input-row",
          style: { flexDirection: "row", alignItems: "center", gap: 8, paddingHorizontal: 10, paddingVertical: 8, borderTopWidth: 1, borderTopColor: "#303238" },
        },
        create(native.TextInput, {
          key: "input",
          testID: "oa-mobile-terminal-input",
          accessibilityLabel: "Terminal input",
          editable: props.readOnly !== true,
          autoCapitalize: "none",
          autoCorrect: false,
          blurOnSubmit: false,
          returnKeyType: "send",
          placeholder: props.readOnly === true ? "Session is not running" : "type and press return",
          placeholderTextColor: "#777981",
          style: { flex: 1, color: "#f5f5f7", fontFamily: "Menlo", fontSize: 12, padding: 0 },
          onSubmitEditing: (event: unknown) => {
            const value = (event as { nativeEvent?: { text?: unknown } }).nativeEvent?.text
            if (typeof value === "string") emitData(`${value}\r`)
          },
        }),
        create(native.Pressable, {
          key: "interrupt",
          testID: "oa-mobile-terminal-ctrl-c",
          accessibilityRole: "button",
          accessibilityLabel: "Send Control C",
          disabled: props.readOnly === true,
          onPress: () => emitData("\u0003"),
          style: { paddingHorizontal: 10, paddingVertical: 7, borderRadius: 8, backgroundColor: "#303238" },
        }, create(native.Text, { style: { color: "#f5f5f7", fontSize: 11, fontWeight: "700" } }, "Ctrl-C"))),
        ) as ReactElementLike
      },
      unmount: () => undefined,
    }
  },
}
