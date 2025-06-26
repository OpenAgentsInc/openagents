'use client';

import React from 'react';
import { Animator, Animated, cx, Text, FrameOctagon } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { MessageSquare, Zap, Shield, Globe } from 'lucide-react';

const HomePage = (): React.ReactElement => {
  return (
    <AppLayout>
      <div className="space-y-8">
        {/* Welcome Section */}
        <Animator combine manager="stagger">
          <Animator>
            <Animated animated={[['y', 20, 0, 0]]}>
              <div className="space-y-4">
                <h1 className="text-4xl font-bold font-mono text-cyan-300">
                  <Text>Welcome to OpenAgents</Text>
                </h1>
                <p className="text-lg text-cyan-500/80 font-mono">
                  <Text>Bitcoin-powered AI agents for the decentralized future</Text>
                </p>
              </div>
            </Animated>
          </Animator>

          {/* Quick Stats */}
          <Animator>
            <Animated
              className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mt-8"
              animated={[['y', 20, 0, 0]]}
            >
              {[
                { icon: <MessageSquare />, label: 'Active Chats', value: '12' },
                { icon: <Zap />, label: 'Lightning Txns', value: '1,337' },
                { icon: <Shield />, label: 'Agents Online', value: '42' },
                { icon: <Globe />, label: 'Global Nodes', value: '256' },
              ].map((stat, index) => (
                <div
                  key={index}
                  className="relative p-4 border border-cyan-500/30 rounded bg-black/50 backdrop-blur"
                >
                  <div className="flex items-center gap-3">
                    <span className="text-cyan-500/70">{stat.icon}</span>
                    <div>
                      <p className="text-2xl font-bold text-cyan-300 font-mono">{stat.value}</p>
                      <p className="text-sm text-cyan-500/70 font-mono uppercase">{stat.label}</p>
                    </div>
                  </div>
                </div>
              ))}
            </Animated>
          </Animator>

          {/* Main Content Area */}
          <Animator>
            <Animated
              className="relative mt-12 p-8 border border-cyan-500/30 rounded bg-black/30 backdrop-blur"
              animated={[['y', 20, 0, 0]]}
            >
              <div className="absolute inset-0 overflow-hidden rounded opacity-20">
                <FrameOctagon />
              </div>
              
              <div className="relative space-y-6">
                <h2 className="text-2xl font-bold text-cyan-300 font-mono">
                  <Text>Getting Started</Text>
                </h2>
                
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                  <div className="space-y-3">
                    <h3 className="text-lg font-semibold text-cyan-400 font-mono">
                      <Text>Create Your First Agent</Text>
                    </h3>
                    <p className="text-cyan-500/70 font-mono text-sm leading-relaxed">
                      Deploy autonomous agents that can interact with the Bitcoin Lightning Network,
                      process payments, and communicate via the Nostr protocol.
                    </p>
                  </div>
                  
                  <div className="space-y-3">
                    <h3 className="text-lg font-semibold text-cyan-400 font-mono">
                      <Text>Explore the Playground</Text>
                    </h3>
                    <p className="text-cyan-500/70 font-mono text-sm leading-relaxed">
                      Test agent capabilities in our interactive playground. Experiment with
                      different models, prompts, and Lightning Network integrations.
                    </p>
                  </div>
                </div>
              </div>
            </Animated>
          </Animator>

          {/* Recent Activity */}
          <Animator>
            <Animated
              className="mt-8 space-y-4"
              animated={[['y', 20, 0, 0]]}
            >
              <h2 className="text-xl font-bold text-cyan-300 font-mono">
                <Text>Recent Activity</Text>
              </h2>
              
              <div className="space-y-2">
                {[
                  { time: '2 mins ago', action: 'New agent deployed: WeatherBot' },
                  { time: '5 mins ago', action: 'Lightning payment: 1000 sats received' },
                  { time: '12 mins ago', action: 'Chat session started with GPT-4' },
                  { time: '1 hour ago', action: 'Agent training completed' },
                ].map((activity, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between p-3 border border-cyan-500/20 rounded bg-black/30"
                  >
                    <span className="text-cyan-400 font-mono text-sm">{activity.action}</span>
                    <span className="text-cyan-500/50 font-mono text-xs">{activity.time}</span>
                  </div>
                ))}
              </div>
            </Animated>
          </Animator>
        </Animator>
      </div>
    </AppLayout>
  );
};

export default HomePage;