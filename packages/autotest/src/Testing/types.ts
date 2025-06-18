export interface InteractionStep {
  readonly action: "click" | "fill" | "select" | "wait" | "navigate"
  readonly selector?: string
  readonly value?: string
  readonly timeout?: number
}

export interface WaitOptions {
  readonly timeout?: number
  readonly interval?: number
}

export interface NavigationOptions {
  readonly waitUntil?: "load" | "domcontentloaded" | "networkidle0" | "networkidle2"
  readonly timeout?: number
}
