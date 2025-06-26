'use client'

import React from 'react'
import { AnimatorGeneralProvider, BleepsProvider } from '@arwes/react'
import { Background } from './Background'

const bleepsSettings = {
  master: { volume: 0.5 },
  bleeps: {
    click: {
      sources: [
        { src: '/sounds/click.webm', type: 'audio/webm' },
        { src: '/sounds/click.mp3', type: 'audio/mpeg' }
      ]
    },
    type: {
      sources: [
        { src: '/sounds/type.webm', type: 'audio/webm' },
        { src: '/sounds/type.mp3', type: 'audio/mpeg' }
      ],
      volume: 0.3
    },
    info: {
      sources: [
        { src: '/sounds/info.webm', type: 'audio/webm' },
        { src: '/sounds/info.mp3', type: 'audio/mpeg' }
      ],
      volume: 0.5
    },
    error: {
      sources: [
        { src: '/sounds/error.webm', type: 'audio/webm' },
        { src: '/sounds/error.mp3', type: 'audio/mpeg' }
      ],
      volume: 0.5
    }
  }
}

export function PageLayout({ children }: { children: React.ReactNode }) {
  return (
    <AnimatorGeneralProvider>
      <BleepsProvider {...bleepsSettings}>
        <div className="relative h-screen w-screen overflow-hidden bg-black text-cyan-500">
          <Background />
          <div className="relative z-10 h-full w-full overflow-auto">
            {children}
          </div>
        </div>
      </BleepsProvider>
    </AnimatorGeneralProvider>
  )
}