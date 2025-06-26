"use client";

import React, { type ReactNode, useState } from 'react';
import { usePathname } from 'next/navigation';
import Link from 'next/link';
import {
  Animated,
  FrameOctagon,
  styleFrameClipOctagon,
  cx,
  Animator,
  Text
} from '@arwes/react';
import { ArwesLogoIcon } from './ArwesLogoIcon';
import { ArwesLogoType } from './ArwesLogoType';
import { NavSidebar } from './NavSidebar';
import { Menu as MenuIcon } from 'lucide-react';
import { Github, X, SoundHigh, SoundOff } from 'iconoir-react';

interface LayoutWithFramesProps {
  children: ReactNode;
}

const HEIGHT_CLASS = 'h-10 md:h-12';

export const LayoutWithFrames = (props: LayoutWithFramesProps): React.ReactElement => {
  const { children } = props;
  const pathname = usePathname();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [isAudioEnabled, setIsAudioEnabled] = useState(true);

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* Header */}
      <header className="relative">
        <div className="p-4">
          <div className="relative">
            {/* Header Background Frame */}
            <div 
              className="absolute inset-0"
              style={{
                clipPath: styleFrameClipOctagon({ squareSize: 8 })
              }}
            >
              <FrameOctagon
                style={{
                  // @ts-expect-error CSS variables
                  '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.1)',
                  '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                }}
                squareSize={8}
              />
            </div>

            {/* Header Content */}
            <div className="relative flex items-center justify-between px-6 h-14">
              <div className="flex items-center gap-6">
                <Link href="/" className="flex items-center gap-2 opacity-80 hover:opacity-100 transition-opacity">
                  <ArwesLogoIcon className="w-6 h-6" />
                  <ArwesLogoType className="h-4" />
                </Link>
                
                <nav className="hidden md:flex items-center gap-2">
                  <Link 
                    href="/" 
                    className={cx(
                      'relative px-4 py-2',
                      'text-cyan-500 hover:text-cyan-300',
                      'font-mono text-xs uppercase tracking-wider',
                      'transition-all duration-200',
                      'hover:bg-cyan-500/10',
                      pathname === '/' && 'text-cyan-300 bg-cyan-500/10'
                    )}
                  >
                    <Text>Home</Text>
                    {pathname === '/' && (
                      <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                    )}
                  </Link>
                  <Link 
                    href="/chat" 
                    className={cx(
                      'relative px-4 py-2',
                      'text-cyan-500 hover:text-cyan-300',
                      'font-mono text-xs uppercase tracking-wider',
                      'transition-all duration-200',
                      'hover:bg-cyan-500/10',
                      pathname === '/chat' && 'text-cyan-300 bg-cyan-500/10'
                    )}
                  >
                    <Text>Chat</Text>
                    {pathname === '/chat' && (
                      <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                    )}
                  </Link>
                  <Link 
                    href="/agents" 
                    className={cx(
                      'relative px-4 py-2',
                      'text-cyan-500 hover:text-cyan-300',
                      'font-mono text-xs uppercase tracking-wider',
                      'transition-all duration-200',
                      'hover:bg-cyan-500/10',
                      pathname === '/agents' && 'text-cyan-300 bg-cyan-500/10'
                    )}
                  >
                    <Text>Agents</Text>
                    {pathname === '/agents' && (
                      <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                    )}
                  </Link>
                </nav>
              </div>

              {/* Right side icons */}
              <div className="flex items-center gap-3">
                {/* Desktop icons */}
                <div className="hidden md:flex items-center gap-2">
                  <a
                    href="https://github.com/OpenAgentsInc/openagents"
                    target="_blank"
                    rel="noopener noreferrer"
                    className={cx(
                      'p-2 rounded',
                      'text-cyan-500/60 hover:text-cyan-300 hover:bg-cyan-500/10',
                      'transition-all duration-200'
                    )}
                    title="View on GitHub"
                  >
                    <Github width={19} height={19} />
                  </a>
                  <a
                    href="https://x.com/OpenAgentsInc"
                    target="_blank"
                    rel="noopener noreferrer"
                    className={cx(
                      'p-2 rounded',
                      'text-cyan-500/60 hover:text-cyan-300 hover:bg-cyan-500/10',
                      'transition-all duration-200'
                    )}
                    title="Follow on X"
                  >
                    <X width={19} height={19} />
                  </a>
                  <button
                    onClick={() => setIsAudioEnabled(!isAudioEnabled)}
                    className={cx(
                      'p-2 rounded',
                      'text-cyan-500/60 hover:text-cyan-300 hover:bg-cyan-500/10',
                      'transition-all duration-200'
                    )}
                    title={isAudioEnabled ? 'Mute sounds' : 'Enable sounds'}
                  >
                    {isAudioEnabled ? <SoundHigh width={19} height={19} /> : <SoundOff width={19} height={19} />}
                  </button>
                </div>
                
                {/* Mobile menu button */}
                <button
                  onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                  className="md:hidden text-cyan-500 hover:text-cyan-300 p-2"
                >
                  <MenuIcon size={24} />
                </button>
              </div>
            </div>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <aside className="hidden lg:block w-64 p-4">
          <div className="relative h-full">
            <div 
              className="absolute inset-0"
              style={{
                clipPath: styleFrameClipOctagon({ squareSize: 8 })
              }}
            >
              <FrameOctagon
                style={{
                  // @ts-expect-error CSS variables
                  '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.1)',
                  '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                }}
                squareSize={8}
              />
            </div>
            <div className="relative p-4 h-full overflow-y-auto">
              <NavSidebar />
            </div>
          </div>
        </aside>

        {/* Main Content */}
        <main className="flex-1 p-4">
          <div className="relative h-full">
            {/* Main Background Frame */}
            <div 
              className="absolute inset-0"
              style={{
                clipPath: styleFrameClipOctagon({ squareSize: 16 })
              }}
            >
              <FrameOctagon
                style={{
                  // @ts-expect-error CSS variables
                  '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.15)',
                  '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                }}
                squareSize={16}
              />
            </div>
            
            <div className="relative p-8 h-full overflow-y-auto">
              {children}
            </div>
          </div>
        </main>
      </div>

      {/* Mobile Menu */}
      {mobileMenuOpen && (
        <div className="lg:hidden fixed inset-0 z-50 bg-black/95">
          <div className="flex flex-col h-full">
            <div className="flex items-center justify-between p-4 border-b border-cyan-500/30">
              <Text className="text-cyan-500 text-sm uppercase">Menu</Text>
              <button
                onClick={() => setMobileMenuOpen(false)}
                className="text-cyan-500"
              >
                ✕
              </button>
            </div>
            <div className="flex-1 p-4 overflow-y-auto">
              <NavSidebar />
            </div>
          </div>
        </div>
      )}
    </div>
  );
};