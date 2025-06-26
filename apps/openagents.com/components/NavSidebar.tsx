"use client";

import React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { Animated, Animator, cx, Text, BleepsOnAnimator } from '@arwes/react';
import { Home, MessageSquare, Settings, FileText, Users, Zap } from 'lucide-react';

interface NavItem {
  href: string;
  label: string;
  icon: React.ReactNode;
}

const navItems: NavItem[] = [
  { href: '/', label: 'Home', icon: <Home size={18} /> },
  { href: '/chat', label: 'Chat', icon: <MessageSquare size={18} /> },
  { href: '/agents', label: 'Agents', icon: <Users size={18} /> },
  { href: '/playground', label: 'Playground', icon: <Zap size={18} /> },
  { href: '/docs', label: 'Documentation', icon: <FileText size={18} /> },
  { href: '/settings', label: 'Settings', icon: <Settings size={18} /> },
];

export const NavSidebar = (): React.ReactElement => {
  const pathname = usePathname();

  return (
    <nav className="space-y-2">
      <Animator combine manager="stagger" duration={{ stagger: 0.05 }}>
        <BleepsOnAnimator transitions={{ entering: 'click' }} />
        
        <div className="mb-6">
          <Animated animated={['flicker']}>
            <h2 className="text-cyan-500/60 font-mono text-xs uppercase tracking-wider mb-2">
              <Text>Navigation</Text>
            </h2>
          </Animated>
        </div>

        {navItems.map((item) => {
          const isActive = pathname === item.href;
          
          return (
            <Animator key={item.href}>
              <Link href={item.href}>
                <Animated
                  className={cx(
                    'flex items-center gap-3 px-3 py-2 rounded transition-all duration-200',
                    'hover:bg-cyan-500/5 hover:text-cyan-300/80',
                    'font-mono text-xs uppercase tracking-wider',
                    isActive ? 'bg-cyan-500/10 text-cyan-300/70' : 'text-cyan-500/60'
                  )}
                  animated={['flicker']}
                >
                  <span className="opacity-60">{item.icon}</span>
                  <Text>{item.label}</Text>
                </Animated>
              </Link>
            </Animator>
          );
        })}
      </Animator>

      {/* Add a separator */}
      <div className="my-6 border-t border-cyan-500/10" />

      {/* Quick Actions */}
      <Animator combine manager="stagger" duration={{ stagger: 0.05 }}>
        <div className="mb-4">
          <Animated animated={['flicker']}>
            <h2 className="text-cyan-500/60 font-mono text-xs uppercase tracking-wider mb-2">
              <Text>Quick Actions</Text>
            </h2>
          </Animated>
        </div>

        <Animator>
          <Animated
            as="button"
            className={cx(
              'w-full flex items-center gap-3 px-3 py-2 rounded transition-all duration-200',
              'hover:bg-cyan-500/5 hover:text-cyan-300/80',
              'font-mono text-xs uppercase tracking-wider text-cyan-500/60',
              'text-left'
            )}
            animated={['flicker']}
          >
            <span className="opacity-60">+</span>
            <Text>New Chat</Text>
          </Animated>
        </Animator>
      </Animator>
    </nav>
  );
};