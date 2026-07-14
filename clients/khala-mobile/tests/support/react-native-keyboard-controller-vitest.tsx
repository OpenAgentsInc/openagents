import React from "react"

const host = (name: string) => ({ children, ...props }: Record<string, any>) =>
  React.createElement(name, props, children)

export const KeyboardAwareScrollView = host("KeyboardAwareScrollView")
export const KeyboardProvider = host("KeyboardProvider")
