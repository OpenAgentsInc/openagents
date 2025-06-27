import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { CopyButton } from '../atoms/CopyButton.stories'

// Language icon components
const JSIcon = ({ className }: { className?: string }) => (
  <span className={cx('font-bold', className)}>JS</span>
)

const TSIcon = ({ className }: { className?: string }) => (
  <span className={cx('font-bold', className)}>TS</span>
)

const HTMLIcon = ({ className }: { className?: string }) => (
  <span className={cx('font-bold', className)}>HTML</span>
)

const CSSIcon = ({ className }: { className?: string }) => (
  <span className={cx('font-bold', className)}>CSS</span>
)

const PythonIcon = ({ className }: { className?: string }) => (
  <span className={cx('font-bold', className)}>PY</span>
)

const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

// Simplified syntax highlighting
const highlightCode = (code: string, language: string): string => {
  // This is a very basic syntax highlighter for demo purposes
  // In production, use a proper syntax highlighting library
  let highlighted = code
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')

  if (language === 'javascript' || language === 'typescript') {
    highlighted = highlighted
      .replace(/\b(const|let|var|function|return|if|else|for|while|class|import|export|from|default)\b/g, '<span class="text-purple-400">$1</span>')
      .replace(/\b(true|false|null|undefined)\b/g, '<span class="text-cyan-400">$1</span>')
      .replace(/(['"])([^'"]*?)\1/g, '<span class="text-green-400">$1$2$1</span>')
      .replace(/\/\/(.*)$/gm, '<span class="text-gray-500">//$1</span>')
      .replace(/\b(\d+)\b/g, '<span class="text-orange-400">$1</span>')
  } else if (language === 'html') {
    highlighted = highlighted
      .replace(/(&lt;\/?)(\w+)([^&]*?)(&gt;)/g, '<span class="text-gray-500">$1</span><span class="text-red-400">$2</span>$3<span class="text-gray-500">$4</span>')
      .replace(/(\w+)=(["'])([^"']*?)\2/g, '<span class="text-yellow-400">$1</span>=<span class="text-green-400">$2$3$2</span>')
  } else if (language === 'css') {
    highlighted = highlighted
      .replace(/([.#]?[\w-]+)\s*{/g, '<span class="text-yellow-400">$1</span> {')
      .replace(/([\w-]+):\s*([^;]+);/g, '<span class="text-cyan-400">$1</span>: <span class="text-green-400">$2</span>;')
  }

  return highlighted
}

// CodeBlock component
export interface CodeBlockProps {
  code: string
  language?: string
  title?: string
  showLineNumbers?: boolean
  showCopyButton?: boolean
  highlightLines?: number[]
  maxHeight?: number
  wrap?: boolean
  animated?: boolean
  className?: string
  onCopy?: (code: string) => void
}

export const CodeBlock = ({
  code = '',
  language = 'javascript',
  title,
  showLineNumbers = true,
  showCopyButton = true,
  highlightLines = [],
  maxHeight,
  wrap = false,
  animated = true,
  className = '',
  onCopy
}: CodeBlockProps) => {
  const [active, setActive] = useState(false)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 150)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const languageConfig: Record<string, { icon: any; color: string; name: string }> = {
    javascript: { icon: JSIcon, color: 'text-yellow-400', name: 'JavaScript' },
    typescript: { icon: TSIcon, color: 'text-blue-400', name: 'TypeScript' },
    html: { icon: HTMLIcon, color: 'text-orange-400', name: 'HTML' },
    css: { icon: CSSIcon, color: 'text-cyan-400', name: 'CSS' },
    python: { icon: PythonIcon, color: 'text-green-400', name: 'Python' },
    text: { icon: CodeIcon, color: 'text-gray-400', name: 'Plain Text' }
  }

  const config = languageConfig[language] || languageConfig.text
  const Icon = config.icon

  const safeCode = code || ''
  const lines = safeCode.split('\n')
  const highlightedCode = highlightCode(safeCode, language)
  const highlightedLines = highlightedCode.split('\n')

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(safeCode)
      setCopied(true)
      onCopy?.(safeCode)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy:', err)
    }
  }

  const codeContent = (
    <div
      className={cx(
        'relative rounded-lg border border-cyan-500/30 bg-black/50 overflow-hidden',
        className
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-cyan-500/20 bg-black/30">
        <div className="flex items-center gap-2">
          <Icon className={cx('text-sm', config.color)} />
          <span className="text-sm text-gray-400 font-sans">
            {title || config.name}
          </span>
        </div>
        {showCopyButton && (
          <CopyButton
            text={safeCode}
            variant="icon"
            size="small"
            animated={false}
            onCopy={handleCopy}
          />
        )}
      </div>

      {/* Code Content */}
      <div
        className={cx(
          'overflow-auto',
          maxHeight && maxHeight > 0 ? `max-h-[${maxHeight}px]` : ''
        )}
        style={maxHeight ? { maxHeight: `${maxHeight}px` } : undefined}
      >
        <table className="w-full">
          <tbody>
            {highlightedLines.map((line, index) => {
              const lineNumber = index + 1
              const isHighlighted = highlightLines.includes(lineNumber)
              
              return (
                <tr
                  key={index}
                  className={cx(
                    'group',
                    isHighlighted && 'bg-cyan-500/10'
                  )}
                >
                  {showLineNumbers && (
                    <td
                      className={cx(
                        'select-none px-4 py-0 text-right align-top',
                        'text-gray-600 text-sm font-mono',
                        'border-r border-gray-800'
                      )}
                      style={{ minWidth: '3rem' }}
                    >
                      {lineNumber}
                    </td>
                  )}
                  <td className="px-4 py-0 w-full">
                    <pre
                      className={cx(
                        'text-sm font-mono',
                        wrap ? 'whitespace-pre-wrap' : 'whitespace-pre',
                        'text-gray-200'
                      )}
                      dangerouslySetInnerHTML={{ __html: line || '&nbsp;' }}
                    />
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )

  if (!animated) {
    return codeContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          {codeContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/CodeBlock',
  component: CodeBlock,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Syntax-highlighted code display component with line numbers, copy functionality, and line highlighting support.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    code: {
      control: 'text',
      description: 'Code content to display'
    },
    language: {
      control: 'select',
      options: ['javascript', 'typescript', 'html', 'css', 'python', 'text'],
      description: 'Programming language for syntax highlighting'
    },
    title: {
      control: 'text',
      description: 'Optional title for the code block'
    },
    showLineNumbers: {
      control: 'boolean',
      description: 'Show line numbers'
    },
    showCopyButton: {
      control: 'boolean',
      description: 'Show copy button'
    },
    highlightLines: {
      control: 'object',
      description: 'Array of line numbers to highlight'
    },
    maxHeight: {
      control: 'number',
      description: 'Maximum height in pixels before scrolling'
    },
    wrap: {
      control: 'boolean',
      description: 'Wrap long lines'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof CodeBlock>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    code: `function bitcoinPuns() {
  const puns = [
    "I'm hodling on to my Bitcoin!",
    "Don't be coin-fused about crypto",
    "Satoshi Naka-motto: To the moon!"
  ];
  
  return puns.map(pun => console.log(pun));
}`,
    language: 'javascript'
  }
}

export const HTMLExample: Story = {
  args: {
    code: `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Bitcoin Puns</title>
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <h1 class="title">Welcome to Bitcoin Puns!</h1>
  <div id="pun-container">
    <p class="pun">Loading puns...</p>
  </div>
  <script src="app.js"></script>
</body>
</html>`,
    language: 'html',
    title: 'index.html'
  }
}

export const CSSExample: Story = {
  args: {
    code: `/* Bitcoin Puns Styles */
.title {
  color: #00d4ff;
  font-size: 3rem;
  text-align: center;
  text-shadow: 0 0 20px rgba(0, 212, 255, 0.5);
}

#pun-container {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
  background: rgba(0, 0, 0, 0.8);
  border: 1px solid #00d4ff;
  border-radius: 8px;
}

.pun {
  font-size: 1.2rem;
  line-height: 1.6;
  color: #ffffff;
  margin-bottom: 1rem;
}`,
    language: 'css',
    title: 'styles.css'
  }
}

export const TypeScriptExample: Story = {
  args: {
    code: `interface BitcoinPun {
  id: string;
  text: string;
  category: 'wordplay' | 'technical' | 'price';
  rating: number;
}

class PunGenerator {
  private puns: BitcoinPun[] = [];
  
  constructor() {
    this.loadPuns();
  }
  
  async loadPuns(): Promise<void> {
    const response = await fetch('/api/puns');
    this.puns = await response.json();
  }
  
  getRandomPun(): BitcoinPun | null {
    if (this.puns.length === 0) return null;
    const index = Math.floor(Math.random() * this.puns.length);
    return this.puns[index];
  }
}`,
    language: 'typescript',
    title: 'PunGenerator.ts'
  }
}

export const WithHighlightedLines: Story = {
  args: {
    code: `function deployToCloudflare(projectName) {
  // Validate project name
  if (!projectName) {
    throw new Error('Project name is required');
  }
  
  // Build the project
  console.log('Building project...');
  const buildResult = buildProject();
  
  // Deploy to Cloudflare Workers
  console.log('Deploying to Cloudflare...');
  const deployment = cloudflare.deploy({
    name: projectName,
    files: buildResult.files,
    routes: ['/*']
  });
  
  return deployment.url;
}`,
    language: 'javascript',
    highlightLines: [3, 4, 13, 14, 15]
  }
}

export const LongCode: Story = {
  args: {
    code: `// Bitcoin Puns Website Generator
import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';

const BitcoinPunsApp = () => {
  const [puns, setPuns] = useState([]);
  const [currentPun, setCurrentPun] = useState(null);
  const [loading, setLoading] = useState(true);
  
  const bitcoinPuns = [
    { id: 1, text: "I'm not coin-ing around!", category: 'wordplay' },
    { id: 2, text: "HODL on tight!", category: 'technical' },
    { id: 3, text: "To the moon! 🚀", category: 'price' },
    { id: 4, text: "Don't go bacon my heart, Bitcoin", category: 'wordplay' },
    { id: 5, text: "Satoshi Naka-motto: Never give up!", category: 'wordplay' },
    { id: 6, text: "I'm having a bit of a coin-undrum", category: 'wordplay' },
    { id: 7, text: "Mining my own business", category: 'technical' },
    { id: 8, text: "Fork it, let's go!", category: 'technical' },
    { id: 9, text: "Proof of Pun consensus achieved", category: 'technical' },
    { id: 10, text: "My portfolio is bit-ter sweet", category: 'price' }
  ];
  
  useEffect(() => {
    // Simulate loading
    setTimeout(() => {
      setPuns(bitcoinPuns);
      setCurrentPun(bitcoinPuns[0]);
      setLoading(false);
    }, 1000);
  }, []);
  
  const getRandomPun = () => {
    const randomIndex = Math.floor(Math.random() * puns.length);
    setCurrentPun(puns[randomIndex]);
  };
  
  if (loading) {
    return <div className="loader">Mining for puns...</div>;
  }
  
  return (
    <div className="app">
      <h1>Bitcoin Puns Generator</h1>
      <motion.div
        className="pun-display"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
      >
        <p className="pun-text">{currentPun.text}</p>
        <span className="pun-category">{currentPun.category}</span>
      </motion.div>
      <button onClick={getRandomPun} className="generate-btn">
        Generate New Pun
      </button>
    </div>
  );
};

export default BitcoinPunsApp;`,
    language: 'javascript',
    title: 'BitcoinPunsApp.jsx',
    maxHeight: 400
  }
}

export const NoLineNumbers: Story = {
  args: {
    code: `// Simple one-liner
console.log("Bitcoin to the moon! 🚀");`,
    language: 'javascript',
    showLineNumbers: false
  }
}

export const PlainText: Story = {
  args: {
    code: `Bitcoin Puns Collection:

1. I'm hodling on for dear life!
2. Don't be so coin-descending
3. That's a bit coin-cidental
4. Stop mining your own business
5. Fork this, I'm going home`,
    language: 'text',
    title: 'puns.txt',
    showLineNumbers: false
  }
}

export const WithCustomTitle: Story = {
  args: {
    code: `# Deploy to production
npm run build
npm run deploy --env=production`,
    language: 'text',
    title: 'Deployment Commands',
    showLineNumbers: false
  }
}

export const WrapLongLines: Story = {
  args: {
    code: `const veryLongLine = "This is a very long line of code that would normally extend beyond the boundaries of the code block container, but with wrap enabled, it will wrap to the next line instead of requiring horizontal scrolling. This is useful for displaying long strings, URLs, or comments that might otherwise be difficult to read.";

const apiEndpoint = "https://api.openagents.com/v1/deployments/create?project=bitcoin-puns&environment=production&region=global&optimize=true";`,
    language: 'javascript',
    wrap: true
  }
}

export const MultipleCodeBlocks: Story = {
  args: {
    code: 'console.log("hello");'
  },
  render: () => (
    <div className="space-y-4">
      <p className="text-cyan-300 mb-4 font-sans">Here's how to create a Bitcoin puns website:</p>
      
      <CodeBlock
        code={`<!DOCTYPE html>
<html>
<head>
  <title>Bitcoin Puns</title>
</head>
<body>
  <div id="app"></div>
</body>
</html>`}
        language="html"
        title="index.html"
      />
      
      <CodeBlock
        code={`body {
  background: #000;
  color: #00d4ff;
  font-family: monospace;
}`}
        language="css"
        title="styles.css"
      />
      
      <CodeBlock
        code={`document.getElementById('app').innerHTML = 
  '<h1>Welcome to Bitcoin Puns!</h1>';`}
        language="javascript"
        title="app.js"
      />
    </div>
  )
}

export const InteractiveCopy: Story = {
  args: {
    code: 'console.log("hello");'
  },
  render: () => {
    const [message, setMessage] = useState('')
    
    return (
      <div className="space-y-4">
        <CodeBlock
          code={`// Click the copy button to test
const bitcoinPrice = 100000; // 🚀
console.log('To the moon!');`}
          language="javascript"
          onCopy={(code) => {
            setMessage(`Copied ${code.length} characters!`)
            setTimeout(() => setMessage(''), 3000)
          }}
        />
        {message && (
          <p className="text-green-400 text-center animate-pulse font-sans">{message}</p>
        )}
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    code: `// Playground - try different settings
function playground() {
  console.log('Edit me!');
}`,
    language: 'javascript',
    title: 'playground.js',
    showLineNumbers: true,
    showCopyButton: true,
    highlightLines: [],
    maxHeight: undefined,
    wrap: false,
    animated: true
  }
}