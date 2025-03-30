import * as AspectRatioPrimitive from "@radix-ui/react-aspect-ratio"
import React from 'react'
import { react19 } from '@openagents/core'

// Make AspectRatioPrimitive compatible with React 19
const CompatRoot = react19.compat(AspectRatioPrimitive.Root)

function AspectRatio({
  ...props
}: React.ComponentProps<typeof AspectRatioPrimitive.Root>) {
  return <CompatRoot data-slot="aspect-ratio" {...props} />
}

export { AspectRatio }
