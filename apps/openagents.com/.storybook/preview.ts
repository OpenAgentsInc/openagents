import type { Preview } from '@storybook/nextjs'
import React from 'react'
import { AnimatorGeneralProvider, BleepsProvider } from '@arwes/react'
import '../app/globals.css'

// Import Arwes configuration
import { animatorGeneralSettings } from '../config/animator'
import { bleepsSettings } from '../config/bleeps'

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
       color: /(background|color)$/i,
       date: /Date$/i,
      },
    },
    backgrounds: {
      default: 'dark',
      values: [
        {
          name: 'dark',
          value: '#000000',
        },
        {
          name: 'arwes',
          value: 'hsla(180, 69%, 5%, 1)',
        },
      ],
    },
    layout: 'padded',
  },
  decorators: [
    (Story) => (
      <AnimatorGeneralProvider {...animatorGeneralSettings}>
        <BleepsProvider {...bleepsSettings}>
          <div style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
            <Story />
          </div>
        </BleepsProvider>
      </AnimatorGeneralProvider>
    ),
  ],
};

export default preview;