import React from 'react'
import { Platform } from 'react-native'

export default function StorybookRoot() {
  if (Platform.OS === 'web') return null
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const StorybookUIRoot = require('../.rnstorybook').default as React.ComponentType
  return <StorybookUIRoot />
}

