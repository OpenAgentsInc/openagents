import { describe, expect, mock, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"

mock.module("../src/components/touchable-feedback", () => ({
  TouchableFeedback: ({
    accessibilityLabel,
    accessibilityRole,
    accessibilityState,
    children,
    disabled,
    onPress,
    testID,
  }: {
    accessibilityLabel?: string
    accessibilityRole?: "button" | "link" | "none"
    accessibilityState?: Record<string, unknown>
    children?: React.ReactNode
    disabled?: boolean
    onPress?: () => void
    testID?: string
  }) =>
    React.createElement(
      "TouchableFeedback",
      {
        accessibilityLabel,
        accessibilityRole,
        accessibilityState: { ...accessibilityState, disabled },
        onPress: disabled ? undefined : onPress,
        testID,
      },
      children,
    ),
}))

const [{ KhalaEmptyState }, { KhalaListItem }, { KhalaTextField }] =
  await Promise.all([
    import("../src/components/khala-empty-state"),
    import("../src/components/khala-list-item"),
    import("../src/components/khala-text-field"),
  ])

describe("Khala ordinary UI primitives", () => {
  const mount = async (element: React.ReactElement) => {
    let renderer: ReturnType<typeof createTestRenderer> | undefined
    await act(async () => {
      renderer = createTestRenderer(element)
      await Promise.resolve()
    })
    return renderer!
  }

  test("KhalaTextField derives label, disabled, and invalid accessibility state", async () => {
    const renderer = await mount(
      React.createElement(KhalaTextField, {
        disabled: true,
        errorText: "Token failed validation",
        label: "OpenAgents token",
        onChangeText: () => undefined,
        value: "",
      }),
    )

    const inputs = renderer.root.findAllByType("TextInput" as unknown as React.ComponentType)
    expect(inputs).toHaveLength(1)
    expect(inputs[0]!.props.accessibilityLabel).toBe("OpenAgents token")
    expect(inputs[0]!.props.accessibilityState).toMatchObject({
      disabled: true,
    })
    expect(inputs[0]!.props["aria-invalid"]).toBe(true)
    expect(inputs[0]!.props.editable).toBe(false)
  })

  test("KhalaListItem exposes an accessible button row when interactive", async () => {
    const renderer = await mount(
      React.createElement(KhalaListItem, {
        accessibilityLabel: "Open thread",
        detail: "2 messages",
        disabled: true,
        meta: "1h",
        onPress: () => undefined,
        title: "Mobile-started Codex session test",
      }),
    )

    const rows = renderer.root.findAllByType("TouchableFeedback" as unknown as React.ComponentType)
    expect(rows).toHaveLength(1)
    expect(rows[0]!.props.accessibilityRole).toBe("button")
    expect(rows[0]!.props.accessibilityLabel).toBe("Open thread")
    expect(rows[0]!.props.accessibilityState).toMatchObject({ disabled: true })
    expect(rows[0]!.props.onPress).toBeUndefined()
  })

  test("KhalaEmptyState marks loading fallback as progress", async () => {
    const renderer = await mount(
      React.createElement(KhalaEmptyState, {
        detail: "Looking for a signed-in Mac on your Tailnet.",
        loading: true,
        title: "Loading threads",
      }),
    )

    const states = renderer.root.findAllByType("View" as unknown as React.ComponentType)
    expect(states[0]!.props.accessibilityRole).toBe("progressbar")
  })
})
