"use client";

import React, { useState, useCallback } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { 
  type AnimatedProp, 
  Animated, 
  Animator, 
  cx, 
  Text, 
  FrameOctagon,
  styleFrameClipOctagon,
  Illuminator,
  memo
} from '@arwes/react';
import { useConvexAuth } from "convex/react";
import { ArwesLogoType } from './ArwesLogoType';
import { ArwesLogoIcon } from './ArwesLogoIcon';
import { Menu as MenuIcon, Settings, Bell, User } from 'lucide-react';
import { Menu } from './Menu';
import { MenuItem } from './MenuItem';

interface HeaderMainProps {
  className?: string;
  animated?: AnimatedProp;
  onMenuToggle?: () => void;
}

const HEIGHT_CLASS = 'h-10 md:h-12';

export const HeaderMain = memo((props: HeaderMainProps): JSX.Element => {
  const { className, animated, onMenuToggle } = props;
  const { isAuthenticated } = useConvexAuth();
  const pathname = usePathname();
  const isIndex = pathname === '/';

  return (
    <Animated
      as="header"
      className={cx('flex justify-center items-center select-none', className)}
      animated={animated}
    >
      <div className={cx('flex mx-auto p-2 w-full max-w-screen-3xl', 'md:px-4', 'xl:py-4')}>
        <div className={cx('relative flex-1 flex px-4')}>
          {/* BACKGROUND */}
          {!isIndex && (
            <Animator merge>
              <Animated
                role="presentation"
                className="absolute inset-0 overflow-hidden"
                style={{
                  clipPath: styleFrameClipOctagon({ squareSize: 8 })
                }}
                animated={['flicker']}
              >
                <FrameOctagon
                  style={{
                    '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.1)',
                    '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                  }}
                  squareSize={8}
                />
                <Illuminator
                  color="hsla(180, 69%, 25%, 0.1)"
                  size={400}
                />
              </Animated>
            </Animator>
          )}

          {/* CONTENT */}
          <div className="relative flex-1 flex flex-row justify-between items-center">
            {/* LEFT PANEL */}
            <Animator combine manager="stagger">
              <Animated className="flex flex-row gap-4" animated={[['x', 16, 0, 0]]}>
                <Link 
                  className="transition-opacity ease-out duration-200 opacity-60 hover:opacity-100"
                  href="/" 
                  onClick={() => {}}
                >
                  <h1
                    className={cx('flex flex-row justify-center items-center gap-2', HEIGHT_CLASS)}
                    title="OpenAgents"
                  >
                    <Animator>
                      <ArwesLogoIcon
                        className={cx('w-5 h-5 md:w-6 md:h-6')}
                        animated={['flicker']}
                      />
                    </Animator>

                    <Animator
                      merge
                      condition={!isIndex}
                      unmountOnExited
                      unmountOnDisabled={isIndex}
                    >
                      <ArwesLogoType className="h-3 md:h-4" animated={['flicker']} />
                    </Animator>
                  </h1>
                </Link>

                <Animator
                  combine
                  manager="stagger"
                  condition={!isIndex}
                  unmountOnExited
                  unmountOnDisabled={isIndex}
                >
                  <Menu className={HEIGHT_CLASS}>
                    <Animator>
                      <MenuItem active={pathname === '/'} animated={['flicker']}>
                        <Link href="/" title="Go to Home">
                          <span className="hidden md:block">Home</span>
                        </Link>
                      </MenuItem>
                    </Animator>
                    <Animator>
                      <MenuItem active={pathname.startsWith('/chat')} animated={['flicker']}>
                        <Link href="/chat" title="Go to Chat">
                          <span className="hidden md:block">Chat</span>
                        </Link>
                      </MenuItem>
                    </Animator>
                    <Animator>
                      <MenuItem active={pathname.startsWith('/agents')} animated={['flicker']}>
                        <Link href="/agents" title="Go to Agents">
                          <span className="hidden md:block">Agents</span>
                        </Link>
                      </MenuItem>
                    </Animator>
                    <Animator>
                      <MenuItem active={pathname.startsWith('/playground')} animated={['flicker']}>
                        <Link href="/playground" title="Go to Playground">
                          <span className="hidden md:block">Playground</span>
                        </Link>
                      </MenuItem>
                    </Animator>
                  </Menu>
                </Animator>
              </Animated>
            </Animator>

            {/* RIGHT PANEL */}
            <Animator combine manager="switch">
              <Animator
                combine
                manager="staggerReverse"
              >
                <Animated
                  as="nav"
                  className="flex flex-row gap-4"
                  animated={[['x', -8, 0, 0]]}
                >
                  <Menu className={HEIGHT_CLASS}>
                    <Animator>
                      <MenuItem animated={['flicker']}>
                        <button>
                          <Settings />
                        </button>
                      </MenuItem>
                    </Animator>
                    {onMenuToggle && (
                      <Animator>
                        <MenuItem animated={['flicker']} className="lg:hidden">
                          <button onClick={onMenuToggle}>
                            <MenuIcon />
                          </button>
                        </MenuItem>
                      </Animator>
                    )}
                  </Menu>

                  <Menu className={cx(HEIGHT_CLASS, 'hidden lg:flex')}>
                    <Animator>
                      <MenuItem animated={['flicker']}>
                        <a
                          className="normal-case"
                          href="https://github.com/OpenAgentsInc/openagents"
                          target="github"
                          title="View on GitHub"
                        >
                          v0.1.0
                        </a>
                      </MenuItem>
                    </Animator>
                  </Menu>

                  <Menu className={HEIGHT_CLASS}>
                    <Animator>
                      <MenuItem animated={['flicker']}>
                        <button
                          title={isAuthenticated ? 'Profile' : 'Sign In'}
                        >
                          {isAuthenticated ? (
                            <><User /> <span className="hidden md:block">Profile</span></>
                          ) : (
                            <Link href="/signin">
                              <span className="uppercase">Sign In</span>
                            </Link>
                          )}
                        </button>
                      </MenuItem>
                    </Animator>
                  </Menu>
                </Animated>
              </Animator>
            </Animator>
          </div>
        </div>
      </div>
    </Animated>
  );
});

HeaderMain.displayName = 'HeaderMain';