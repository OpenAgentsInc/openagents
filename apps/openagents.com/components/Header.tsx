"use client";

import { Animator } from '@arwes/react-animator';
import { Animated } from '@arwes/react-animated';
import { Text } from '@arwes/react-text';
import Link from 'next/link';
import { useState } from 'react';
import { useConvexAuth } from "convex/react";

export function Header() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const { isAuthenticated } = useConvexAuth();

  const navItems = [
    { href: '/', label: 'Home' },
    { href: '/docs', label: 'Docs' },
    { href: '/playground', label: 'Playground' },
    { href: '/pricing', label: 'Pricing' },
    { href: isAuthenticated ? '/dashboard' : '/signin', label: isAuthenticated ? 'Dashboard' : 'Sign In' },
  ];

  return (
    <header className="fixed top-0 left-0 right-0 z-50 bg-black/90 backdrop-blur-md border-b border-cyan-500/30">
      <nav className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Logo */}
          <Link href="/" className="flex items-center space-x-2">
            <Animated>
              <div className="text-2xl font-bold font-sans text-cyan-500 tracking-wider">
                <Text>OPENAGENTS</Text>
              </div>
            </Animated>
          </Link>

          {/* Desktop Navigation */}
          <div className="hidden md:flex items-center space-x-8">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="relative text-cyan-500 hover:text-cyan-300 transition-colors duration-200 font-mono text-sm uppercase tracking-wider"
              >
                <Animator active>
                  <Animated>
                    <Text>{item.label}</Text>
                  </Animated>
                </Animator>
              </Link>
            ))}
          </div>

          {/* Mobile menu button */}
          <button
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            className="md:hidden text-cyan-500 hover:text-cyan-300 transition-colors duration-200"
          >
            <svg className="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              {mobileMenuOpen ? (
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              ) : (
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
              )}
            </svg>
          </button>
        </div>
      </nav>

      {/* Mobile Navigation */}
      {mobileMenuOpen && (
        <div className="md:hidden bg-black/95 backdrop-blur-md border-t border-cyan-500/30">
          <div className="px-2 pt-2 pb-3 space-y-1">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                onClick={() => setMobileMenuOpen(false)}
                className="block px-3 py-2 text-cyan-500 hover:text-cyan-300 hover:bg-cyan-500/10 transition-colors duration-200 font-mono text-sm uppercase tracking-wider"
              >
                <Text>{item.label}</Text>
              </Link>
            ))}
          </div>
        </div>
      )}
    </header>
  );
}