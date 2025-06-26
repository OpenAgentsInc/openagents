import type { Meta, StoryObj } from '@storybook/nextjs'
import { Text, Animator, Animated, AnimatorGeneralProvider } from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Arwes/Text Animated',
  component: Text,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Working Text animations based on Arwes play examples',
      },
    },
  },
} satisfies Meta<typeof Text>

export default meta
type Story = StoryObj<typeof meta>

// Simple working animation - based on Arwes play examples
export const SimpleAnimation: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    
    useEffect(() => {
      // Start animation after mount
      const timer = setTimeout(() => setActive(true), 500);
      return () => clearTimeout(timer);
    }, []);
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a', minWidth: '600px' }}>
        <div style={{ marginBottom: '20px' }}>
          <button 
            onClick={() => setActive(!active)}
            style={{
              padding: '10px 20px',
              backgroundColor: '#1a1a1a',
              color: '#0ff',
              border: '1px solid #0ff',
              cursor: 'pointer',
              fontFamily: 'Berkeley Mono, monospace'
            }}
          >
            Toggle Animation (active: {active ? 'YES' : 'NO'})
          </button>
        </div>
        
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.5 }}>
          <Animator active={active}>
            <div style={{ marginBottom: '20px' }}>
              <Text className="text-cyan-300" style={{ fontSize: '24px' }}>
                Default sequence animation
              </Text>
            </div>
            
            <div style={{ marginBottom: '20px' }}>
              <Text manager="decipher" className="text-yellow-300 font-mono" style={{ fontSize: '20px' }}>
                DECIPHER ANIMATION EFFECT
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}

// Animated wrapper example
export const WithAnimatedWrapper: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300);
      return () => clearTimeout(timer);
    }, []);
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a' }}>
        <AnimatorGeneralProvider>
          <Animator active={active}>
            <Animated animated={['fade']} style={{ marginBottom: '20px' }}>
              <Text className="text-cyan-300 text-2xl">
                Fade in animation
              </Text>
            </Animated>
            
            <Animated animated={[['x', -20, 0]]} style={{ marginBottom: '20px' }}>
              <Text className="text-yellow-300 text-2xl">
                Slide from left
              </Text>
            </Animated>
            
            <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]} style={{ marginBottom: '20px' }}>
              <Text className="text-green-400 text-2xl">
                Slide from bottom with fade
              </Text>
            </Animated>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}

// Stagger animation
export const StaggeredText: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 300);
      return () => clearTimeout(timer);
    }, []);
    
    const items = ['SYSTEM', 'ONLINE', 'READY', 'FOR', 'INPUT'];
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a' }}>
        <AnimatorGeneralProvider duration={{ enter: 0.3, stagger: 0.1 }}>
          <Animator active={active} manager="stagger">
            {items.map((item, index) => (
              <Animator key={index}>
                <div style={{ marginBottom: '10px' }}>
                  <Text 
                    manager="decipher" 
                    className="text-cyan-300 font-mono text-3xl"
                  >
                    {item}
                  </Text>
                </div>
              </Animator>
            ))}
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}

// Fixed duration example
export const FixedDuration: Story = {
  render: () => {
    const [active, setActive] = useState(true);
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a' }}>
        <button 
          onClick={() => setActive(!active)}
          style={{
            marginBottom: '20px',
            padding: '10px 20px',
            backgroundColor: '#1a1a1a',
            color: '#0ff',
            border: '1px solid #0ff',
            cursor: 'pointer',
            fontFamily: 'Berkeley Mono, monospace'
          }}
        >
          Toggle Animation
        </button>
        
        <AnimatorGeneralProvider>
          <Animator active={active} duration={{ enter: 2, exit: 1 }}>
            <div style={{ marginBottom: '20px' }}>
              <Text fixed className="text-cyan-300">
                Fixed duration (2s enter, 1s exit) - short text
              </Text>
            </div>
            
            <div>
              <Text fixed className="text-yellow-300">
                Fixed duration (2s enter, 1s exit) - This is a much longer text that would normally take more time to animate character by character, but with fixed duration it completes in the same time as the short text above.
              </Text>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}

// Real world example
export const RealWorldExample: Story = {
  render: () => {
    const [active, setActive] = useState(false);
    const [message, setMessage] = useState(0);
    
    const messages = [
      'Initializing quantum processors...',
      'Establishing neural link...',
      'Synchronizing with mainframe...',
      'System ready for input'
    ];
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 500);
      return () => clearTimeout(timer);
    }, []);
    
    useEffect(() => {
      const interval = setInterval(() => {
        setMessage(prev => (prev + 1) % messages.length);
      }, 3000);
      return () => clearInterval(interval);
    }, []);
    
    return (
      <div style={{ padding: '40px', backgroundColor: '#0a0a0a', minWidth: '500px' }}>
        <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.3 }}>
          <Animator active={active}>
            <div style={{ marginBottom: '30px' }}>
              <Text 
                as="h1" 
                className="text-4xl font-bold text-cyan-300" 
                style={{ fontFamily: 'Titillium Web, sans-serif' }}
              >
                OPENAGENTS SYSTEM
              </Text>
            </div>
            
            <Animated animated={[['opacity', 0.5, 1]]}>
              <div style={{ 
                padding: '20px', 
                border: '1px solid rgba(0, 255, 255, 0.3)',
                backgroundColor: 'rgba(0, 255, 255, 0.05)',
                marginBottom: '20px'
              }}>
                <Text 
                  manager="sequence" 
                  className="text-cyan-300 font-mono"
                  key={message} // Force re-render for animation
                >
                  {messages[message]}
                </Text>
              </div>
            </Animated>
            
            <Animated animated={[['y', 10, 0], ['opacity', 0, 1]]}>
              <div style={{ display: 'flex', gap: '20px', marginTop: '30px' }}>
                <button style={{
                  padding: '10px 20px',
                  backgroundColor: 'transparent',
                  color: '#0ff',
                  border: '1px solid #0ff',
                  cursor: 'pointer',
                  fontFamily: 'Berkeley Mono, monospace'
                }}>
                  <Text manager="decipher">CONNECT</Text>
                </button>
                <button style={{
                  padding: '10px 20px',
                  backgroundColor: 'transparent',
                  color: '#ff0',
                  border: '1px solid #ff0',
                  cursor: 'pointer',
                  fontFamily: 'Berkeley Mono, monospace'
                }}>
                  <Text manager="decipher">CONFIGURE</Text>
                </button>
              </div>
            </Animated>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    );
  },
}