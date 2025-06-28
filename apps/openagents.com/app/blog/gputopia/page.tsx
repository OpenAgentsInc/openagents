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
                  src="/images/blog05.png" 
                  alt="GPUtopia 2.0"
                  className="w-full h-64 md:h-96 object-cover mb-8 -mx-8 -mt-12"
                />
                
                <Text as="h1" className="text-3xl md:text-4xl font-bold text-cyan-100 mb-4">
                  GPUtopia 2.0
                </Text>
                
                <Text className="text-lg text-cyan-300/60 mb-4">
                  We're rebooting our swarm compute network as OpenAgents Compute.
                </Text>
                
                <div className="flex items-center gap-4 text-sm text-cyan-500/60">
                  <div className="flex items-center gap-2">
                    <CalendarDays size={16} />
                    <Text>May 14, 2025</Text>
                  </div>
                  <div className="flex items-center gap-2">
                    <Clock size={16} />
                    <Text>4 min read</Text>
                  </div>
                </div>
              </header>
              
              <div className="prose prose-invert prose-cyan max-w-none">
                <TwitterEmbed tweetId="1922738011621687492" />

                <p>Transcript of episode 174:</p>

                <p>Folks I'm excited to announce that this month we are rebooting our swarm compute network, previously called GPUtopia, now called OpenAgents Compute.</p>

                <p>Why should you care?</p>

                <p>Because you can sell your spare compute for bitcoin - as simple as running a simple desktop app, adding your computer to our global compute marketplace and getting paid in bitcoin.</p>

                <p>This is something we already launched. It worked. We paid out I don't know how many tens or hundreds of compute providers. You could provide your compute first through a web application, then through a "workerbee" software that supported inference, finetuning, embedding, image generation, all through an OpenAI-compatible endpoint.</p>

                <p>We phased this out. Part of this was, we had a bunch of sellers, not enough buyers. Now this was 18 months ago. And we thought that in the future, the ideal buyer that's going to really help launch this as a two-sided marketplace are going to be agents.</p>

                <p>Once agents are good and easy enough to use, that should solve that demand issue.</p>

                <p>Eighteen months later, a pivot to OpenAgents later, 170 videos of building a whole bunch of agents infrastructure later, we're ready to bring this back in.</p>

                <p>So much has changed in the last 18 months.</p>

                <p>We've got great ways of running local models so that we don't need to be building this infrastructure anymore.</p>

                <p>We can just pay people for running things like Ollama, focusing on what we're good at.</p>

                <p>The thing we'll uniquely provide to this is the super cool interface. You're going to install an app, it's going to be a way for you to see the earnings and visualize the network activity, paired with a cool game-style sci-fi HUD for actually using these agents.</p>

                <p>It's going to feel a little bit like a video game, but it's going to have agent payments tied directly into it.</p>

                <p>We are going to be building and beta-testing this over the next week or so in our Discord.</p>

                <p>Our objective is to take all this stuff we've built so far and put it into one open-source software package.</p>

                <p>We're going to continue doing the whole build-in-public thing.</p>

                <p>We're going to be beta-testing things still in our Discord, that's been going great.</p>

                <p>We want to get the first payments out to sellers of compute a week from today at the latest.</p>

                <p>If you want to get a start on that, we're going to be using Ollama as our local model provider. So you'll need Ollama installed locally.</p>

                <p>Go <a href="https://ollama.com/download">install Ollama</a> and then obviously you'll have through OpenAgents the wallet, which works just fine as you can see in our <a href="https://x.com/OpenAgentsInc/status/1922303008617984363">last video</a>.</p>

                <p>Stay tuned!</p>

                <img src="/images/blog05a.png" className="border border-cyan-500/20 rounded-lg" />
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