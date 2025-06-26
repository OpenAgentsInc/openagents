"use client";

import React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cx, Text } from '@arwes/react';
import { Home, MessageSquare, Settings, FileText, Users, Zap, Plus } from 'lucide-react';

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
  const pathname = usePathname() || '/';

  return (
    <nav className="w-full h-full flex flex-col">
      {/* New Chat at top */}
      <div className="mb-8">
        <button
          className={cx(
            'w-full flex items-center gap-3 px-3 py-2.5',
            'text-cyan-300 hover:text-cyan-100',
            'font-mono text-sm uppercase tracking-wider',
            'transition-all duration-200',
            'cursor-pointer'
          )}
        >
          <Plus size={18} className="text-cyan-300" />
          <Text>New Chat</Text>
        </button>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Navigation at bottom */}
      <div className="space-y-1">
        {navItems.map((item) => {
          const isActive = pathname === item.href || 
                          (item.href !== '/' && pathname.startsWith(item.href));
          
          return (
            <Link 
              key={item.href} 
              href={item.href}
              className={cx(
                'flex items-center gap-3 px-3 py-2',
                'font-mono text-xs uppercase tracking-wider',
                'transition-all duration-200',
                isActive 
                  ? 'text-cyan-300' 
                  : 'text-cyan-500/60 hover:text-cyan-300'
              )}
            >
              <span className={cx(
                'flex-shrink-0',
                isActive ? 'text-cyan-300' : 'text-cyan-500/60'
              )}>
                {item.icon}
              </span>
              <Text>{item.label}</Text>
            </Link>
          );
        })}
      </div>
    </nav>
  );
};