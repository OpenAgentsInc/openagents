'use client'

import { BlogPageWrapper } from '@/components/blog/BlogPageWrapper';
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
                  src="/images/blog03.png" 
                  alt="We're on Discord"
                  className="w-full h-64 md:h-96 object-cover mb-8 -mx-8 -mt-12"
                />
                
                <Text as="h1" className="text-3xl md:text-4xl font-bold text-cyan-100 mb-4">
                  We're on Discord
                </Text>
                
                <Text className="text-lg text-cyan-300/60 mb-4">
                  OpenAgents now has a Discord. You should join.
                </Text>
                
                <div className="flex items-center gap-4 text-sm text-cyan-500/60">
                  <div className="flex items-center gap-2">
                    <CalendarDays size={16} />
                    <Text>May 10, 2025</Text>
                  </div>
                  <div className="flex items-center gap-2">
                    <Clock size={16} />
                    <Text>2 min read</Text>
                  </div>
                </div>
              </header>
              
              <div className="prose prose-invert prose-cyan max-w-none">
                <p>It's barebones for now but you can ask us anything or throw any ideas at us.</p>

                <p>Join here: <a href="https://discord.gg/ShuRwwAZAM">https://discord.gg/ShuRwwAZAM</a></p>

                <p>We'll soon organize playtests there for our upcoming <a href="https://x.com/OpenAgentsInc/status/1919797578452869267">Commander product</a>.</p>

                <p>Commander will look and feel a lot like a video game-- so we think Discord is the right place to grow its community.</p>

                <p>See you there!</p>
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