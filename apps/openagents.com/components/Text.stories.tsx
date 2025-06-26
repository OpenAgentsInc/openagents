import type { Meta, StoryObj } from '@storybook/nextjs'
import { Text, Animator, Animated, cx } from '@arwes/react'
import React, { useState, useEffect } from 'react'

const meta = {
  title: 'Arwes/Text',
  component: Text,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'The Text component provides animated text rendering with two animation managers: sequence (character-by-character) and decipher (cipher effect).',
      },
    },
  },
  tags: ['autodocs'],
  argTypes: {
    manager: {
      control: 'select',
      options: ['sequence', 'decipher'],
      description: 'Animation manager type',
      defaultValue: 'sequence',
    },
    as: {
      control: 'select',
      options: ['div', 'p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'span'],
      description: 'HTML element to render as (use div to avoid nesting issues)',
      defaultValue: 'div',
    },
    fixed: {
      control: 'boolean',
      description: 'Fixed duration mode',
    },
    contentClassName: {
      control: 'text',
      description: 'CSS class for content',
    },
    contentStyle: {
      control: 'object',
      description: 'Inline styles for content',
    },
    children: {
      control: 'text',
      description: 'Text content',
    },
  },
} satisfies Meta<typeof Text>

export default meta
type Story = StoryObj<typeof meta>

// Debug story to test basic rendering
export const DebugBasic: Story = {
  args: {
    children: 'This text should be visible',
    as: 'div',
    className: 'text-white',
  },
  render: (args) => (
    <div style={{ padding: '20px', background: 'rgba(255,255,255,0.1)' }}>
      <div style={{ color: 'white', marginBottom: '10px' }}>
        Debug info: args = {JSON.stringify(args, null, 2)}
      </div>
      <Text {...args} />
    </div>
  ),
}

// Test without any animation
export const PlainText: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ padding: '20px', backgroundColor: '#222' }}>
      <Text className="text-white">Plain text without animation</Text>
      <br />
      <Text as="div" className="text-cyan-300">Cyan text as div</Text>
      <br />
      <Text as="p" className="text-yellow-300">Yellow text as p</Text>
    </div>
  ),
}

// Test font loading - Both fonts side by side
export const FontTest: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ padding: '20px', backgroundColor: '#1a1a1a' }}>
      <div className="grid grid-cols-2 gap-8">
        {/* Berkeley Mono Column */}
        <div>
          <h3 className="text-white mb-4 font-bold">Berkeley Mono (monospace)</h3>
          <div className="space-y-2">
            <Text className="font-mono text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace' }}>
              ABCDEFGHIJKLMNOPQRSTUVWXYZ
            </Text>
            <Text className="font-mono text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace' }}>
              abcdefghijklmnopqrstuvwxyz
            </Text>
            <Text className="font-mono text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace' }}>
              0123456789 !@#$%^&*()
            </Text>
            <Text className="font-mono text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace', fontWeight: 700 }}>
              Bold: The quick brown fox
            </Text>
            <Text className="font-mono text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace', fontStyle: 'italic' }}>
              Italic: jumps over the lazy dog
            </Text>
          </div>
        </div>

        {/* Titillium Web Column */}
        <div>
          <h3 className="text-white mb-4 font-bold">Titillium Web (sans-serif)</h3>
          <div className="space-y-2">
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif' }}>
              ABCDEFGHIJKLMNOPQRSTUVWXYZ
            </Text>
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif' }}>
              abcdefghijklmnopqrstuvwxyz
            </Text>
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif' }}>
              0123456789 !@#$%^&*()
            </Text>
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif', fontWeight: 300 }}>
              Light: The quick brown fox
            </Text>
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif', fontWeight: 600 }}>
              SemiBold: jumps over the
            </Text>
            <Text className="font-sans text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif', fontWeight: 700 }}>
              Bold: lazy dog
            </Text>
          </div>
        </div>
      </div>

      {/* Direct comparison */}
      <div className="mt-8 space-y-4">
        <h3 className="text-white font-bold">Direct Comparison:</h3>
        <div className="space-y-2">
          <Text className="text-cyan-300" style={{ fontFamily: 'Berkeley Mono, monospace', fontSize: '18px' }}>
            Berkeley Mono: The future of digital agents is here
          </Text>
          <Text className="text-yellow-300" style={{ fontFamily: 'Titillium Web, sans-serif', fontSize: '18px' }}>
            Titillium Web: The future of digital agents is here
          </Text>
        </div>
      </div>
    </div>
  ),
}

// Test like in actual app
export const LikeInApp: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div className="text-center text-cyan-500/40 py-16 font-mono text-sm">
      <Text>Start a conversation...</Text>
    </div>
  ),
}

// Test Text component directly
export const TextDirect: Story = {
  args: {
    children: '',
  },
  render: () => {
    const TextDebug = () => {
      const [mounted, setMounted] = React.useState(false);
      
      React.useEffect(() => {
        setMounted(true);
        console.log('TextDirect mounted');
      }, []);
      
      return (
        <div style={{ padding: '20px' }}>
          <div style={{ color: 'white', marginBottom: '10px' }}>
            Mounted: {mounted ? 'YES' : 'NO'}
          </div>
          <div style={{ border: '2px solid red', padding: '10px', marginBottom: '10px' }}>
            <span style={{ color: 'white' }}>Direct Text (no wrapper):</span>
            <br />
            <Text>Default Text component</Text>
          </div>
          <div style={{ border: '2px solid green', padding: '10px', marginBottom: '10px' }}>
            <span style={{ color: 'white' }}>Text with explicit props:</span>
            <br />
            <Text as="div" className="text-cyan-300" style={{ fontSize: '20px' }}>
              Text with props
            </Text>
          </div>
          <div style={{ border: '2px solid blue', padding: '10px' }}>
            <span style={{ color: 'white' }}>Text in Animator (no Animated):</span>
            <br />
            <Animator root active={true}>
              <Text as="div" className="text-yellow-300">
                Text in Animator
              </Text>
            </Animator>
          </div>
        </div>
      );
    };
    
    return <TextDebug />;
  },
}

// Force visible text
export const ForceVisible: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ backgroundColor: '#000', padding: '20px' }}>
      <Text style={{ color: '#fff' }}>White text on black</Text>
      <br />
      <Text style={{ color: '#0ff' }}>Cyan text on black</Text>
      <br />
      <AnimatedTextWrapper>
        <Text style={{ color: '#ff0' }}>Yellow text in wrapper</Text>
      </AnimatedTextWrapper>
      <br />
      <AnimatedTextWrapper>
        <Text as="div" manager="sequence" style={{ color: '#0f0' }}>
          Green sequence text
        </Text>
      </AnimatedTextWrapper>
      <br />
      <AnimatedTextWrapper>
        <Text as="div" manager="decipher" style={{ color: '#f0f' }}>
          MAGENTA DECIPHER TEXT
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

// Debug visibility issues
export const DebugVisibility: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ padding: '20px', backgroundColor: '#333' }}>
      <div style={{ color: 'white', marginBottom: '20px' }}>Regular HTML divs:</div>
      <div style={{ color: 'cyan' }}>This is a regular cyan div</div>
      <div className="text-cyan-300">This uses Tailwind class</div>
      
      <div style={{ color: 'white', marginTop: '20px', marginBottom: '20px' }}>Text components:</div>
      <Text style={{ color: 'yellow' }}>Text with inline style</Text>
      <br />
      <Text className="text-yellow-300">Text with Tailwind class</Text>
      
      <div style={{ color: 'white', marginTop: '20px', marginBottom: '20px' }}>Inside AnimatedTextWrapper:</div>
      <AnimatedTextWrapper>
        <div style={{ color: 'red' }}>Regular div inside wrapper</div>
        <Text style={{ color: 'green' }}>Text with inline style inside wrapper</Text>
        <br />
        <Text className="text-green-400">Text with Tailwind inside wrapper</Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

// Debug story with Animator
export const DebugWithAnimator: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ color: 'white', border: '1px solid red', padding: '20px' }}>
      <div>Outside Animator: This should be visible</div>
      <Animator root active={true}>
        <div>Inside Animator: This should also be visible</div>
        <Text as="div" className="text-cyan-300">
          Text component: This is cyan text
        </Text>
      </Animator>
    </div>
  ),
}

// Helper component for animated stories
// NOTE: For Storybook visibility, we're not using Animator wrapper
// In production, you would wrap with Animator for animations
const AnimatedTextWrapper = ({ 
  children, 
  duration = { enter: 1, exit: 0.5 }
}: { 
  children: React.ReactNode
  duration?: { enter: number, exit: number }
}) => {
  // For now, just return children directly to ensure visibility
  return <>{children}</>
}

export const Default: Story = {
  args: {
    children: 'The universe is a vast expanse of space and time.',
    as: 'div',
    className: 'text-cyan-300',
  },
  render: (args) => {
    console.log('[Default Story] args:', args);
    return (
      <div style={{ padding: '20px', backgroundColor: '#1a1a1a' }}>
        <AnimatedTextWrapper>
          <Text as={args.as} className={args.className}>
            {args.children}
          </Text>
        </AnimatedTextWrapper>
      </div>
    );
  },
}

export const SequenceAnimation: Story = {
  args: {
    manager: 'sequence',
    children: 'Characters appear one by one in sequence...',
    className: 'text-cyan-300',
    as: 'div',
  },
  render: (args) => (
    <div style={{ backgroundColor: '#222', padding: '20px' }}>
      <AnimatedTextWrapper duration={{ enter: 2, exit: 1 }}>
        <Text 
          as={args.as} 
          manager={args.manager}
          className={args.className}
          contentClassName={args.className}
          style={{ color: 'cyan' }}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const DecipherAnimation: Story = {
  args: {
    manager: 'decipher',
    children: 'CLASSIFIED INFORMATION DECRYPTING...',
    className: 'font-mono text-yellow-300',
    as: 'div',
  },
  render: (args) => (
    <div style={{ backgroundColor: '#1a1a1a', padding: '20px' }}>
      <AnimatedTextWrapper duration={{ enter: 1.5, exit: 0.5 }}>
        <Text 
          as={args.as} 
          manager={args.manager}
          className={args.className}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const HeadingStyles: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div className="space-y-4 p-6" style={{ backgroundColor: '#1a1a1a' }}>
      <AnimatedTextWrapper>
        <Text as="h1" className="text-4xl font-bold text-cyan-300">
          Heading Level 1
        </Text>
      </AnimatedTextWrapper>
      <AnimatedTextWrapper>
        <Text as="h2" className="text-3xl font-bold text-cyan-400">
          Heading Level 2
        </Text>
      </AnimatedTextWrapper>
      <AnimatedTextWrapper>
        <Text as="h3" className="text-2xl font-bold text-cyan-500">
          Heading Level 3
        </Text>
      </AnimatedTextWrapper>
      <AnimatedTextWrapper>
        <Text as="h4" className="text-xl font-bold text-cyan-600">
          Heading Level 4
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const ComplexContent: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div style={{ backgroundColor: '#1a1a1a', padding: '20px' }}>
      <AnimatedTextWrapper duration={{ enter: 3, exit: 1 }}>
        <Text as="div" fixed className="text-cyan-300 max-w-2xl">
          <h3 className="text-xl font-bold mb-2">Advanced Neural Interface</h3>
          <p className="mb-2">
            The <strong>quantum processing unit</strong> operates at 
            <abbr title="Terahertz">THz</abbr> frequencies, enabling 
            <em>real-time</em> neural pathway mapping.
          </p>
          <p>
            For more information, visit the{' '}
            <a href="#" className="text-yellow-300 underline">
              technical documentation
            </a>{' '}
            or contact the research team.
          </p>
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const SideBySideComparison: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div className="flex gap-8">
      <div className="flex-1">
        <h4 className="text-cyan-300 mb-2 font-mono text-sm">SEQUENCE</h4>
        <div style={{ backgroundColor: 'rgba(0,255,255,0.05)', padding: '10px' }}>
          <AnimatedTextWrapper>
            <Text as="div" manager="sequence" className="text-cyan-300">
              Characters appear one by one, creating a typewriter effect 
              with a blinking cursor at the end.
            </Text>
          </AnimatedTextWrapper>
        </div>
      </div>
      <div className="flex-1">
        <h4 className="text-yellow-300 mb-2 font-mono text-sm">DECIPHER</h4>
        <div style={{ backgroundColor: 'rgba(255,255,0,0.05)', padding: '10px' }}>
          <AnimatedTextWrapper>
            <Text as="div" manager="decipher" className="text-yellow-300 font-mono">
              All characters scramble and decrypt simultaneously into 
              the final message.
            </Text>
          </AnimatedTextWrapper>
        </div>
      </div>
    </div>
  ),
}

export const CustomStyling: Story = {
  args: {
    children: 'Custom styled text with inline styles',
    as: 'div',
    contentStyle: {
      background: 'linear-gradient(to right, #00ffff, #00ff00)',
      WebkitBackgroundClip: 'text',
      WebkitTextFillColor: 'transparent',
      fontSize: '24px',
      fontWeight: 'bold',
    },
  },
  render: (args) => (
    <div style={{ backgroundColor: '#000', padding: '20px' }}>
      <AnimatedTextWrapper>
        <Text 
          as={args.as}
          contentStyle={args.contentStyle}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const MonospaceCode: Story = {
  args: {
    manager: 'decipher',
    as: 'div',
    children: `function initialize() {
  console.log('System online');
  return true;
}`,
    className: 'font-mono text-green-400 whitespace-pre bg-black/50 p-4 rounded',
  },
  render: (args) => (
    <div style={{ backgroundColor: '#0a0a0a', padding: '20px' }}>
      <AnimatedTextWrapper duration={{ enter: 2, exit: 0.5 }}>
        <Text 
          as={args.as}
          manager={args.manager}
          className={args.className}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const ShortMessages: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div className="space-y-4 p-6" style={{ backgroundColor: '#0a0a0a' }}>
      <AnimatedTextWrapper>
        <Text as="div" manager="decipher" className="text-cyan-300 font-mono text-2xl">
          SYSTEM READY
        </Text>
      </AnimatedTextWrapper>
      <AnimatedTextWrapper>
        <Text as="div" manager="decipher" className="text-yellow-300 font-mono text-2xl">
          ACCESS GRANTED
        </Text>
      </AnimatedTextWrapper>
      <AnimatedTextWrapper>
        <Text as="div" manager="decipher" className="text-red-400 font-mono text-2xl">
          WARNING: LOW POWER
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const FixedVsDynamic: Story = {
  args: {
    children: '',
  },
  render: () => (
    <div className="space-y-8 p-6" style={{ backgroundColor: '#1a1a1a' }}>
      <div>
        <h4 className="text-cyan-300 mb-2 font-mono text-sm">FIXED DURATION</h4>
        <AnimatedTextWrapper>
          <Text as="div" fixed className="text-cyan-300">
            This text has a fixed animation duration regardless of content length.
            The animation time is predetermined.
          </Text>
        </AnimatedTextWrapper>
      </div>
      <div>
        <h4 className="text-yellow-300 mb-2 font-mono text-sm">DYNAMIC DURATION</h4>
        <AnimatedTextWrapper>
          <Text as="div" className="text-yellow-300">
            This text adjusts its animation duration based on the content length.
            Longer text takes more time to animate.
          </Text>
        </AnimatedTextWrapper>
      </div>
    </div>
  ),
}

export const CenteredText: Story = {
  args: {
    manager: 'decipher',
    children: 'CENTERED DISPLAY',
    className: 'text-4xl font-bold text-cyan-300 text-center',
    as: 'div',
    contentStyle: {
      textShadow: '0 0 20px rgba(0, 255, 255, 0.5)',
    },
  },
  render: (args) => (
    <div className="w-full max-w-lg mx-auto p-8" style={{ backgroundColor: '#0a0a0a' }}>
      <AnimatedTextWrapper>
        <Text 
          as={args.as}
          manager={args.manager}
          className={args.className}
          contentStyle={args.contentStyle}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}

export const StatusMessages: Story = {
  args: {
    children: '',
  },
  render: () => {
    const [messageIndex, setMessageIndex] = useState(0)
    const messages = [
      { text: 'Initializing neural network...', className: 'text-cyan-300' },
      { text: 'Loading quantum processors...', className: 'text-blue-300' },
      { text: 'Establishing secure connection...', className: 'text-green-300' },
      { text: 'System online and operational', className: 'text-yellow-300' },
    ]

    useEffect(() => {
      const interval = setInterval(() => {
        setMessageIndex((prev) => (prev + 1) % messages.length)
      }, 3000)
      return () => clearInterval(interval)
    }, [])

    return (
      <div className="w-96 p-6" style={{ backgroundColor: '#0a0a0a' }}>
        <AnimatedTextWrapper>
          <Text 
            as="div"
            manager="sequence" 
            className={cx('font-mono', messages[messageIndex].className)}
          >
            {messages[messageIndex].text}
          </Text>
        </AnimatedTextWrapper>
      </div>
    )
  },
}

export const Playground: Story = {
  args: {
    children: 'Experiment with different text animations and styles',
    manager: 'sequence',
    as: 'div',
    fixed: false,
    className: 'text-cyan-300',
  },
  render: (args) => (
    <div className="min-w-[400px] p-6" style={{ backgroundColor: '#1a1a1a' }}>
      <AnimatedTextWrapper>
        <Text 
          as={args.as}
          manager={args.manager}
          className={args.className}
          fixed={args.fixed}
        >
          {args.children}
        </Text>
      </AnimatedTextWrapper>
    </div>
  ),
}