import React from 'react'
import { ConvexProvider, ConvexReactClient } from 'convex/react'

function defaultUrl() {
  return 'http://127.0.0.1:7788'
}

const client = new ConvexReactClient(defaultUrl(), { verbose: false })

export function ConvexProviderLocal({ children }: { children: React.ReactNode }) {
  return <ConvexProvider client={client}>{children}</ConvexProvider>
}

