import type { ComponentProps } from "react"

import { Toggle } from "./toggle"

export type KhalaCheckboxProps = Omit<ComponentProps<typeof Toggle>, "variant">

export const KhalaCheckbox = (props: KhalaCheckboxProps) => <Toggle {...props} variant="checkbox" />
