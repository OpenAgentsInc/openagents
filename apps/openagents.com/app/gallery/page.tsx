'use client'

import React, { useState, useMemo } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  Grid,
  Search,
  Filter,
  Star,
  ExternalLink,
  GitFork,
  Eye,
  Calendar,
  User,
  Code,
  Zap,
  Rocket,
  Heart,
  TrendingUp,
  Clock
} from 'lucide-react'
import { AppLayout } from '@/components/AppLayout'
import Link from 'next/link'

// Mock project data - in production this would come from database
interface GalleryProject {
  id: string
  name: string
  description: string
  author: string
  authorAvatar: string
  deployedUrl: string
  sourceUrl?: string
  createdAt: string
  updatedAt: string
  framework: 'react' | 'next' | 'html'
  category: 'frontend' | 'backend' | 'fullstack' | 'static'
  tags: string[]
  stats: {
    views: number
    forks: number
    stars: number
    deploys: number
  }
  featured: boolean
  screenshot: string
  isPublic: boolean
}

const MOCK_PROJECTS: GalleryProject[] = [
  {
    id: 'bitcoin-puns-generator',
    name: 'Bitcoin Puns Generator',
    description: 'AI-powered Bitcoin pun generator with social sharing and daily rewards. Built with Next.js and OpenAI.',
    author: 'SatoshiJokester',
    authorAvatar: '/avatars/satoshi.png',
    deployedUrl: 'https://bitcoin-puns.openagents.dev',
    sourceUrl: 'https://github.com/openagents/bitcoin-puns',
    createdAt: '2025-06-20T10:00:00Z',
    updatedAt: '2025-06-26T15:30:00Z',
    framework: 'next',
    category: 'frontend',
    tags: ['AI', 'Bitcoin', 'Humor', 'Next.js', 'OpenAI'],
    stats: { views: 2847, forks: 23, stars: 156, deploys: 12 },
    featured: true,
    screenshot: '/gallery/bitcoin-puns-preview.png',
    isPublic: true
  },
  {
    id: 'nostr-chat-app',
    name: 'Decentralized Chat',
    description: 'Privacy-first chat application built on Nostr protocol with Lightning payments integration.',
    author: 'NostrBuilder',
    authorAvatar: '/avatars/nostr.png',
    deployedUrl: 'https://nostr-chat.openagents.dev',
    createdAt: '2025-06-18T14:20:00Z',
    updatedAt: '2025-06-25T09:15:00Z',
    framework: 'react',
    category: 'frontend',
    tags: ['Nostr', 'Privacy', 'Lightning', 'Chat', 'Decentralized'],
    stats: { views: 1923, forks: 18, stars: 89, deploys: 8 },
    featured: true,
    screenshot: '/gallery/nostr-chat-preview.png',
    isPublic: true
  },
  {
    id: 'lightning-tip-jar',
    name: 'Lightning Tip Jar',
    description: 'Simple tip jar widget for content creators with Lightning Network integration and real-time notifications.',
    author: 'LightningDev',
    authorAvatar: '/avatars/lightning.png',
    deployedUrl: 'https://tip-jar.openagents.dev',
    sourceUrl: 'https://github.com/openagents/lightning-tip-jar',
    createdAt: '2025-06-15T11:45:00Z',
    updatedAt: '2025-06-24T16:20:00Z',
    framework: 'html',
    category: 'static',
    tags: ['Lightning', 'Tips', 'Widget', 'Payments'],
    stats: { views: 1456, forks: 34, stars: 201, deploys: 45 },
    featured: false,
    screenshot: '/gallery/tip-jar-preview.png',
    isPublic: true
  },
  {
    id: 'bitcoin-dashboard',
    name: 'Bitcoin Analytics Dashboard',
    description: 'Real-time Bitcoin metrics and portfolio tracker with beautiful charts and price alerts built with React.',
    author: 'BitcoinAnalyst',
    authorAvatar: '/avatars/analyst.png',
    deployedUrl: 'https://btc-dashboard.openagents.dev',
    createdAt: '2025-06-12T08:30:00Z',
    updatedAt: '2025-06-23T12:00:00Z',
    framework: 'react',
    category: 'frontend',
    tags: ['Bitcoin', 'Analytics', 'Dashboard', 'Charts', 'React', 'Recharts'],
    stats: { views: 3421, forks: 67, stars: 287, deploys: 23 },
    featured: true,
    screenshot: '/gallery/btc-dashboard-preview.png',
    isPublic: true
  },
  {
    id: 'portfolio-generator',
    name: 'Developer Portfolio Generator',
    description: 'AI-powered portfolio generator that creates beautiful developer portfolios from GitHub data.',
    author: 'CodeCrafter',
    authorAvatar: '/avatars/crafter.png',
    deployedUrl: 'https://portfolio-gen.openagents.dev',
    sourceUrl: 'https://github.com/openagents/portfolio-generator',
    createdAt: '2025-06-10T16:15:00Z',
    updatedAt: '2025-06-22T10:45:00Z',
    framework: 'next',
    category: 'fullstack',
    tags: ['Portfolio', 'AI', 'GitHub', 'Generator', 'Developer Tools'],
    stats: { views: 5632, forks: 89, stars: 445, deploys: 156 },
    featured: false,
    screenshot: '/gallery/portfolio-gen-preview.png',
    isPublic: true
  }
]

const FRAMEWORK_COLORS = {
  react: 'text-blue-400 bg-blue-400/10 border-blue-400/20',
  next: 'text-cyan-400 bg-cyan-400/10 border-cyan-400/20',
  html: 'text-orange-400 bg-orange-400/10 border-orange-400/20'
}

const CATEGORY_COLORS = {
  frontend: 'text-cyan-300',
  backend: 'text-green-300',
  fullstack: 'text-purple-300',
  static: 'text-orange-300'
}

export default function GalleryPage() {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedFramework, setSelectedFramework] = useState<string>('all')
  const [selectedCategory, setSelectedCategory] = useState<string>('all')
  const [sortBy, setSortBy] = useState<'featured' | 'recent' | 'popular' | 'trending'>('featured')
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid')

  // Filter and sort projects
  const filteredProjects = useMemo(() => {
    let filtered = MOCK_PROJECTS.filter(project => {
      const matchesSearch = project.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                           project.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
                           project.tags.some(tag => tag.toLowerCase().includes(searchQuery.toLowerCase()))
      
      const matchesFramework = selectedFramework === 'all' || project.framework === selectedFramework
      const matchesCategory = selectedCategory === 'all' || project.category === selectedCategory
      
      return matchesSearch && matchesFramework && matchesCategory && project.isPublic
    })

    // Sort projects
    switch (sortBy) {
      case 'recent':
        return filtered.sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
      case 'popular':
        return filtered.sort((a, b) => b.stats.views - a.stats.views)
      case 'trending':
        return filtered.sort((a, b) => b.stats.stars - a.stats.stars)
      default:
        return filtered.sort((a, b) => (b.featured ? 1 : 0) - (a.featured ? 1 : 0))
    }
  }, [searchQuery, selectedFramework, selectedCategory, sortBy])

  const featuredProjects = MOCK_PROJECTS.filter(p => p.featured && p.isPublic).slice(0, 3)

  return (
    <AppLayout>
      <div className="min-h-screen bg-black">
        {/* Header */}
        <div className="border-b border-cyan-900/30 bg-offblack/50">
          <div className="max-w-7xl mx-auto px-6 py-8">
            <div className="text-center mb-8">
              <Text className="text-4xl font-bold text-cyan-300 mb-4 font-sans">
                🏛️ Project Gallery
              </Text>
              <Text className="text-xl text-gray-300 max-w-2xl mx-auto font-sans">
                Discover amazing projects built by the OpenAgents community. 
                Get inspired, fork projects, and share your own creations.
              </Text>
            </div>

            {/* Stats */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
              <div className="bg-black/30 rounded-lg p-4 border border-cyan-900/20">
                <Text className="text-2xl font-bold text-cyan-400 font-mono">
                  {MOCK_PROJECTS.length}
                </Text>
                <Text className="text-sm text-gray-400 font-sans">Projects</Text>
              </div>
              <div className="bg-black/30 rounded-lg p-4 border border-cyan-900/20">
                <Text className="text-2xl font-bold text-green-400 font-mono">
                  {MOCK_PROJECTS.reduce((sum, p) => sum + p.stats.deploys, 0)}
                </Text>
                <Text className="text-sm text-gray-400 font-sans">Deployments</Text>
              </div>
              <div className="bg-black/30 rounded-lg p-4 border border-cyan-900/20">
                <Text className="text-2xl font-bold text-yellow-400 font-mono">
                  {MOCK_PROJECTS.reduce((sum, p) => sum + p.stats.stars, 0)}
                </Text>
                <Text className="text-sm text-gray-400 font-sans">Stars</Text>
              </div>
              <div className="bg-black/30 rounded-lg p-4 border border-cyan-900/20">
                <Text className="text-2xl font-bold text-purple-400 font-mono">
                  {new Set(MOCK_PROJECTS.map(p => p.author)).size}
                </Text>
                <Text className="text-sm text-gray-400 font-sans">Creators</Text>
              </div>
            </div>
          </div>
        </div>

        {/* Featured Projects */}
        <div className="max-w-7xl mx-auto px-6 py-8">
          <div className="flex items-center gap-3 mb-6">
            <Star className="w-6 h-6 text-yellow-400" />
            <Text className="text-2xl font-bold text-white font-sans">Featured Projects</Text>
          </div>
          
          <div className="grid md:grid-cols-3 gap-6 mb-12">
            {featuredProjects.map(project => (
              <div key={project.id} className="group relative bg-offblack border border-cyan-900/30 rounded-lg overflow-hidden hover:border-cyan-500/50 transition-all duration-300">
                <div className="absolute top-3 right-3 z-10">
                  <div className="bg-yellow-500/20 border border-yellow-500/50 rounded-full px-2 py-1">
                    <Star className="w-4 h-4 text-yellow-400" />
                  </div>
                </div>
                
                <div className="aspect-video bg-black/50 border-b border-cyan-900/30 flex items-center justify-center">
                  <Text className="text-gray-500 font-mono text-sm">
                    {project.screenshot ? 'Preview Available' : 'No Preview'}
                  </Text>
                </div>
                
                <div className="p-6">
                  <div className="flex items-center justify-between mb-3">
                    <Text className="text-lg font-bold text-cyan-300 group-hover:text-cyan-200 transition-colors font-sans">
                      {project.name}
                    </Text>
                    <div className={cx(
                      'px-2 py-1 rounded text-xs border font-mono',
                      FRAMEWORK_COLORS[project.framework]
                    )}>
                      {project.framework}
                    </div>
                  </div>
                  
                  <Text className="text-gray-300 text-sm mb-4 line-clamp-2 font-sans">
                    {project.description}
                  </Text>
                  
                  <div className="flex items-center justify-between text-sm text-gray-400 mb-4">
                    <div className="flex items-center gap-2">
                      <User className="w-4 h-4" />
                      <Text className="font-mono">{project.author}</Text>
                    </div>
                    <div className="flex items-center gap-4">
                      <div className="flex items-center gap-1">
                        <Eye className="w-4 h-4" />
                        <Text className="font-mono">{project.stats.views}</Text>
                      </div>
                      <div className="flex items-center gap-1">
                        <Star className="w-4 h-4" />
                        <Text className="font-mono">{project.stats.stars}</Text>
                      </div>
                    </div>
                  </div>
                  
                  <div className="flex gap-2">
                    <a
                      href={project.deployedUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex-1 bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50 text-cyan-300 hover:text-cyan-200 px-4 py-2 rounded transition-all text-center text-sm font-sans"
                    >
                      <ExternalLink className="w-4 h-4 inline mr-2" />
                      View Live
                    </a>
                    {project.sourceUrl && (
                      <a
                        href={project.sourceUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="bg-gray-700/50 hover:bg-gray-600/50 border border-gray-600/50 text-gray-300 hover:text-gray-200 px-4 py-2 rounded transition-all text-sm font-sans"
                      >
                        <Code className="w-4 h-4" />
                      </a>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Filters and Search */}
        <div className="max-w-7xl mx-auto px-6 py-6 border-t border-cyan-900/30">
          <div className="flex flex-col lg:flex-row gap-4 items-start lg:items-center justify-between">
            {/* Search */}
            <div className="relative flex-1 max-w-md">
              <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 w-4 h-4" />
              <input
                type="text"
                placeholder="Search projects, tags, or authors..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full bg-black/50 border border-gray-600/50 rounded-lg pl-10 pr-4 py-2 text-gray-300 placeholder-gray-500 focus:border-cyan-500/50 focus:outline-none font-sans"
              />
            </div>

            {/* Filters */}
            <div className="flex flex-wrap gap-3 items-center">
              <select
                value={selectedFramework}
                onChange={(e) => setSelectedFramework(e.target.value)}
                className="bg-black/50 border border-gray-600/50 rounded-lg px-3 py-2 text-gray-300 text-sm focus:border-cyan-500/50 focus:outline-none font-sans"
              >
                <option value="all">All Frameworks</option>
                <option value="react">React</option>
                <option value="next">Next.js</option>
                <option value="html">HTML/CSS</option>
              </select>

              <select
                value={selectedCategory}
                onChange={(e) => setSelectedCategory(e.target.value)}
                className="bg-black/50 border border-gray-600/50 rounded-lg px-3 py-2 text-gray-300 text-sm focus:border-cyan-500/50 focus:outline-none font-sans"
              >
                <option value="all">All Categories</option>
                <option value="frontend">Frontend</option>
                <option value="backend">Backend</option>
                <option value="fullstack">Full Stack</option>
                <option value="static">Static Site</option>
              </select>

              <select
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value as any)}
                className="bg-black/50 border border-gray-600/50 rounded-lg px-3 py-2 text-gray-300 text-sm focus:border-cyan-500/50 focus:outline-none font-sans"
              >
                <option value="featured">Featured</option>
                <option value="recent">Recently Updated</option>
                <option value="popular">Most Viewed</option>
                <option value="trending">Most Starred</option>
              </select>

              <div className="flex bg-black/50 border border-gray-600/50 rounded-lg p-1">
                <button
                  onClick={() => setViewMode('grid')}
                  className={cx(
                    'p-2 rounded transition-colors',
                    viewMode === 'grid' 
                      ? 'bg-cyan-500/20 text-cyan-300' 
                      : 'text-gray-400 hover:text-gray-300'
                  )}
                >
                  <Grid className="w-4 h-4" />
                </button>
                <button
                  onClick={() => setViewMode('list')}
                  className={cx(
                    'p-2 rounded transition-colors',
                    viewMode === 'list' 
                      ? 'bg-cyan-500/20 text-cyan-300' 
                      : 'text-gray-400 hover:text-gray-300'
                  )}
                >
                  <Filter className="w-4 h-4" />
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Projects Grid/List */}
        <div className="max-w-7xl mx-auto px-6 pb-12">
          <div className="flex items-center justify-between mb-6">
            <Text className="text-lg text-gray-300 font-sans">
              {filteredProjects.length} project{filteredProjects.length !== 1 ? 's' : ''} found
            </Text>
            
            <Link
              href="/templates"
              className="flex items-center gap-2 bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50 text-cyan-300 hover:text-cyan-200 px-4 py-2 rounded transition-all text-sm font-sans"
            >
              <Rocket className="w-4 h-4" />
              Start Building
            </Link>
          </div>

          {viewMode === 'grid' ? (
            <div className="grid md:grid-cols-2 xl:grid-cols-3 gap-6">
              {filteredProjects.map(project => (
                <div key={project.id} className="group bg-offblack border border-gray-700/50 rounded-lg overflow-hidden hover:border-cyan-500/50 transition-all duration-300">
                  <div className="aspect-video bg-black/50 border-b border-gray-700/50 flex items-center justify-center">
                    <Text className="text-gray-500 font-mono text-sm">
                      Preview Coming Soon
                    </Text>
                  </div>
                  
                  <div className="p-4">
                    <div className="flex items-center justify-between mb-2">
                      <Text className="font-bold text-cyan-300 group-hover:text-cyan-200 transition-colors font-sans">
                        {project.name}
                      </Text>
                      <div className={cx(
                        'px-2 py-1 rounded text-xs border font-mono',
                        FRAMEWORK_COLORS[project.framework]
                      )}>
                        {project.framework}
                      </div>
                    </div>
                    
                    <Text className="text-gray-300 text-sm mb-3 line-clamp-2 font-sans">
                      {project.description}
                    </Text>
                    
                    <div className="flex items-center gap-3 text-xs text-gray-400 mb-3">
                      <div className="flex items-center gap-1">
                        <Eye className="w-3 h-3" />
                        <Text className="font-mono">{project.stats.views}</Text>
                      </div>
                      <div className="flex items-center gap-1">
                        <Star className="w-3 h-3" />
                        <Text className="font-mono">{project.stats.stars}</Text>
                      </div>
                      <div className="flex items-center gap-1">
                        <GitFork className="w-3 h-3" />
                        <Text className="font-mono">{project.stats.forks}</Text>
                      </div>
                    </div>
                    
                    <div className="flex gap-2">
                      <a
                        href={project.deployedUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex-1 bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50 text-cyan-300 hover:text-cyan-200 px-3 py-2 rounded transition-all text-center text-sm font-sans"
                      >
                        <ExternalLink className="w-4 h-4 inline mr-2" />
                        Live
                      </a>
                      {project.sourceUrl && (
                        <a
                          href={project.sourceUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="bg-gray-700/50 hover:bg-gray-600/50 border border-gray-600/50 text-gray-300 hover:text-gray-200 px-3 py-2 rounded transition-all text-sm font-sans"
                        >
                          <Code className="w-4 h-4" />
                        </a>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="space-y-4">
              {filteredProjects.map(project => (
                <div key={project.id} className="group bg-offblack border border-gray-700/50 rounded-lg p-6 hover:border-cyan-500/50 transition-all">
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      <div className="flex items-center gap-3 mb-2">
                        <Text className="text-lg font-bold text-cyan-300 group-hover:text-cyan-200 transition-colors font-sans">
                          {project.name}
                        </Text>
                        <div className={cx(
                          'px-2 py-1 rounded text-xs border font-mono',
                          FRAMEWORK_COLORS[project.framework]
                        )}>
                          {project.framework}
                        </div>
                        <Text className={cx('text-xs font-mono', CATEGORY_COLORS[project.category])}>
                          {project.category}
                        </Text>
                      </div>
                      
                      <Text className="text-gray-300 mb-3 font-sans">
                        {project.description}
                      </Text>
                      
                      <div className="flex items-center gap-4 text-sm text-gray-400">
                        <div className="flex items-center gap-1">
                          <User className="w-4 h-4" />
                          <Text className="font-mono">{project.author}</Text>
                        </div>
                        <div className="flex items-center gap-1">
                          <Calendar className="w-4 h-4" />
                          <Text className="font-mono">
                            {new Date(project.updatedAt).toLocaleDateString()}
                          </Text>
                        </div>
                        <div className="flex items-center gap-3">
                          <div className="flex items-center gap-1">
                            <Eye className="w-4 h-4" />
                            <Text className="font-mono">{project.stats.views}</Text>
                          </div>
                          <div className="flex items-center gap-1">
                            <Star className="w-4 h-4" />
                            <Text className="font-mono">{project.stats.stars}</Text>
                          </div>
                          <div className="flex items-center gap-1">
                            <GitFork className="w-4 h-4" />
                            <Text className="font-mono">{project.stats.forks}</Text>
                          </div>
                        </div>
                      </div>
                    </div>
                    
                    <div className="flex gap-2 ml-4">
                      <a
                        href={project.deployedUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50 text-cyan-300 hover:text-cyan-200 px-4 py-2 rounded transition-all text-sm font-sans"
                      >
                        <ExternalLink className="w-4 h-4 inline mr-2" />
                        View Live
                      </a>
                      {project.sourceUrl && (
                        <a
                          href={project.sourceUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="bg-gray-700/50 hover:bg-gray-600/50 border border-gray-600/50 text-gray-300 hover:text-gray-200 px-4 py-2 rounded transition-all text-sm font-sans"
                        >
                          <Code className="w-4 h-4 inline mr-2" />
                          Source
                        </a>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}

          {filteredProjects.length === 0 && (
            <div className="text-center py-12">
              <Text className="text-2xl text-gray-400 mb-4 font-sans">
                No projects found
              </Text>
              <Text className="text-gray-500 mb-6 font-sans">
                Try adjusting your search criteria or explore our templates to get started.
              </Text>
              <Link
                href="/templates"
                className="inline-flex items-center gap-2 bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50 text-cyan-300 hover:text-cyan-200 px-6 py-3 rounded transition-all font-sans"
              >
                <Rocket className="w-5 h-5" />
                Browse Templates
              </Link>
            </div>
          )}
        </div>
      </div>
    </AppLayout>
  )
}