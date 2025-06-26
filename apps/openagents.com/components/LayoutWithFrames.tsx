"use client";

import React, { type ReactNode, useState, useEffect, useRef, Children } from 'react';
import { usePathname } from 'next/navigation';
import Link from 'next/link';
import {
  Animated,
  FrameOctagon,
  Illuminator,
  styleFrameClipOctagon,
  memo,
  cx,
  Animator,
  BleepsOnAnimator,
  Text
} from '@arwes/react';
import { ArwesLogoIcon } from './ArwesLogoIcon';
import { ArwesLogoType } from './ArwesLogoType';
import { Menu } from './Menu';
import { MenuItem } from './MenuItem';
import { NavSidebar } from './NavSidebar';
import { Menu as MenuIcon } from 'lucide-react';

interface LayoutWithFramesProps {
  children: ReactNode;
  className?: string;
}

const CONTAINER_WIDTH_CLASS = 'w-full min-w-0 max-w-screen-3xl min-h-0';
const ASIDE_WIDTH_CLASS = 'w-full min-w-0 max-w-[16rem] min-h-0';
const MAIN_WIDTH_CLASS = 'w-full min-w-0 max-w-[50rem] min-h-0';
const HEIGHT_CLASS = 'h-10 md:h-12';

export const LayoutWithFrames = memo((props: LayoutWithFramesProps): React.ReactElement => {
  const { className, children } = props;
  const pathname = usePathname();
  const overflowElementRef = useRef<HTMLDivElement>(null);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  
  // For simplicity, assuming lg/xl breakpoints
  const isLG = true; // In real app, use useAppBreakpoint('lg')
  const isXL = false; // In real app, use useAppBreakpoint('xl')

  useEffect(() => {
    const overflowElement = overflowElementRef.current;
    if (overflowElement) {
      overflowElement.scrollTop = 0;
    }
  }, [pathname]);

  return (
    <>
      {/* Header with FrameOctagon */}
      <Animator combine>
        <header className="relative flex justify-center items-center select-none">
          <div className={cx('flex mx-auto p-2 w-full', 'md:px-4', 'xl:py-4', CONTAINER_WIDTH_CLASS)}>
            <div className={cx('relative flex-1 flex px-4')}>
              {/* Header Background Frame */}
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
                      // @ts-expect-error CSS variables
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

              {/* Header Content */}
              <div className="relative flex-1 flex flex-row justify-between items-center">
                <Animator combine manager="stagger">
                  <Animated className="flex flex-row gap-4" animated={[['x', 16, 0, 0]]}>
                    <Link 
                      className="transition-opacity ease-out duration-200 opacity-60 hover:opacity-100"
                      href="/"
                    >
                      <h1 className={cx('flex flex-row justify-center items-center gap-2', HEIGHT_CLASS)}>
                        <Animator>
                          <ArwesLogoIcon
                            className={cx('w-5 h-5 md:w-6 md:h-6')}
                            animated={['flicker']}
                          />
                        </Animator>
                        <Animator>
                          <ArwesLogoType className="h-3 md:h-4" animated={['flicker']} />
                        </Animator>
                      </h1>
                    </Link>

                    <Menu className={HEIGHT_CLASS}>
                      <Animator>
                        <MenuItem active={pathname === '/'} animated={['flicker']}>
                          <Link href="/">Home</Link>
                        </MenuItem>
                      </Animator>
                      <Animator>
                        <MenuItem active={pathname.startsWith('/chat')} animated={['flicker']}>
                          <Link href="/chat">Chat</Link>
                        </MenuItem>
                      </Animator>
                      <Animator>
                        <MenuItem active={pathname.startsWith('/agents')} animated={['flicker']}>
                          <Link href="/agents">Agents</Link>
                        </MenuItem>
                      </Animator>
                    </Menu>
                  </Animated>
                </Animator>

                {/* Mobile menu button */}
                <button
                  onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                  className="lg:hidden text-cyan-500 hover:text-cyan-300 transition-colors"
                >
                  <MenuIcon size={24} />
                </button>
              </div>
            </div>
          </div>
        </header>
      </Animator>

      {/* Main Content Area */}
      <Animator combine>
        <BleepsOnAnimator transitions={{ entering: 'type' }} />

        <div className={cx('relative flex-1 flex flex-col justify-start items-center w-full min-w-0 min-h-0', className)}>
          {/* Background elements with frames */}
          <Animator duration={{ delay: isLG ? 0.2 : 0 }}>
            <div
              role="presentation"
              className={cx('absolute inset-0 flex px-2 pb-2', 'md:px-4 md:pb-4')}
            >
              <div
                className={cx(
                  'flex flex-row justify-center gap-4 mx-auto',
                  'lg:justify-between',
                  CONTAINER_WIDTH_CLASS
                )}
              >
                {/* Left sidebar space */}
                {isLG && <aside className={cx('relative', ASIDE_WIDTH_CLASS)} />}

                {/* Main content background frame */}
                <main className="flex justify-center w-full">
                  <Animated
                    className={cx('relative overflow-hidden mx-auto', MAIN_WIDTH_CLASS)}
                    style={{
                      clipPath: styleFrameClipOctagon({ squareSize: isLG ? 16 : 8 })
                    }}
                    animated={['flicker']}
                  >
                    <FrameOctagon
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.15)',
                        '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                      }}
                      squareSize={isLG ? 16 : 8}
                    />
                    {isXL && (
                      <Illuminator
                        color="hsla(180, 69%, 25%, 0.1)"
                        size={400}
                      />
                    )}
                  </Animated>
                </main>

                {/* Right sidebar space */}
                {isXL && <aside className={cx('relative', ASIDE_WIDTH_CLASS)} />}
              </div>
            </div>
          </Animator>

          {/* Content elements */}
          <div
            className={cx(
              'relative flex-1 flex justify-center pb-2 w-full min-w-0 min-h-0',
              'md:pb-4'
            )}
          >
            <div
              ref={overflowElementRef}
              className={cx(
                'flex-1 overflow-y-auto flex justify-center gap-4 px-2',
                'md:px-4',
                CONTAINER_WIDTH_CLASS
              )}
            >
              {/* Left Sidebar with Frame */}
              {isLG && (
                <aside className={cx('sticky top-0 flex', ASIDE_WIDTH_CLASS)}>
                  <Animator>
                    <Animated className="relative flex w-full" animated={['flicker']}>
                      <FrameOctagon
                        style={{
                          // @ts-expect-error css variables
                          '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.1)',
                          '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
                        }}
                        squareSize={8}
                      />
                      {isXL && (
                        <div className="absolute inset-0 overflow-hidden">
                          <Illuminator
                            color="hsla(180, 69%, 25%, 0.1)"
                            size={400}
                          />
                        </div>
                      )}
                      <div className="relative overflow-y-auto flex p-4 w-full">
                        <NavSidebar />
                      </div>
                    </Animated>
                  </Animator>
                </aside>
              )}

              {/* Main Content */}
              <Animator
                combine
                manager="stagger"
                duration={{ delay: isLG ? 0.2 : 0, stagger: 0.05, limit: 25 }}
              >
                <main className="flex justify-center w-full min-w-0 min-h-0">
                  <div
                    className={cx(
                      'flex flex-col mx-auto mb-auto p-4',
                      'md:p-8',
                      'xl:p-12',
                      MAIN_WIDTH_CLASS
                    )}
                  >
                    {children}
                  </div>
                </main>
              </Animator>

              {/* Right Sidebar (if needed) */}
              {isXL && (
                <aside className={cx('sticky top-0 flex', ASIDE_WIDTH_CLASS)}>
                  {/* Empty for now */}
                </aside>
              )}
            </div>
          </div>
        </div>
      </Animator>

      {/* Mobile Menu Overlay */}
      {mobileMenuOpen && (
        <div className="lg:hidden fixed inset-0 z-50 bg-black/95 backdrop-blur-md">
          <div className="flex flex-col h-full">
            <div className="flex items-center justify-between p-4 border-b border-cyan-500/30">
              <h2 className="text-cyan-500 font-mono text-sm uppercase tracking-wider">Menu</h2>
              <button
                onClick={() => setMobileMenuOpen(false)}
                className="text-cyan-500 hover:text-cyan-300 transition-colors"
              >
                <svg className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4">
              <NavSidebar />
            </div>
          </div>
        </div>
      )}
    </>
  );
});

(LayoutWithFrames as any).displayName = 'LayoutWithFrames';