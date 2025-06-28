'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cx, Text } from '@arwes/react';
import { Plus, Search, Clock, Settings, User, LogOut } from 'lucide-react';
import { useAuth } from '@/hooks/useAuth';
import { ButtonSimple } from './ButtonSimple';

interface ChatSession {
  id: string;
  title: string;
  timestamp: Date;
  preview?: string;
}

export const ChatSidebar = (): React.ReactElement => {
  const pathname = usePathname();
  const { isAuthenticated, user, signOut } = useAuth();
  const [searchQuery, setSearchQuery] = useState('');
  
  // Mock chat sessions for now
  const [sessions] = useState<ChatSession[]>([
    { id: '1', title: 'Bitcoin Lightning App', timestamp: new Date(Date.now() - 3600000), preview: 'Create a Bitcoin Lightning payment app...' },
    { id: '2', title: 'React Dashboard', timestamp: new Date(Date.now() - 7200000), preview: 'Build a React dashboard with charts...' },
    { id: '3', title: 'API Integration', timestamp: new Date(Date.now() - 86400000), preview: 'How to integrate with OpenRouter API...' },
  ]);

  const filteredSessions = sessions.filter(session => 
    session.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
    session.preview?.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const getRelativeTime = (date: Date) => {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const hours = Math.floor(diff / 3600000);
    if (hours < 1) return 'Just now';
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days === 1) return 'Yesterday';
    return `${days}d ago`;
  };

  return (
    <div className="w-64 h-full bg-black/40 border-r border-cyan-500/20 flex flex-col">
      {/* New Chat Button */}
      <div className="p-3">
        <Link href="/" className="block">
          <ButtonSimple className="w-full justify-center">
            <Plus size={14} />
            <span>New chat</span>
          </ButtonSimple>
        </Link>
      </div>

      {/* Search */}
      <div className="px-3 pb-3">
        <div className="relative">
          <Search size={16} className="absolute left-3 top-1/2 transform -translate-y-1/2 text-cyan-500/40" />
          <input
            type="text"
            placeholder="Search chats"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className={cx(
              'w-full pl-9 pr-3 py-2',
              'bg-black/30 border border-cyan-500/20',
              'rounded-lg text-sm text-cyan-300',
              'placeholder-cyan-500/40',
              'focus:outline-none focus:border-cyan-500/40',
              'transition-all duration-200'
            )}
          />
        </div>
      </div>

      {/* Chat History */}
      <div className="flex-1 overflow-y-auto px-3">
        <div className="mb-2">
          <Text className="text-xs text-cyan-500/60 uppercase tracking-wider px-2">Today</Text>
        </div>
        <div className="space-y-1">
          {filteredSessions.map((session) => (
            <Link
              key={session.id}
              href={`/chat/${session.id}`}
              className={cx(
                'block px-3 py-2 rounded-lg',
                'hover:bg-cyan-500/10',
                'transition-all duration-200',
                'group'
              )}
            >
              <div className="flex items-start justify-between">
                <div className="flex-1 min-w-0">
                  <Text className="text-sm text-cyan-300 font-medium truncate">
                    {session.title}
                  </Text>
                  <Text className="text-xs text-cyan-500/60 truncate mt-0.5">
                    {session.preview}
                  </Text>
                </div>
                <Text className="text-xs text-cyan-500/40 ml-2 flex-shrink-0">
                  {getRelativeTime(session.timestamp)}
                </Text>
              </div>
            </Link>
          ))}
        </div>
      </div>

      {/* User Section */}
      {isAuthenticated && (
        <div className="border-t border-cyan-500/20 p-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 rounded-full bg-cyan-500/20 flex items-center justify-center">
                <User size={16} className="text-cyan-400" />
              </div>
              <Text className="text-sm text-cyan-300 truncate max-w-[120px]">
                {user?.login || user?.name || 'User'}
              </Text>
            </div>
            <button
              onClick={signOut}
              className="p-2 hover:bg-cyan-500/10 rounded-lg transition-colors"
              title="Sign out"
            >
              <LogOut size={16} className="text-cyan-500/60 hover:text-cyan-400" />
            </button>
          </div>
        </div>
      )}

      {/* Bottom Actions */}
      <div className="border-t border-cyan-500/20 p-3 space-y-1">
        <Link
          href="/projects"
          className={cx(
            'flex items-center gap-2 px-3 py-2 rounded-lg',
            'hover:bg-cyan-500/10 transition-all duration-200',
            'text-sm text-cyan-300/80 hover:text-cyan-300'
          )}
        >
          <Clock size={16} />
          <Text>Your Projects</Text>
        </Link>
        <Link
          href="/settings"
          className={cx(
            'flex items-center gap-2 px-3 py-2 rounded-lg',
            'hover:bg-cyan-500/10 transition-all duration-200',
            'text-sm text-cyan-300/80 hover:text-cyan-300'
          )}
        >
          <Settings size={16} />
          <Text>Settings</Text>
        </Link>
      </div>
    </div>
  );
};