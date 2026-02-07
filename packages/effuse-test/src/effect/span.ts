import { FiberRef } from "effect"

import type { SpanId } from "../spec.ts"

export const CurrentSpanId = FiberRef.unsafeMake<SpanId | undefined>(undefined)

