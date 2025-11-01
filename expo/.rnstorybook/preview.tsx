import React from 'react'
import type { Preview } from '@storybook/react-native'
import { View, ActivityIndicator, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { applyTypographyGlobals, useTypographySetup, Typography } from '@/constants/typography'

const preview: Preview = {
  parameters: {},
  decorators: [
    (Story) => {
      const fontsLoaded = useTypographySetup()
      React.useEffect(() => {
        if (fontsLoaded) applyTypographyGlobals()
      }, [fontsLoaded])
      if (!fontsLoaded) {
        return (
          <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
            <ActivityIndicator size="large" color={Colors.foreground} />
            <Text style={{ marginTop: 10, color: Colors.secondary, fontFamily: Typography.bold, fontSize: 16 }}>Loading fonts…</Text>
          </View>
        )
      }
      return <Story />
    },
  ],
}

export default preview
