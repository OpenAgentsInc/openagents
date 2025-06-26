"use client";

import React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cx, Text } from '@arwes/react';
import { Home, MessageSquare, Settings, FileText, Users, Zap } from 'lucide-react';

interface NavItem {
  href: string;
  label: string;
  icon: React.ReactNode;
}

const navItems: NavItem[] = [
  { href: '/', label: 'Home', icon: <Home size={16} /> },
  { href: '/chat', label: 'Chat', icon: <MessageSquare size={16} /> },
  { href: '/agents', label: 'Agents', icon: <Users size={16} /> },
  { href: '/playground', label: 'Playground', icon: <Zap size={16} /> },
  { href: '/docs', label: 'Documentation', icon: <FileText size={16} /> },
  { href: '/settings', label: 'Settings', icon: <Settings size={16} /> },
];

export const NavSidebar = (): React.ReactElement => {
  const pathname = usePathname();

  return (
    <nav className="w-full">
      <div className="space-y-1">
        {navItems.map((item) => {
          const isActive = pathname === item.href || 
                          (item.href !== '/' && pathname.startsWith(item.href));
          
          return (
            <Link 
              key={item.href} 
              href={item.href}
              className={cx(
                'relative flex items-center gap-3 px-3 py-2.5 w-full',
                'border border-cyan-500/20',
                'font-mono text-xs uppercase tracking-wider',
                'transition-all duration-200',
                isActive 
                  ? 'bg-cyan-500/10 text-cyan-300 border-cyan-500/40' 
                  : 'text-cyan-500/80 hover:bg-cyan-500/10 hover:text-cyan-300 hover:border-cyan-500/40'
              )}
            >
              <span className={cx(
                'flex-shrink-0',
                isActive ? 'text-cyan-300' : 'text-cyan-500/60'
              )}>
                {item.icon}
              </span>
              <Text>{item.label}</Text>
              {isActive && (
                <div className="absolute right-0 top-0 bottom-0 w-0.5 bg-cyan-300" />
              )}
            </Link>
          );
        })}
      </div>

      {/* Separator */}
      <div className="my-6 border-t border-cyan-500/20" />

      {/* Quick Actions */}
      <div className="space-y-1">
        <button
          className={cx(
            'w-full flex items-center gap-3 px-3 py-2.5',
            'border border-dashed border-cyan-500/30',
            'hover:bg-cyan-500/10 hover:text-cyan-300 hover:border-cyan-500/50',
            'font-mono text-xs uppercase tracking-wider text-cyan-500/80',
            'text-left transition-all duration-200'
          )}
        >
          <span className="text-cyan-500/60 text-lg flex-shrink-0">+</span>
          <Text>New Chat</Text>
        </button>
      </div>
    </nav>
  );
};