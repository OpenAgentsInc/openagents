'use client'

import React from 'react'
import { Animator, Animated, BleepsOnAnimator, cx } from '@arwes/react'
import Link from 'next/link'
import { ArwesLogoType } from '@/components/ArwesLogoType'
import { ButtonSimple } from '@/components/ButtonSimple'
import { PageLayout } from '@/components/PageLayout'
import { Rocket, FileText, Github } from 'lucide-react'

const Home = (): JSX.Element => {
  return (
    <PageLayout>
      <Animator combine manager="sequenceReverse">
        <BleepsOnAnimator transitions={{ entering: 'info' }} continuous />

        <Animated
          as="main"
          className={cx('flex flex-col justify-center items-center gap-4 h-full w-full p-6', 'md:gap-8')}
          animated={[['y', 24, 0, 0]]}
        >
        <Animator>
          <Animated as="h1" className="pb-2" title="OpenAgents">
            <ArwesLogoType className="text-6xl md:text-8xl" />
          </Animated>
        </Animator>

        <Animator>
          <Animated
            as="nav"
            className="flex flex-row justify-center items-center gap-2 md:gap-4 mt-8"
            animated={['flicker']}
          >
            <Link href="/signin">
              <ButtonSimple
                tabIndex={-1}
                title="Get Started"
                animated={[['x', -24, 0, 0]]}
              >
                <Rocket size={14} />
                <span>Get Started</span>
              </ButtonSimple>
            </Link>

            <Link href="/docs">
              <ButtonSimple
                tabIndex={-1}
                title="Go to Documentation"
                animated={[['x', -12, 0, 0]]}
              >
                <FileText size={14} />
                <span>Documentation</span>
              </ButtonSimple>
            </Link>

            <a href="https://github.com/OpenAgentsInc" target="_blank" rel="noopener noreferrer">
              <ButtonSimple
                tabIndex={-1}
                title="Go to GitHub"
                animated={[['x', 12, 0, 0]]}
              >
                <Github size={14} />
                <span>GitHub</span>
              </ButtonSimple>
            </a>
          </Animated>
        </Animator>

        {/* <Animator>
          <Animated
            className="grid grid-cols-1 md:grid-cols-3 gap-6 mt-20 max-w-6xl mx-auto"
            animated={['flicker']}
          >
            <div className="border border-cyan-500 p-6 bg-black/50 backdrop-blur">
              <h3 className="text-xl font-bold mb-3 uppercase font-sans">
                Lightning Native
              </h3>
              <p className="opacity-80 font-mono text-sm">
                Agents that can send and receive Bitcoin payments instantly through the Lightning Network.
              </p>
            </div>

            <div className="border border-cyan-500 p-6 bg-black/50 backdrop-blur">
              <h3 className="text-xl font-bold mb-3 uppercase font-sans">
                Nostr Protocol
              </h3>
              <p className="opacity-80 font-mono text-sm">
                Decentralized communication between agents using the Nostr protocol for censorship resistance.
              </p>
            </div>

            <div className="border border-cyan-500 p-6 bg-black/50 backdrop-blur">
              <h3 className="text-xl font-bold mb-3 uppercase font-sans">
                Effect System
              </h3>
              <p className="opacity-80 font-mono text-sm">
                Built with Effect for robust error handling, async operations, and functional programming.
              </p>
            </div>
          </Animated>
        </Animator> */}
      </Animated>
    </Animator>
    </PageLayout>
  )
}

export default Home