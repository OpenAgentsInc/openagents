// Template types and configuration
export interface ProjectTemplate {
  id: string
  name: string
  description: string
  framework: 'react' | 'next' | 'html'
  category: 'frontend' | 'backend' | 'fullstack' | 'static'
  difficulty: 'beginner' | 'intermediate' | 'advanced'
  features: string[]
  tags: string[]
  previewImage?: string
  livePreview?: string
  sourceUrl?: string
  files: TemplateFile[]
  dependencies?: Record<string, string>
  scripts?: Record<string, string>
  envVars?: TemplateEnvVar[]
}

export interface TemplateFile {
  path: string
  content: string
  type: 'code' | 'config' | 'markdown' | 'asset'
  language?: string
}

export interface TemplateEnvVar {
  key: string
  description: string
  required: boolean
  defaultValue?: string
  example?: string
}

// Template categories for filtering
export const TEMPLATE_CATEGORIES = {
  frontend: 'Frontend',
  backend: 'Backend', 
  fullstack: 'Full Stack',
  static: 'Static Site'
} as const

export const TEMPLATE_FRAMEWORKS = {
  react: 'React',
  next: 'Next.js',
  html: 'HTML/CSS'
} as const

// Production-ready demo templates
export const DEMO_TEMPLATES: ProjectTemplate[] = [
  {
    id: 'react-todo-auth',
    name: 'React Todo App with Authentication',
    description: 'A complete todo application built with React, featuring user authentication, local storage, and a clean modern interface. Perfect for learning React hooks and state management.',
    framework: 'react',
    category: 'frontend',
    difficulty: 'intermediate',
    features: [
      'User authentication with local storage',
      'CRUD operations for todos',
      'Filter and search functionality',
      'Responsive design with Tailwind CSS',
      'TypeScript for type safety',
      'Custom hooks for state management'
    ],
    tags: ['React', 'TypeScript', 'Tailwind CSS', 'Authentication', 'Local Storage'],
    previewImage: '/templates/react-todo-preview.png',
    files: [
      {
        path: 'package.json',
        type: 'config',
        content: JSON.stringify({
          name: 'react-todo-auth',
          version: '1.0.0',
          private: true,
          dependencies: {
            'react': '^18.2.0',
            'react-dom': '^18.2.0',
            'react-scripts': '5.0.1',
            'typescript': '^4.9.5',
            '@types/react': '^18.2.0',
            '@types/react-dom': '^18.2.0',
            'tailwindcss': '^3.3.0',
            'lucide-react': '^0.263.1'
          },
          scripts: {
            start: 'react-scripts start',
            build: 'react-scripts build',
            test: 'react-scripts test',
            eject: 'react-scripts eject'
          }
        }, null, 2)
      },
      {
        path: 'src/App.tsx',
        type: 'code',
        language: 'typescript',
        content: `import React from 'react'
import { TodoProvider } from './contexts/TodoContext'
import { AuthProvider } from './contexts/AuthContext'
import TodoApp from './components/TodoApp'
import './index.css'

function App() {
  return (
    <AuthProvider>
      <TodoProvider>
        <div className="min-h-screen bg-gray-100">
          <TodoApp />
        </div>
      </TodoProvider>
    </AuthProvider>
  )
}

export default App`
      },
      {
        path: 'src/components/TodoApp.tsx',
        type: 'code',
        language: 'typescript',
        content: `import React from 'react'
import { useAuth } from '../hooks/useAuth'
import LoginForm from './LoginForm'
import TodoList from './TodoList'
import Header from './Header'

export default function TodoApp() {
  const { user, isLoading } = useAuth()

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="animate-spin rounded-full h-32 w-32 border-b-2 border-blue-500"></div>
      </div>
    )
  }

  if (!user) {
    return <LoginForm />
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <Header />
      <TodoList />
    </div>
  )
}`
      },
      {
        path: 'src/components/Header.tsx',
        type: 'code',
        language: 'typescript',
        content: `import React from 'react'
import { LogOut, User } from 'lucide-react'
import { useAuth } from '../hooks/useAuth'

export default function Header() {
  const { user, logout } = useAuth()

  return (
    <header className="bg-white rounded-lg shadow-md p-6 mb-8">
      <div className="flex justify-between items-center">
        <div className="flex items-center space-x-3">
          <div className="bg-blue-500 rounded-full p-2">
            <User className="h-6 w-6 text-white" />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-gray-800">My Todos</h1>
            <p className="text-gray-600">Welcome back, {user?.name}!</p>
          </div>
        </div>
        <button
          onClick={logout}
          className="flex items-center space-x-2 px-4 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 transition-colors"
        >
          <LogOut className="h-4 w-4" />
          <span>Logout</span>
        </button>
      </div>
    </header>
  )
}`
      },
      {
        path: 'src/hooks/useAuth.ts',
        type: 'code',
        language: 'typescript',
        content: `import { useContext } from 'react'
import { AuthContext } from '../contexts/AuthContext'

export function useAuth() {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider')
  }
  return context
}`
      },
      {
        path: 'tailwind.config.js',
        type: 'config',
        language: 'javascript',
        content: `/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.{js,jsx,ts,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}`
      },
      {
        path: 'README.md',
        type: 'markdown',
        content: `# React Todo App with Authentication

A modern, feature-rich todo application built with React and TypeScript.

## Features

- 🔐 User authentication with local storage
- ✅ Create, read, update, and delete todos
- 🔍 Search and filter functionality
- 📱 Responsive design with Tailwind CSS
- 🎯 TypeScript for type safety
- 🪝 Custom hooks for clean state management

## Getting Started

1. Install dependencies:
   \\\`\\\`\\\`bash
   npm install
   \\\`\\\`\\\`

2. Start the development server:
   \\\`\\\`\\\`bash
   npm start
   \\\`\\\`\\\`

3. Open [http://localhost:3000](http://localhost:3000) in your browser.

## Project Structure

- \`src/components/\` - React components
- \`src/contexts/\` - React context providers
- \`src/hooks/\` - Custom React hooks
- \`src/types/\` - TypeScript type definitions

## Technologies Used

- React 18 with TypeScript
- Tailwind CSS for styling
- Lucide React for icons
- Local Storage for persistence

## Demo Account

Use any email and password to create an account. Data is stored locally in your browser.
`
      }
    ]
  },

  {
    id: 'nextjs-landing-page',
    name: 'Next.js Landing Page with Forms',
    description: 'A beautiful, conversion-optimized landing page built with Next.js. Features contact forms, newsletter signup, and modern animations. Perfect for businesses and startups.',
    framework: 'next',
    category: 'frontend',
    difficulty: 'intermediate',
    features: [
      'Server-side rendering with Next.js',
      'Contact form with validation',
      'Newsletter signup integration',
      'Responsive design with animations',
      'SEO optimized with metadata',
      'TypeScript for type safety'
    ],
    tags: ['Next.js', 'TypeScript', 'Tailwind CSS', 'Forms', 'Animation', 'SEO'],
    previewImage: '/templates/nextjs-landing-preview.png',
    files: [
      {
        path: 'package.json',
        type: 'config',
        content: JSON.stringify({
          name: 'nextjs-landing-page',
          version: '1.0.0',
          private: true,
          dependencies: {
            'next': '^14.0.0',
            'react': '^18.2.0',
            'react-dom': '^18.2.0',
            'typescript': '^5.0.0',
            '@types/react': '^18.2.0',
            '@types/react-dom': '^18.2.0',
            'tailwindcss': '^3.3.0',
            'framer-motion': '^10.16.0',
            'lucide-react': '^0.263.1'
          },
          scripts: {
            dev: 'next dev',
            build: 'next build',
            start: 'next start',
            lint: 'next lint'
          }
        }, null, 2)
      },
      {
        path: 'app/page.tsx',
        type: 'code',
        language: 'typescript',
        content: `import Hero from './components/Hero'
import Features from './components/Features'
import Testimonials from './components/Testimonials'
import ContactForm from './components/ContactForm'
import Footer from './components/Footer'

export const metadata = {
  title: 'OpenAgents - Build the Future with AI',
  description: 'Transform your ideas into reality with our AI-powered development platform. Build, deploy, and scale applications faster than ever before.',
  keywords: 'AI, development, platform, automation, coding, deployment',
}

export default function HomePage() {
  return (
    <main className="min-h-screen">
      <Hero />
      <Features />
      <Testimonials />
      <ContactForm />
      <Footer />
    </main>
  )
}`
      },
      {
        path: 'app/components/Hero.tsx',
        type: 'code',
        language: 'typescript',
        content: `'use client'

import { motion } from 'framer-motion'
import { ArrowRight, Play, Star } from 'lucide-react'
import Link from 'next/link'

export default function Hero() {
  return (
    <section className="relative bg-gradient-to-br from-blue-50 to-indigo-100 min-h-screen flex items-center">
      <div className="container mx-auto px-6 py-24">
        <div className="grid lg:grid-cols-2 gap-12 items-center">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            className="space-y-8"
          >
            <div className="flex items-center space-x-2 text-sm font-semibold text-blue-600">
              <Star className="h-4 w-4 fill-current" />
              <span>Trusted by 10,000+ developers</span>
            </div>
            
            <h1 className="text-5xl lg:text-6xl font-bold text-gray-900 leading-tight">
              Build the{' '}
              <span className="bg-gradient-to-r from-blue-600 to-purple-600 bg-clip-text text-transparent">
                Future
              </span>{' '}
              with AI
            </h1>
            
            <p className="text-xl text-gray-600 leading-relaxed">
              Transform your ideas into reality with our AI-powered development platform. 
              Build, deploy, and scale applications faster than ever before.
            </p>
            
            <div className="flex flex-col sm:flex-row gap-4">
              <Link 
                href="/signup"
                className="inline-flex items-center px-8 py-4 bg-blue-600 text-white font-semibold rounded-lg hover:bg-blue-700 transition-colors"
              >
                Get Started Free
                <ArrowRight className="ml-2 h-5 w-5" />
              </Link>
              
              <button className="inline-flex items-center px-8 py-4 text-gray-700 font-semibold hover:text-blue-600 transition-colors">
                <Play className="mr-2 h-5 w-5" />
                Watch Demo
              </button>
            </div>
          </motion.div>
          
          <motion.div
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ duration: 0.8, delay: 0.2 }}
            className="relative"
          >
            <div className="bg-white rounded-2xl shadow-2xl p-8">
              <div className="space-y-4">
                <div className="h-4 bg-gray-200 rounded w-3/4"></div>
                <div className="h-4 bg-gray-200 rounded w-1/2"></div>
                <div className="h-4 bg-blue-200 rounded w-5/6"></div>
                <div className="h-32 bg-gradient-to-br from-blue-100 to-purple-100 rounded-lg"></div>
              </div>
            </div>
          </motion.div>
        </div>
      </div>
    </section>
  )
}`
      },
      {
        path: 'README.md',
        type: 'markdown',
        content: `# Next.js Landing Page

A modern, conversion-optimized landing page built with Next.js 14 and TypeScript.

## Features

- 🚀 Server-side rendering with Next.js 14
- 📝 Contact form with validation
- 📧 Newsletter signup integration
- 🎨 Responsive design with Tailwind CSS
- ✨ Smooth animations with Framer Motion
- 🔍 SEO optimized with metadata
- 📱 Mobile-first responsive design

## Getting Started

1. Install dependencies:
   \`\`\`bash
   npm install
   \`\`\`

2. Start the development server:
   \`\`\`bash
   npm run dev
   \`\`\`

3. Open [http://localhost:3000](http://localhost:3000) in your browser.

## Customization

- Update content in \`app/page.tsx\`
- Modify styles in Tailwind CSS classes
- Add your own images in \`public/\` directory
- Configure contact form endpoint in \`app/api/contact/route.ts\`

## Deployment

Deploy instantly to Vercel:

\\\`\\\`\\\`bash
npm run build
\\\`\\\`\\\`

Perfect for marketing sites, product launches, and business landing pages.
`
      }
    ]
  },

  {
    id: 'html-portfolio-site',
    name: 'HTML/CSS Portfolio Site',
    description: 'A clean, professional portfolio website built with vanilla HTML, CSS, and JavaScript. Features smooth animations, responsive design, and a contact form. Perfect for showcasing your work.',
    framework: 'html',
    category: 'static',
    difficulty: 'beginner',
    features: [
      'Vanilla HTML, CSS, and JavaScript',
      'Smooth scroll animations',
      'Responsive grid layout',
      'Contact form with validation',
      'Dark/light theme toggle',
      'Fast loading and SEO friendly'
    ],
    tags: ['HTML', 'CSS', 'JavaScript', 'Portfolio', 'Responsive', 'Animation'],
    previewImage: '/templates/html-portfolio-preview.png',
    files: [
      {
        path: 'index.html',
        type: 'code',
        language: 'html',
        content: `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>John Doe - Full Stack Developer</title>
    <meta name="description" content="Full Stack Developer specializing in modern web technologies. View my portfolio and get in touch.">
    <link rel="stylesheet" href="styles.css">
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
</head>
<body>
    <nav class="navbar">
        <div class="nav-container">
            <a href="#home" class="nav-logo">JD</a>
            <ul class="nav-menu">
                <li><a href="#home" class="nav-link">Home</a></li>
                <li><a href="#about" class="nav-link">About</a></li>
                <li><a href="#projects" class="nav-link">Projects</a></li>
                <li><a href="#contact" class="nav-link">Contact</a></li>
            </ul>
            <button class="theme-toggle" id="themeToggle">🌙</button>
        </div>
    </nav>

    <main>
        <section id="home" class="hero">
            <div class="hero-content">
                <h1 class="hero-title">Hi, I'm John Doe</h1>
                <p class="hero-subtitle">Full Stack Developer</p>
                <p class="hero-description">
                    I create beautiful, responsive websites and applications 
                    using modern technologies and best practices.
                </p>
                <div class="hero-buttons">
                    <a href="#projects" class="btn btn-primary">View My Work</a>
                    <a href="#contact" class="btn btn-secondary">Get In Touch</a>
                </div>
            </div>
            <div class="hero-image">
                <div class="placeholder-image"></div>
            </div>
        </section>

        <section id="about" class="about">
            <div class="container">
                <h2 class="section-title">About Me</h2>
                <div class="about-content">
                    <div class="about-text">
                        <p>
                            I'm a passionate full-stack developer with 5+ years of experience 
                            building web applications. I love working with modern technologies 
                            and creating solutions that make a difference.
                        </p>
                        <div class="skills">
                            <h3>Skills</h3>
                            <div class="skill-tags">
                                <span class="skill-tag">JavaScript</span>
                                <span class="skill-tag">React</span>
                                <span class="skill-tag">Node.js</span>
                                <span class="skill-tag">Python</span>
                                <span class="skill-tag">PostgreSQL</span>
                                <span class="skill-tag">AWS</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </section>

        <section id="projects" class="projects">
            <div class="container">
                <h2 class="section-title">Featured Projects</h2>
                <div class="projects-grid">
                    <div class="project-card">
                        <div class="project-image placeholder-image"></div>
                        <div class="project-content">
                            <h3>E-Commerce Platform</h3>
                            <p>Full-stack e-commerce solution with React, Node.js, and Stripe integration.</p>
                            <div class="project-tech">
                                <span>React</span>
                                <span>Node.js</span>
                                <span>MongoDB</span>
                            </div>
                            <div class="project-links">
                                <a href="#" class="project-link">Live Demo</a>
                                <a href="#" class="project-link">GitHub</a>
                            </div>
                        </div>
                    </div>
                    
                    <div class="project-card">
                        <div class="project-image placeholder-image"></div>
                        <div class="project-content">
                            <h3>Task Management App</h3>
                            <p>Collaborative task management tool with real-time updates and team features.</p>
                            <div class="project-tech">
                                <span>Vue.js</span>
                                <span>Express</span>
                                <span>Socket.io</span>
                            </div>
                            <div class="project-links">
                                <a href="#" class="project-link">Live Demo</a>
                                <a href="#" class="project-link">GitHub</a>
                            </div>
                        </div>
                    </div>
                    
                    <div class="project-card">
                        <div class="project-image placeholder-image"></div>
                        <div class="project-content">
                            <h3>Weather Dashboard</h3>
                            <p>Beautiful weather app with location search and 7-day forecasts.</p>
                            <div class="project-tech">
                                <span>JavaScript</span>
                                <span>CSS Grid</span>
                                <span>Weather API</span>
                            </div>
                            <div class="project-links">
                                <a href="#" class="project-link">Live Demo</a>
                                <a href="#" class="project-link">GitHub</a>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </section>

        <section id="contact" class="contact">
            <div class="container">
                <h2 class="section-title">Get In Touch</h2>
                <div class="contact-content">
                    <div class="contact-info">
                        <p>Have a project in mind? Let's work together!</p>
                        <div class="contact-methods">
                            <a href="mailto:john@example.com" class="contact-method">
                                📧 john@example.com
                            </a>
                            <a href="tel:+1234567890" class="contact-method">
                                📱 (123) 456-7890
                            </a>
                            <a href="https://linkedin.com/in/johndoe" class="contact-method">
                                💼 LinkedIn
                            </a>
                        </div>
                    </div>
                    <form class="contact-form" id="contactForm">
                        <div class="form-group">
                            <input type="text" id="name" name="name" placeholder="Your Name" required>
                        </div>
                        <div class="form-group">
                            <input type="email" id="email" name="email" placeholder="Your Email" required>
                        </div>
                        <div class="form-group">
                            <textarea id="message" name="message" placeholder="Your Message" rows="5" required></textarea>
                        </div>
                        <button type="submit" class="btn btn-primary">Send Message</button>
                    </form>
                </div>
            </div>
        </section>
    </main>

    <footer class="footer">
        <div class="container">
            <p>&copy; 2024 John Doe. All rights reserved.</p>
        </div>
    </footer>

    <script src="script.js"></script>
</body>
</html>`
      },
      {
        path: 'styles.css',
        type: 'code',
        language: 'css',
        content: `/* CSS Variables for theming */
:root {
  --primary-color: #3b82f6;
  --secondary-color: #1e40af;
  --text-color: #1f2937;
  --text-light: #6b7280;
  --bg-color: #ffffff;
  --bg-secondary: #f9fafb;
  --border-color: #e5e7eb;
  --shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  --shadow-lg: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
}

[data-theme="dark"] {
  --primary-color: #60a5fa;
  --secondary-color: #3b82f6;
  --text-color: #f9fafb;
  --text-light: #d1d5db;
  --bg-color: #111827;
  --bg-secondary: #1f2937;
  --border-color: #374151;
  --shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.3);
  --shadow-lg: 0 20px 25px -5px rgba(0, 0, 0, 0.3);
}

/* Reset and base styles */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: 'Inter', sans-serif;
  line-height: 1.6;
  color: var(--text-color);
  background-color: var(--bg-color);
  transition: background-color 0.3s ease, color 0.3s ease;
}

/* Navigation */
.navbar {
  position: fixed;
  top: 0;
  width: 100%;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  border-bottom: 1px solid var(--border-color);
  z-index: 1000;
  transition: all 0.3s ease;
}

[data-theme="dark"] .navbar {
  background: rgba(17, 24, 39, 0.95);
}

.nav-container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 1rem 2rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.nav-logo {
  font-size: 1.5rem;
  font-weight: 700;
  color: var(--primary-color);
  text-decoration: none;
}

.nav-menu {
  display: flex;
  list-style: none;
  gap: 2rem;
}

.nav-link {
  color: var(--text-color);
  text-decoration: none;
  font-weight: 500;
  transition: color 0.3s ease;
}

.nav-link:hover {
  color: var(--primary-color);
}

.theme-toggle {
  background: none;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  padding: 0.5rem;
  border-radius: 50%;
  transition: background-color 0.3s ease;
}

.theme-toggle:hover {
  background-color: var(--bg-secondary);
}

/* Hero Section */
.hero {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 1fr 1fr;
  align-items: center;
  gap: 4rem;
  padding: 8rem 2rem 4rem;
  max-width: 1200px;
  margin: 0 auto;
}

.hero-title {
  font-size: 3.5rem;
  font-weight: 700;
  margin-bottom: 1rem;
  background: linear-gradient(135deg, var(--primary-color), var(--secondary-color));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.hero-subtitle {
  font-size: 1.5rem;
  color: var(--text-light);
  margin-bottom: 1.5rem;
}

.hero-description {
  font-size: 1.1rem;
  margin-bottom: 2rem;
  color: var(--text-light);
}

.hero-buttons {
  display: flex;
  gap: 1rem;
}

.btn {
  padding: 0.75rem 1.5rem;
  border-radius: 0.5rem;
  text-decoration: none;
  font-weight: 600;
  transition: all 0.3s ease;
  display: inline-block;
  text-align: center;
}

.btn-primary {
  background-color: var(--primary-color);
  color: white;
}

.btn-primary:hover {
  background-color: var(--secondary-color);
  transform: translateY(-2px);
  box-shadow: var(--shadow-lg);
}

.btn-secondary {
  background-color: transparent;
  color: var(--text-color);
  border: 2px solid var(--border-color);
}

.btn-secondary:hover {
  border-color: var(--primary-color);
  color: var(--primary-color);
}

.hero-image {
  display: flex;
  justify-content: center;
  align-items: center;
}

.placeholder-image {
  width: 300px;
  height: 300px;
  background: linear-gradient(135deg, var(--primary-color), var(--secondary-color));
  border-radius: 1rem;
  opacity: 0.1;
}

/* Sections */
section {
  padding: 4rem 0;
}

.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 2rem;
}

.section-title {
  text-align: center;
  font-size: 2.5rem;
  font-weight: 700;
  margin-bottom: 3rem;
}

/* About Section */
.about {
  background-color: var(--bg-secondary);
}

.about-content {
  max-width: 800px;
  margin: 0 auto;
  text-align: center;
}

.about-text p {
  font-size: 1.1rem;
  margin-bottom: 2rem;
  color: var(--text-light);
}

.skills h3 {
  margin-bottom: 1rem;
  color: var(--text-color);
}

.skill-tags {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
}

.skill-tag {
  background-color: var(--primary-color);
  color: white;
  padding: 0.5rem 1rem;
  border-radius: 2rem;
  font-size: 0.9rem;
  font-weight: 500;
}

/* Projects Section */
.projects-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 2rem;
}

.project-card {
  background-color: var(--bg-color);
  border-radius: 1rem;
  overflow: hidden;
  box-shadow: var(--shadow);
  transition: transform 0.3s ease, box-shadow 0.3s ease;
}

.project-card:hover {
  transform: translateY(-5px);
  box-shadow: var(--shadow-lg);
}

.project-card .placeholder-image {
  width: 100%;
  height: 200px;
  margin: 0;
}

.project-content {
  padding: 1.5rem;
}

.project-content h3 {
  margin-bottom: 0.5rem;
  color: var(--text-color);
}

.project-content p {
  margin-bottom: 1rem;
  color: var(--text-light);
}

.project-tech {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.project-tech span {
  background-color: var(--bg-secondary);
  padding: 0.25rem 0.5rem;
  border-radius: 0.25rem;
  font-size: 0.8rem;
  color: var(--text-light);
}

.project-links {
  display: flex;
  gap: 1rem;
}

.project-link {
  color: var(--primary-color);
  text-decoration: none;
  font-weight: 500;
}

.project-link:hover {
  text-decoration: underline;
}

/* Contact Section */
.contact {
  background-color: var(--bg-secondary);
}

.contact-content {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4rem;
  align-items: start;
}

.contact-info p {
  font-size: 1.1rem;
  margin-bottom: 2rem;
  color: var(--text-light);
}

.contact-methods {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.contact-method {
  color: var(--text-color);
  text-decoration: none;
  font-weight: 500;
  transition: color 0.3s ease;
}

.contact-method:hover {
  color: var(--primary-color);
}

.contact-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-group input,
.form-group textarea {
  width: 100%;
  padding: 0.75rem;
  border: 2px solid var(--border-color);
  border-radius: 0.5rem;
  background-color: var(--bg-color);
  color: var(--text-color);
  font-family: inherit;
  transition: border-color 0.3s ease;
}

.form-group input:focus,
.form-group textarea:focus {
  outline: none;
  border-color: var(--primary-color);
}

/* Footer */
.footer {
  background-color: var(--bg-color);
  border-top: 1px solid var(--border-color);
  padding: 2rem 0;
  text-align: center;
  color: var(--text-light);
}

/* Responsive Design */
@media (max-width: 768px) {
  .nav-menu {
    gap: 1rem;
  }
  
  .hero {
    grid-template-columns: 1fr;
    text-align: center;
    padding-top: 6rem;
  }
  
  .hero-title {
    font-size: 2.5rem;
  }
  
  .hero-buttons {
    justify-content: center;
    flex-wrap: wrap;
  }
  
  .contact-content {
    grid-template-columns: 1fr;
    gap: 2rem;
  }
  
  .skill-tags {
    justify-content: center;
  }
}

/* Smooth scrolling */
html {
  scroll-behavior: smooth;
}

/* Animation utilities */
.fade-in {
  opacity: 0;
  transform: translateY(20px);
  transition: opacity 0.6s ease, transform 0.6s ease;
}

.fade-in.visible {
  opacity: 1;
  transform: translateY(0);
}`
      },
      {
        path: 'script.js',
        type: 'code',
        language: 'javascript',
        content: `// Theme Toggle Functionality
const themeToggle = document.getElementById('themeToggle');
const body = document.body;

// Check for saved theme preference or default to light mode
const currentTheme = localStorage.getItem('theme') || 'light';
body.setAttribute('data-theme', currentTheme);
updateThemeIcon(currentTheme);

themeToggle.addEventListener('click', () => {
  const currentTheme = body.getAttribute('data-theme');
  const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
  
  body.setAttribute('data-theme', newTheme);
  localStorage.setItem('theme', newTheme);
  updateThemeIcon(newTheme);
});

function updateThemeIcon(theme) {
  themeToggle.textContent = theme === 'dark' ? '☀️' : '🌙';
}

// Smooth scrolling for navigation links
document.querySelectorAll('a[href^="#"]').forEach(anchor => {
  anchor.addEventListener('click', function (e) {
    e.preventDefault();
    const target = document.querySelector(this.getAttribute('href'));
    if (target) {
      target.scrollIntoView({
        behavior: 'smooth',
        block: 'start'
      });
    }
  });
});

// Contact form handling
const contactForm = document.getElementById('contactForm');
contactForm.addEventListener('submit', function(e) {
  e.preventDefault();
  
  // Get form data
  const formData = new FormData(contactForm);
  const name = formData.get('name');
  const email = formData.get('email');
  const message = formData.get('message');
  
  // Simple validation
  if (!name || !email || !message) {
    alert('Please fill in all fields');
    return;
  }
  
  if (!isValidEmail(email)) {
    alert('Please enter a valid email address');
    return;
  }
  
  // Simulate form submission
  const submitButton = contactForm.querySelector('button[type="submit"]');
  const originalText = submitButton.textContent;
  
  submitButton.textContent = 'Sending...';
  submitButton.disabled = true;
  
  // Simulate API call delay
  setTimeout(() => {
    alert('Thank you for your message! I will get back to you soon.');
    contactForm.reset();
    submitButton.textContent = originalText;
    submitButton.disabled = false;
  }, 1000);
});

function isValidEmail(email) {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

// Intersection Observer for scroll animations
const observerOptions = {
  threshold: 0.1,
  rootMargin: '0px 0px -50px 0px'
};

const observer = new IntersectionObserver((entries) => {
  entries.forEach(entry => {
    if (entry.isIntersecting) {
      entry.target.classList.add('visible');
    }
  });
}, observerOptions);

// Observe elements for animation
document.addEventListener('DOMContentLoaded', () => {
  const animatedElements = document.querySelectorAll('.project-card, .about-content, .contact-content');
  animatedElements.forEach(el => {
    el.classList.add('fade-in');
    observer.observe(el);
  });
});

// Active navigation link highlighting
window.addEventListener('scroll', () => {
  const sections = document.querySelectorAll('section[id]');
  const navLinks = document.querySelectorAll('.nav-link');
  
  let current = '';
  
  sections.forEach(section => {
    const sectionTop = section.offsetTop;
    const sectionHeight = section.clientHeight;
    if (window.pageYOffset >= (sectionTop - 200)) {
      current = section.getAttribute('id');
    }
  });
  
  navLinks.forEach(link => {
    link.classList.remove('active');
    if (link.getAttribute('href') === \`#\${current}\`) {
      link.classList.add('active');
    }
  });
});

// Add CSS for active nav link
const style = document.createElement('style');
style.textContent = \\\`
  .nav-link.active {
    color: var(--primary-color) !important;
  }
\\\`;
document.head.appendChild(style);`
      },
      {
        path: 'README.md',
        type: 'markdown',
        content: `# HTML/CSS Portfolio Website

A clean, professional portfolio website built with vanilla HTML, CSS, and JavaScript.

## Features

- 🎨 Clean, modern design
- 📱 Fully responsive layout
- 🌙 Dark/light theme toggle
- ✨ Smooth scroll animations
- 📧 Contact form with validation
- 🚀 Fast loading and SEO friendly
- 🎯 Accessible and semantic HTML

## Sections

- **Hero** - Introduction and call-to-action
- **About** - Personal info and skills
- **Projects** - Portfolio showcase
- **Contact** - Contact form and info

## Customization

1. **Personal Information**: Update content in \`index.html\`
2. **Styling**: Modify colors and layout in \`styles.css\`
3. **Projects**: Add your projects in the projects section
4. **Images**: Replace placeholder images with your own
5. **Contact Form**: Configure form handling in \`script.js\`

## Getting Started

1. Download all files
2. Replace placeholder content with your information
3. Add your project images to an \`images/\` folder
4. Upload to any web hosting service

## File Structure

- \`index.html\` - Main HTML structure
- \`styles.css\` - All styling and responsive design
- \`script.js\` - Interactive functionality
- \`README.md\` - Documentation

Perfect for developers, designers, and creatives looking to showcase their work online.
`
      }
    ]
  }
]

// Helper functions for template management
export function getTemplateById(id: string): ProjectTemplate | undefined {
  return DEMO_TEMPLATES.find(template => template.id === id)
}

export function getTemplatesByCategory(category: keyof typeof TEMPLATE_CATEGORIES): ProjectTemplate[] {
  return DEMO_TEMPLATES.filter(template => template.category === category)
}

export function getTemplatesByFramework(framework: keyof typeof TEMPLATE_FRAMEWORKS): ProjectTemplate[] {
  return DEMO_TEMPLATES.filter(template => template.framework === framework)
}

export function searchTemplates(query: string): ProjectTemplate[] {
  const searchQuery = query.toLowerCase()
  return DEMO_TEMPLATES.filter(template => 
    template.name.toLowerCase().includes(searchQuery) ||
    template.description.toLowerCase().includes(searchQuery) ||
    template.tags.some(tag => tag.toLowerCase().includes(searchQuery)) ||
    template.features.some(feature => feature.toLowerCase().includes(searchQuery))
  )
}

export function getTemplatePreviewUrl(templateId: string): string {
  // In production, this would generate actual preview URLs
  return `/api/templates/${templateId}/preview`
}

export function deployTemplate(templateId: string, projectName: string): Promise<{ deploymentUrl: string }> {
  // In production, this would trigger actual deployment
  return new Promise(resolve => {
    setTimeout(() => {
      resolve({
        deploymentUrl: `https://${projectName.toLowerCase().replace(/\s+/g, '-')}.openagents.dev`
      })
    }, 3000)
  })
}