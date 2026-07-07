import type { ComponentProps } from "react"

import { Toggle } from "./toggle"

export type KhalaRadioProps = Omit<ComponentProps<typeof Toggle>, "variant">

export const KhalaRadio = (props: KhalaRadioProps) => <Toggle {...props} variant="radio" />
