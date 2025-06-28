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
import { ButtonSimple } from './ButtonSimple';
import { useAuth } from '@/hooks/useAuth';
import { Menu as MenuIcon } from 'lucide-react';
import { Github, X, SoundHigh, SoundOff } from 'iconoir-react';
import { ChatSidebar } from './ChatSidebar';

interface LayoutWithFramesProps {
  children: ReactNode;
  showSidebar?: boolean;
}

const HEIGHT_CLASS = 'h-10 md:h-12';

export const LayoutWithFrames = (props: LayoutWithFramesProps): React.ReactElement => {
  const { children, showSidebar = false } = props;
  const pathname = usePathname();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [isAudioEnabled, setIsAudioEnabled] = useState(true);
  const { isAuthenticated, user, signIn, signOut } = useAuth();

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
                
                {!showSidebar && (
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
                      href="/projects" 
                      className={cx(
                        'relative px-4 py-2',
                        'text-cyan-500 hover:text-cyan-300',
                        'font-mono text-xs uppercase tracking-wider',
                        'transition-all duration-200',
                        'hover:bg-cyan-500/10',
                        pathname.startsWith('/projects') && 'text-cyan-300 bg-cyan-500/10'
                      )}
                    >
                      <Text>Projects</Text>
                      {pathname.startsWith('/projects') && (
                        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                      )}
                    </Link>
                    <Link 
                      href="/templates" 
                      className={cx(
                        'relative px-4 py-2',
                        'text-cyan-500 hover:text-cyan-300',
                        'font-mono text-xs uppercase tracking-wider',
                        'transition-all duration-200',
                        'hover:bg-cyan-500/10',
                        pathname.startsWith('/templates') && 'text-cyan-300 bg-cyan-500/10'
                      )}
                    >
                      <Text>Templates</Text>
                      {pathname.startsWith('/templates') && (
                        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                      )}
                    </Link>
                    <Link 
                      href="/gallery" 
                      className={cx(
                        'relative px-4 py-2',
                        'text-cyan-500 hover:text-cyan-300',
                        'font-mono text-xs uppercase tracking-wider',
                        'transition-all duration-200',
                        'hover:bg-cyan-500/10',
                        pathname.startsWith('/gallery') && 'text-cyan-300 bg-cyan-500/10'
                      )}
                    >
                      <Text>Gallery</Text>
                      {pathname.startsWith('/gallery') && (
                        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-cyan-300" />
                      )}
                    </Link>
                  </nav>
                )}
              </div>

              {/* Right side icons */}
              <div className="flex items-center gap-3">
                {/* Auth buttons */}
                <div className="hidden md:flex items-center gap-2">
                  {isAuthenticated ? (
                    <div className="flex items-center gap-2">
                      <span className="text-cyan-300/80 text-xs font-mono">
                        {user?.login || user?.name || 'User'}
                      </span>
                      <ButtonSimple 
                        onClick={signOut}
                        className="px-3 h-8 text-[10px]"
                      >
                        Sign Out
                      </ButtonSimple>
                    </div>
                  ) : (
                    <div className="flex items-center gap-2">
                      <ButtonSimple 
                        onClick={signIn}
                        className="px-3 h-8 text-[10px]"
                      >
                        Sign In
                      </ButtonSimple>
                    </div>
                  )}
                </div>
                
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
                      'transition-all duration-200',
                      'cursor-pointer'
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
        {showSidebar && <ChatSidebar />}
        
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
            
            <div className="relative h-full overflow-y-auto">
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
              {/* Navigation links for mobile */}
              <nav className="space-y-2">
                <Link href="/" className="block px-4 py-2 text-cyan-500 hover:text-cyan-300">Home</Link>
                <Link href="/projects" className="block px-4 py-2 text-cyan-500 hover:text-cyan-300">Projects</Link>
                <Link href="/templates" className="block px-4 py-2 text-cyan-500 hover:text-cyan-300">Templates</Link>
                <Link href="/gallery" className="block px-4 py-2 text-cyan-500 hover:text-cyan-300">Gallery</Link>
              </nav>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};