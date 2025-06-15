import { describe, expect, it } from "vitest"
import { render, screen } from "@testing-library/react"
import { Button } from "../src/web/components/button"

describe("Button", () => {
  it("renders with text", () => {
    render(<Button>Click me</Button>)
    expect(screen.getByRole("button")).toHaveTextContent("Click me")
  })

  it("applies variant classes", () => {
    render(<Button variant="destructive">Delete</Button>)
    const button = screen.getByRole("button")
    expect(button.className).toContain("destructive")
  })

  it("applies size classes", () => {
    render(<Button size="sm">Small</Button>)
    const button = screen.getByRole("button")
    expect(button.className).toContain("sm")
  })

  it("handles click events", () => {
    let clicked = false
    render(<Button onClick={() => clicked = true}>Click</Button>)
    screen.getByRole("button").click()
    expect(clicked).toBe(true)
  })

  it("can be disabled", () => {
    render(<Button disabled>Disabled</Button>)
    expect(screen.getByRole("button")).toBeDisabled()
  })
})