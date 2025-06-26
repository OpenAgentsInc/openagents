"use client";

import React from 'react';
import Link from 'next/link';
import { Animated, Animator, cx, Text, FrameOctagon } from '@arwes/react';
import { useConvexAuth } from "convex/react";
import { ArwesLogoType } from './ArwesLogoType';
import { Menu, Search, Bell, User } from 'lucide-react';

interface HeaderMainProps {
  className?: string;
  onMenuToggle?: () => void;
}

export const HeaderMain = (props: HeaderMainProps): JSX.Element => {
  const { className, onMenuToggle } = props;
  const { isAuthenticated } = useConvexAuth();

  return (
    <header className={cx('relative border-b border-cyan-500/30 bg-black/80 backdrop-blur-md', className)}>
      <div className="relative">
        {/* Background Frame */}
        <Animator>
          <Animated
            role="presentation"
            className="absolute inset-0 overflow-hidden opacity-30"
            animated={['flicker']}
          >
            <FrameOctagon />
          </Animated>
        </Animator>

        {/* Content */}
        <div className="relative flex items-center justify-between h-16 px-4 lg:px-8">
          {/* Left Section */}
          <div className="flex items-center gap-4">
            {/* Mobile Menu Toggle */}
            <button
              onClick={onMenuToggle}
              className="lg:hidden text-cyan-500 hover:text-cyan-300 transition-colors"
            >
              <Menu size={24} />
            </button>

            {/* Logo */}
            <Link href="/" className="flex items-center">
              <Animator>
                <ArwesLogoType className="text-xl" />
              </Animator>
            </Link>
          </div>

          {/* Center Section - Search */}
          <div className="hidden md:flex flex-1 max-w-xl mx-8">
            <Animator>
              <Animated
                className={cx(
                  'relative w-full group',
                  'border border-cyan-500/30 rounded',
                  'hover:border-cyan-500/50 transition-colors'
                )}
                animated={['flicker']}
              >
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-cyan-500/50" size={18} />
                <input
                  type="search"
                  placeholder="Search agents, docs, commands..."
                  className={cx(
                    'w-full py-2 pl-10 pr-4 bg-transparent',
                    'text-cyan-300 placeholder-cyan-500/50',
                    'font-mono text-sm',
                    'focus:outline-none focus:ring-1 focus:ring-cyan-500/50 rounded'
                  )}
                />
              </Animated>
            </Animator>
          </div>

          {/* Right Section */}
          <div className="flex items-center gap-4">
            <Animator combine manager="stagger">
              {/* Notifications */}
              <Animator>
                <Animated
                  as="button"
                  className="relative text-cyan-500 hover:text-cyan-300 transition-colors"
                  animated={['flicker']}
                >
                  <Bell size={20} />
                  <span className="absolute -top-1 -right-1 w-2 h-2 bg-cyan-500 rounded-full animate-pulse" />
                </Animated>
              </Animator>

              {/* User Menu */}
              <Animator>
                {isAuthenticated ? (
                  <Animated
                    as="button"
                    className={cx(
                      'flex items-center gap-2 px-3 py-1.5 rounded',
                      'border border-cyan-500/30 hover:border-cyan-500/50',
                      'text-cyan-500 hover:text-cyan-300 transition-all'
                    )}
                    animated={['flicker']}
                  >
                    <User size={18} />
                    <Text className="hidden sm:block font-mono text-sm">Profile</Text>
                  </Animated>
                ) : (
                  <Link href="/signin">
                    <Animated
                      className={cx(
                        'flex items-center gap-2 px-3 py-1.5 rounded',
                        'border border-cyan-500/30 hover:border-cyan-500/50',
                        'text-cyan-500 hover:text-cyan-300 transition-all'
                      )}
                      animated={['flicker']}
                    >
                      <Text className="font-mono text-sm uppercase">Sign In</Text>
                    </Animated>
                  </Link>
                )}
              </Animator>
            </Animator>
          </div>
        </div>
      </div>
    </header>
  );
};