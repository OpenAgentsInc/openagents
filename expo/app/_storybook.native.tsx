import React from 'react'

// Native platforms: load the on-device Storybook UI
// eslint-disable-next-line @typescript-eslint/no-var-requires
const StorybookUIRoot = require('../.rnstorybook').default as React.ComponentType

export default function StorybookRoot() {
  return <StorybookUIRoot />
}

