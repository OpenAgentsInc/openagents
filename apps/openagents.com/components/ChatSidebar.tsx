'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cx, Text, FrameCorners, FrameKranox, Animator, AnimatorGeneralProvider, Animated } from '@arwes/react';
import { Plus, MessageSquare, Settings, User, LogOut, ChevronRight, Sparkles } from 'lucide-react';
import { Github, X, SoundHigh, SoundOff } from 'iconoir-react';
import { useAuth } from '@/hooks/useAuth';
import { ButtonSimple } from './ButtonSimple';
import { ArwesLogoIcon } from './ArwesLogoIcon';
import { ArwesLogoType } from './ArwesLogoType';

interface ChatSession {
  id: string;
  title: string;
  timestamp: Date;
  preview?: string;
}

// Chat item component
const ChatItem = ({ 
  session, 
  isHovered, 
  onHover 
}: { 
  session: ChatSession; 
  isHovered: boolean;
  onHover: (id: string | null) => void;
}) => {
  return (
    <Link
      href={`/chat/${session.id}`}
      onMouseEnter={() => onHover(session.id)}
      onMouseLeave={() => onHover(null)}
      className={cx(
        'block relative group transition-all duration-200',
        isHovered && 'scale-[1.02]'
      )}
    >
      <div className="relative">
        {/* Background frame on hover */}
        {isHovered && (
          <div className="absolute inset-0 pointer-events-none">
            <FrameCorners
              style={{
                '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.5)',
                '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.3)',
                '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 0.4)'
              } as React.CSSProperties}
            />
          </div>
        )}
        
        {/* Content */}
        <div className={cx(
          'relative px-3 py-2.5 flex items-center gap-2',
          'transition-all duration-200',
          isHovered ? 'text-cyan-100' : 'text-cyan-300/70'
        )}>
          <MessageSquare size={16} className={cx(
            'flex-shrink-0 transition-all duration-200',
            isHovered ? 'text-cyan-400' : 'text-cyan-500/40'
          )} />
          
          <div className="flex-1 min-w-0">
            <Text className={cx(
              'text-sm font-medium truncate block transition-all duration-200',
              isHovered && 'text-cyan-200'
            )}>
              {session.title}
            </Text>
          </div>
          
          {isHovered && (
            <ChevronRight size={14} className="text-cyan-400/60 flex-shrink-0 animate-pulse" />
          )}
        </div>
      </div>
    </Link>
  );
};

export const ChatSidebar = (): React.ReactElement => {
  const pathname = usePathname();
  const { isAuthenticated, user, signIn, signOut } = useAuth();
  const [hoveredSession, setHoveredSession] = useState<string | null>(null);
  const [isAudioEnabled, setIsAudioEnabled] = useState(true);
  
  // Mock chat sessions organized by time
  const [sessions] = useState<{
    today: ChatSession[];
    yesterday: ChatSession[];
    previous: ChatSession[];
  }>({
    today: [
      { id: '1', title: 'Bitcoin Lightning App', timestamp: new Date(Date.now() - 3600000), preview: 'Create a Bitcoin Lightning payment app...' },
      { id: '2', title: 'React Dashboard', timestamp: new Date(Date.now() - 7200000), preview: 'Build a React dashboard with charts...' },
    ],
    yesterday: [
      { id: '3', title: 'API Integration', timestamp: new Date(Date.now() - 86400000), preview: 'How to integrate with OpenRouter API...' },
    ],
    previous: [
      { id: '4', title: 'E-commerce Site', timestamp: new Date(Date.now() - 172800000), preview: 'Build a modern e-commerce platform...' },
      { id: '5', title: 'Blog Platform', timestamp: new Date(Date.now() - 259200000), preview: 'Create a blog with markdown support...' },
    ]
  });

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <div className="w-72 h-full bg-black/80 backdrop-blur-sm flex flex-col relative border-r border-cyan-500/20">
        {/* Background Frame */}
        <div className="absolute inset-0 pointer-events-none">
          <FrameCorners
            style={{
              '--arwes-frames-bg-color': 'transparent',
              '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.1)',
              '--arwes-frames-deco-color': 'hsla(180, 75%, 70%, 0.2)'
            } as React.CSSProperties}
          />
        </div>

        {/* Subtle glow effect */}
        <div className="absolute inset-y-0 -right-px w-px bg-gradient-to-b from-transparent via-cyan-500/20 to-transparent pointer-events-none" />

        {/* Content */}
        <div className="relative flex flex-col h-full">
          {/* Logo Header */}
          <div className="p-4 border-b border-cyan-500/10">
            <Animator active={true}>
              <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                <Link href="/" className="flex items-center gap-3 opacity-80 hover:opacity-100 transition-opacity">
                  <ArwesLogoIcon className="w-7 h-7" />
                  <ArwesLogoType className="text-lg" />
                </Link>
              </Animated>
            </Animator>
          </div>

          {/* New Chat Button */}
          <div className="px-4 pb-4 pt-3 border-b border-cyan-500/10">
            <Animator active={true} duration={{ delay: 0.1 }}>
              <Animated animated={[['opacity', 0, 1], ['y', -10, 0]]}>
                <Link href="/" className="block">
                  <ButtonSimple className="w-full justify-center h-10">
                    <Sparkles size={16} className="animate-pulse" />
                    <span className="text-sm">New chat</span>
                  </ButtonSimple>
                </Link>
              </Animated>
            </Animator>
          </div>

          {/* Chat History */}
          <div className="flex-1 overflow-y-auto custom-scrollbar">
            <div className="p-3 space-y-4">
              {/* Today's chats */}
              {sessions.today.length > 0 && (
                <Animator active={true} duration={{ delay: 0.1 }}>
                  <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                    <div>
                      <Text className="text-[10px] text-cyan-500/40 uppercase tracking-wider font-bold px-3 mb-2">
                        Today
                      </Text>
                      <div className="space-y-1">
                        {sessions.today.map((session) => (
                          <ChatItem 
                            key={session.id} 
                            session={session}
                            isHovered={hoveredSession === session.id}
                            onHover={setHoveredSession}
                          />
                        ))}
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )}

              {/* Yesterday's chats */}
              {sessions.yesterday.length > 0 && (
                <Animator active={true} duration={{ delay: 0.2 }}>
                  <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                    <div>
                      <Text className="text-[10px] text-cyan-500/40 uppercase tracking-wider font-bold px-3 mb-2">
                        Yesterday
                      </Text>
                      <div className="space-y-1">
                        {sessions.yesterday.map((session) => (
                          <ChatItem 
                            key={session.id} 
                            session={session}
                            isHovered={hoveredSession === session.id}
                            onHover={setHoveredSession}
                          />
                        ))}
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )}

              {/* Previous chats */}
              {sessions.previous.length > 0 && (
                <Animator active={true} duration={{ delay: 0.3 }}>
                  <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                    <div>
                      <Text className="text-[10px] text-cyan-500/40 uppercase tracking-wider font-bold px-3 mb-2">
                        Previous 7 Days
                      </Text>
                      <div className="space-y-1">
                        {sessions.previous.map((session) => (
                          <ChatItem 
                            key={session.id} 
                            session={session}
                            isHovered={hoveredSession === session.id}
                            onHover={setHoveredSession}
                          />
                        ))}
                      </div>
                    </div>
                  </Animated>
                </Animator>
              )}
            </div>
          </div>

          {/* User Section */}
          <div className="border-t border-cyan-500/10">
            {isAuthenticated ? (
              <Animator active={true} duration={{ delay: 0.4 }}>
                <Animated animated={[['opacity', 0, 1], ['y', 10, 0]]}>
                  <div className="p-3">
                    <div className="relative">
                      <FrameKranox
                        style={{
                          '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.3)',
                          '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.3)',
                        } as React.CSSProperties}
                      />
                      <div className="relative p-3 flex items-center justify-between">
                        <div className="flex items-center gap-3">
                          <div className="w-8 h-8 rounded-full bg-cyan-500/20 flex items-center justify-center border border-cyan-500/30">
                            <User size={16} className="text-cyan-400" />
                          </div>
                          <div className="flex-1 min-w-0">
                            <Text className="text-sm text-cyan-300 font-medium truncate block">
                              {user?.login || user?.name || 'User'}
                            </Text>
                            <Text className="text-xs text-cyan-500/50">
                              Free tier
                            </Text>
                          </div>
                        </div>
                        <button
                          onClick={signOut}
                          className="p-2 hover:bg-cyan-500/10 rounded transition-colors"
                          title="Sign out"
                        >
                          <LogOut size={16} className="text-cyan-500/60 hover:text-cyan-400" />
                        </button>
                      </div>
                    </div>
                  </div>
                </Animated>
              </Animator>
            ) : (
              <div className="p-4">
                <ButtonSimple 
                  onClick={signIn}
                  className="w-full justify-center text-xs"
                >
                  <User size={14} />
                  <span>Sign in</span>
                </ButtonSimple>
              </div>
            )}

            {/* Bottom section with settings and social icons */}
            <div className="p-3 space-y-3">
              {/* Settings link */}
              <Link
                href="/settings"
                className={cx(
                  'flex items-center gap-2 px-3 py-2 rounded',
                  'hover:bg-cyan-500/10 transition-all duration-200',
                  'text-sm text-cyan-300/60 hover:text-cyan-300'
                )}
              >
                <Settings size={16} />
                <Text>Settings</Text>
              </Link>

              {/* Social icons */}
              <div className="flex items-center justify-center gap-2 px-3 py-2">
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
            </div>
          </div>
        </div>

        {/* Custom scrollbar styles */}
        <style jsx>{`
          .custom-scrollbar::-webkit-scrollbar {
            width: 6px;
          }
          .custom-scrollbar::-webkit-scrollbar-track {
            background: transparent;
          }
          .custom-scrollbar::-webkit-scrollbar-thumb {
            background: hsla(180, 75%, 50%, 0.2);
            border-radius: 3px;
          }
          .custom-scrollbar::-webkit-scrollbar-thumb:hover {
            background: hsla(180, 75%, 50%, 0.3);
          }
        `}</style>
      </div>
    </AnimatorGeneralProvider>
  );
};