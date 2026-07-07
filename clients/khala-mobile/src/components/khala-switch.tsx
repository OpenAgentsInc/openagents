import type { ComponentProps } from "react"

import { Toggle } from "./toggle"

export type KhalaSwitchProps = Omit<ComponentProps<typeof Toggle>, "variant">

export const KhalaSwitch = (props: KhalaSwitchProps) => <Toggle {...props} variant="switch" />
