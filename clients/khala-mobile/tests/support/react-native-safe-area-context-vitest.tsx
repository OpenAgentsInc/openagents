import React from "react"

const host = (name: string) => ({ children, ...props }: Record<string, any>) =>
  React.createElement(name, props, children)

export const SafeAreaProvider = host("SafeAreaProvider")
export const SafeAreaView = host("SafeAreaView")
export const initialWindowMetrics = {
  frame: { height: 844, width: 390, x: 0, y: 0 },
  insets: { bottom: 0, left: 0, right: 0, top: 0 },
}
export const useSafeAreaInsets = () => ({ bottom: 0, left: 0, right: 0, top: 0 })
