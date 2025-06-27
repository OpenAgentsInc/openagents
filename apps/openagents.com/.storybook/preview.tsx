import type { Preview } from '@storybook/nextjs'
import React from 'react'
import { AnimatorGeneralProvider, BleepsProvider } from '@arwes/react'
import '../app/globals.css'
import './fonts.css'

// Import Arwes configuration
import { animatorGeneralSettings } from '../config/animator'
import { bleepsSettings } from '../config/bleeps'

// Mock Next.js navigation
const mockRouter = {
  pathname: '/',
  route: '/',
  query: {},
  asPath: '/',
  push: () => Promise.resolve(true),
  replace: () => Promise.resolve(true),
  reload: () => {},
  back: () => {},
  prefetch: () => Promise.resolve(),
  beforePopState: () => {},
  events: {
    on: () => {},
    off: () => {},
    emit: () => {},
  },
}

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
    docs: {
      canvas: {
        sourceState: 'shown',
      },
    },
    layout: 'padded',
    nextjs: {
      navigation: {
        pathname: '/',
      },
      router: mockRouter,
    },
  },
  decorators: [
    (Story, context) => {
      // Force dark background for all stories, especially docs
      const isDocsMode = context.viewMode === 'docs'
      
      return (
        <AnimatorGeneralProvider {...animatorGeneralSettings}>
          <BleepsProvider {...bleepsSettings}>
            <div 
              className="font-mono"
              style={{
                backgroundColor: '#000000',
                color: '#ffffff',
                minHeight: isDocsMode ? 'auto' : 'auto',
                padding: isDocsMode ? '20px' : '20px',
                overflow: 'visible',
                height: 'auto'
              }}
            >
              <Story />
            </div>
          </BleepsProvider>
        </AnimatorGeneralProvider>
      )
    },
  ],
};

export default preview;