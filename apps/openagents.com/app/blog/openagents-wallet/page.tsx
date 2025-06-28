'use client'

import { BlogPageWrapper } from '@/components/blog/BlogPageWrapper';
import { TwitterEmbed } from '@/components/blog/TwitterEmbed';
import { Text, GridLines, Dots, Animator, Animated } from '@arwes/react';
import { CalendarDays, Clock } from 'lucide-react';

export default function Page() {
  return (
    <BlogPageWrapper>
      <div className="relative z-10 h-full overflow-y-auto">
        <div className="absolute inset-0 pointer-events-none">
          <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
          <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
        </div>
        
        <article className="relative max-w-4xl mx-auto px-8 py-12">
          <Animator active={true}>
            <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
              <header className="mb-8">
                <img 
                  src="/images/blog04.png" 
                  alt="Introducing the OpenAgents Wallet"
                  className="w-full h-64 md:h-96 object-cover mb-8 -mx-8 -mt-12"
                />
                
                <Text as="h1" className="text-3xl md:text-4xl font-bold text-cyan-100 mb-4">
                  Introducing the OpenAgents Wallet
                </Text>
                
                <Text className="text-lg text-cyan-300/60 mb-4">
                  Earning and spending bitcoin with AI agents should be easy and fun. Now it is!
                </Text>
                
                <div className="flex items-center gap-4 text-sm text-cyan-500/60">
                  <div className="flex items-center gap-2">
                    <CalendarDays size={16} />
                    <Text>May 13, 2025</Text>
                  </div>
                  <div className="flex items-center gap-2">
                    <Clock size={16} />
                    <Text>3 min read</Text>
                  </div>
                </div>
              </header>
              
              <div className="prose prose-invert prose-cyan max-w-none">
                <TwitterEmbed tweetId="1922303008617984363" />

                <h3>tl;dr</h3>

                <p>We just launched a web wallet at <a href="https://wallet.openagents.com">wallet.openagents.com</a> with support for Bitcoin and Lightning through the new <a href="https://spark.money">Spark SDK</a>.</p>

                <h3>Why</h3>

                <p>We needed an easy way for any person or AI agent to earn and spend bitcoin.</p>

                <p>Here's what we wanted from our wallet:</p>

                <ul>
                  <li>100% open-source - both the <a href="https://github.com/OpenAgentsInc/openagents/tree/main/apps/wallet">wallet</a> and <a href="https://github.com/buildonspark/spark">payments infrastructure</a></li>
                  <li>Self-custodial - no one holds your funds but you</li>
                  <li>No login required - only a seed phrase</li>
                  <li>Never asks you for sensitive personal information</li>
                  <li>No blockchains other than bitcoin</li>
                  <li>First-class support for Bitcoin Lightning</li>
                  <li>First-class support for bitcoin-native stablecoins</li>
                  <li>No fees for payments between OpenAgents wallets</li>
                </ul>

                <h3>Help test it</h3>

                <p>We'll be throwing a few satoshis at a few folks in <a href="https://openagents.test/blog/discord">our Discord</a> who help test it out and report any bugs.</p>

                <h3>How not to use it</h3>

                <p>As a web wallet connected to various OpenAgents products, this is <strong>not</strong> for long-term storage of your bitcoin stack. This is an experimental hot wallet you should assume is not secure.</p>

                <p>Don't load more than a handful of satoshis at a time. When you earn bitcoin to your wallet, please withdraw it regularly to a more established self-custodial wallet. (Try <a href="https://zeusln.com/">Zeus</a>!)</p>

                <h3>Coming soon</h3>

                <ul>
                  <li>Integration with our <a href="/blog/agent-payments-api">Agent Payments API</a></li>
                  <li>Integration with our <a href="https://x.com/OpenAgentsInc/status/1919797578452869267">Commander</a> product</li>
                  <li>And <a href="https://x.com/OpenAgentsInc/status/1919428581358227525">more</a></li>
                </ul>
              </div>
              
              <footer className="mt-12 pt-8 border-t border-cyan-500/20">
                <div className="flex justify-between items-center">
                  <a 
                    href="/blog" 
                    className="text-cyan-400 hover:text-cyan-300 transition-colors"
                  >
                    ← Back to Blog
                  </a>
                </div>
              </footer>
            </Animated>
          </Animator>
        </article>
      </div>
    </BlogPageWrapper>
  );
}