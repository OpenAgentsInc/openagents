'use client'

import React, { useState, useMemo } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  Search, 
  Filter, 
  ExternalLink, 
  Code, 
  Play, 
  Download,
  Star,
  Clock,
  Users,
  Tag
} from 'lucide-react'
import { AppLayout } from '@/components/AppLayout'
import { 
  DEMO_TEMPLATES, 
  TEMPLATE_CATEGORIES, 
  TEMPLATE_FRAMEWORKS,
  type ProjectTemplate 
} from '@/lib/templates'
import { useToast } from '@/components/Toast'

type FilterType = 'all' | keyof typeof TEMPLATE_CATEGORIES
type FrameworkFilter = 'all' | keyof typeof TEMPLATE_FRAMEWORKS
type DifficultyFilter = 'all' | 'beginner' | 'intermediate' | 'advanced'

function TemplatesPageContent() {
  const toast = useToast()
  const [searchQuery, setSearchQuery] = useState('')
  const [categoryFilter, setCategoryFilter] = useState<FilterType>('all')
  const [frameworkFilter, setFrameworkFilter] = useState<FrameworkFilter>('all')
  const [difficultyFilter, setDifficultyFilter] = useState<DifficultyFilter>('all')
  const [sortBy, setSortBy] = useState<'name' | 'difficulty' | 'framework'>('name')

  // Filter and search templates
  const filteredTemplates = useMemo(() => {
    let filtered = DEMO_TEMPLATES

    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase()
      filtered = filtered.filter(template =>
        template.name.toLowerCase().includes(query) ||
        template.description.toLowerCase().includes(query) ||
        template.tags.some(tag => tag.toLowerCase().includes(query)) ||
        template.features.some(feature => feature.toLowerCase().includes(query))
      )
    }

    // Apply category filter
    if (categoryFilter !== 'all') {
      filtered = filtered.filter(template => template.category === categoryFilter)
    }

    // Apply framework filter
    if (frameworkFilter !== 'all') {
      filtered = filtered.filter(template => template.framework === frameworkFilter)
    }

    // Apply difficulty filter
    if (difficultyFilter !== 'all') {
      filtered = filtered.filter(template => template.difficulty === difficultyFilter)
    }

    // Sort templates
    filtered.sort((a, b) => {
      switch (sortBy) {
        case 'name':
          return a.name.localeCompare(b.name)
        case 'difficulty':
          const difficultyOrder = { beginner: 0, intermediate: 1, advanced: 2 }
          return difficultyOrder[a.difficulty] - difficultyOrder[b.difficulty]
        case 'framework':
          return a.framework.localeCompare(b.framework)
        default:
          return 0
      }
    })

    return filtered
  }, [searchQuery, categoryFilter, frameworkFilter, difficultyFilter, sortBy])

  const handleDeploy = async (template: ProjectTemplate) => {
    toast.info('Deploying Template', `Starting deployment for ${template.name}...`)
    
    // Simulate deployment
    setTimeout(() => {
      const deploymentUrl = `https://${template.id}.openagents.dev`
      toast.success('Template Deployed!', `${template.name} is now live at ${deploymentUrl}`, {
        action: {
          label: 'View Live',
          onClick: () => window.open(deploymentUrl, '_blank')
        }
      })
    }, 3000)
  }

  const getDifficultyColor = (difficulty: string) => {
    switch (difficulty) {
      case 'beginner':
        return 'text-green-400 bg-green-400/10 border-green-400/20'
      case 'intermediate':
        return 'text-yellow-400 bg-yellow-400/10 border-yellow-400/20'
      case 'advanced':
        return 'text-red-400 bg-red-400/10 border-red-400/20'
      default:
        return 'text-cyan-400 bg-cyan-400/10 border-cyan-400/20'
    }
  }

  const getFrameworkIcon = (framework: string) => {
    // In a real app, these would be actual framework icons
    return '⚡'
  }

  return (
    <div className="min-h-screen bg-black">
        {/* Header */}
        <div className="border-b border-cyan-900/30 bg-offblack">
          <div className="container mx-auto px-6 py-8">
            <div className="text-center mb-8">
              <Text as="h1" className="text-4xl font-bold text-cyan-300 mb-4 font-sans">
                Project Templates
              </Text>
              <Text className="text-xl text-cyan-300/80 max-w-2xl mx-auto font-sans">
                Jump-start your development with production-ready templates. 
                Deploy instantly or customize to fit your needs.
              </Text>
            </div>

            {/* Search and Filters */}
            <div className="max-w-4xl mx-auto space-y-4">
              {/* Search Bar */}
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-gray-400" />
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search templates by name, technology, or features..."
                  className={cx(
                    'w-full pl-12 pr-4 py-3 bg-black/50 border border-cyan-900/30',
                    'rounded-lg text-cyan-300 placeholder-gray-500 font-sans',
                    'focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/50'
                  )}
                />
              </div>

              {/* Filter Controls */}
              <div className="flex flex-wrap gap-4 items-center">
                <div className="flex items-center gap-2">
                  <Filter className="w-4 h-4 text-cyan-400" />
                  <Text className="text-sm text-cyan-400 font-sans">Filters:</Text>
                </div>

                {/* Category Filter */}
                <select
                  value={categoryFilter}
                  onChange={(e) => setCategoryFilter(e.target.value as FilterType)}
                  className={cx(
                    'px-3 py-1.5 bg-black/50 border border-cyan-900/30 rounded',
                    'text-cyan-300 text-sm font-sans focus:outline-none focus:border-cyan-500/50'
                  )}
                >
                  <option value="all">All Categories</option>
                  {Object.entries(TEMPLATE_CATEGORIES).map(([key, label]) => (
                    <option key={key} value={key}>{label}</option>
                  ))}
                </select>

                {/* Framework Filter */}
                <select
                  value={frameworkFilter}
                  onChange={(e) => setFrameworkFilter(e.target.value as FrameworkFilter)}
                  className={cx(
                    'px-3 py-1.5 bg-black/50 border border-cyan-900/30 rounded',
                    'text-cyan-300 text-sm font-sans focus:outline-none focus:border-cyan-500/50'
                  )}
                >
                  <option value="all">All Frameworks</option>
                  {Object.entries(TEMPLATE_FRAMEWORKS).map(([key, label]) => (
                    <option key={key} value={key}>{label}</option>
                  ))}
                </select>

                {/* Difficulty Filter */}
                <select
                  value={difficultyFilter}
                  onChange={(e) => setDifficultyFilter(e.target.value as DifficultyFilter)}
                  className={cx(
                    'px-3 py-1.5 bg-black/50 border border-cyan-900/30 rounded',
                    'text-cyan-300 text-sm font-sans focus:outline-none focus:border-cyan-500/50'
                  )}
                >
                  <option value="all">All Levels</option>
                  <option value="beginner">Beginner</option>
                  <option value="intermediate">Intermediate</option>
                  <option value="advanced">Advanced</option>
                </select>

                {/* Sort By */}
                <select
                  value={sortBy}
                  onChange={(e) => setSortBy(e.target.value as 'name' | 'difficulty' | 'framework')}
                  className={cx(
                    'px-3 py-1.5 bg-black/50 border border-cyan-900/30 rounded',
                    'text-cyan-300 text-sm font-sans focus:outline-none focus:border-cyan-500/50'
                  )}
                >
                  <option value="name">Sort by Name</option>
                  <option value="difficulty">Sort by Difficulty</option>
                  <option value="framework">Sort by Framework</option>
                </select>

                <div className="ml-auto text-sm text-gray-400 font-sans">
                  {filteredTemplates.length} template{filteredTemplates.length !== 1 ? 's' : ''} found
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Templates Grid */}
        <div className="container mx-auto px-6 py-8">
          {filteredTemplates.length === 0 ? (
            <div className="text-center py-16">
              <Text className="text-xl text-gray-400 mb-4 font-sans">
                No templates match your search criteria
              </Text>
              <Text className="text-gray-500 font-sans">
                Try adjusting your filters or search terms
              </Text>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
              {filteredTemplates.map((template) => (
                <div
                  key={template.id}
                  className={cx(
                    'bg-offblack border border-cyan-900/30 rounded-lg overflow-hidden',
                    'hover:border-cyan-500/50 transition-all duration-300',
                    'hover:shadow-lg hover:shadow-cyan-500/10',
                    'hover:-translate-y-1'
                  )}
                >
                  {/* Template Preview */}
                  <div className="relative h-48 bg-gradient-to-br from-cyan-900/20 to-blue-900/20">
                    <div className="absolute inset-0 flex items-center justify-center">
                      <div className={cx(
                        'w-16 h-16 rounded-lg bg-cyan-500/20 border border-cyan-500/30',
                        'flex items-center justify-center text-2xl'
                      )}>
                        {getFrameworkIcon(template.framework)}
                      </div>
                    </div>
                    
                    {/* Framework Badge */}
                    <div className="absolute top-3 left-3">
                      <span className={cx(
                        'px-2 py-1 bg-black/50 border border-cyan-500/30 rounded',
                        'text-xs text-cyan-300 font-mono uppercase tracking-wide'
                      )}>
                        {TEMPLATE_FRAMEWORKS[template.framework]}
                      </span>
                    </div>

                    {/* Difficulty Badge */}
                    <div className="absolute top-3 right-3">
                      <span className={cx(
                        'px-2 py-1 border rounded text-xs font-medium',
                        getDifficultyColor(template.difficulty)
                      )}>
                        {template.difficulty}
                      </span>
                    </div>
                  </div>

                  {/* Template Info */}
                  <div className="p-6">
                    <div className="flex items-start justify-between mb-3">
                      <Text as="h3" className="text-lg font-semibold text-cyan-300 font-sans">
                        {template.name}
                      </Text>
                      <span className={cx(
                        'px-2 py-1 bg-cyan-500/10 border border-cyan-500/20 rounded',
                        'text-xs text-cyan-400 font-sans'
                      )}>
                        {TEMPLATE_CATEGORIES[template.category]}
                      </span>
                    </div>

                    <Text className="text-sm text-gray-400 mb-4 line-clamp-3 font-sans">
                      {template.description}
                    </Text>

                    {/* Features */}
                    <div className="mb-4">
                      <Text className="text-xs text-cyan-400 mb-2 font-sans">Key Features:</Text>
                      <div className="flex flex-wrap gap-1">
                        {template.features.slice(0, 3).map((feature, index) => (
                          <span
                            key={index}
                            className={cx(
                              'px-2 py-1 bg-gray-800/50 border border-gray-700/30 rounded',
                              'text-xs text-gray-300 font-sans'
                            )}
                          >
                            {feature}
                          </span>
                        ))}
                        {template.features.length > 3 && (
                          <span className="text-xs text-gray-500 font-sans">
                            +{template.features.length - 3} more
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Tags */}
                    <div className="mb-6">
                      <div className="flex flex-wrap gap-1">
                        {template.tags.slice(0, 4).map((tag, index) => (
                          <span
                            key={index}
                            className={cx(
                              'px-2 py-1 bg-blue-500/10 border border-blue-500/20 rounded',
                              'text-xs text-blue-300 font-sans'
                            )}
                          >
                            <Tag className="w-3 h-3 inline mr-1" />
                            {tag}
                          </span>
                        ))}
                      </div>
                    </div>

                    {/* Actions */}
                    <div className="flex gap-2">
                      <button
                        onClick={() => handleDeploy(template)}
                        className={cx(
                          'flex-1 flex items-center justify-center gap-2 px-4 py-2',
                          'bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50',
                          'text-cyan-300 hover:text-cyan-200 rounded transition-all',
                          'font-sans text-sm font-medium'
                        )}
                      >
                        <Play className="w-4 h-4" />
                        Deploy
                      </button>
                      
                      <button
                        onClick={() => {
                          window.open(`/templates/${template.id}/preview`, '_blank')
                        }}
                        className={cx(
                          'px-3 py-2 border border-gray-500/30 hover:border-gray-400/50',
                          'text-gray-400 hover:text-gray-300 rounded transition-all',
                          'font-sans text-sm'
                        )}
                        title="Preview template"
                      >
                        <ExternalLink className="w-4 h-4" />
                      </button>

                      <button
                        onClick={() => {
                          // TODO: Download template files
                          toast.info('Download Starting', 'Template files will be downloaded shortly')
                        }}
                        className={cx(
                          'px-3 py-2 border border-gray-500/30 hover:border-gray-400/50',
                          'text-gray-400 hover:text-gray-300 rounded transition-all',
                          'font-sans text-sm'
                        )}
                        title="Download template"
                      >
                        <Download className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Stats Section */}
        <div className="border-t border-cyan-900/30 bg-offblack">
          <div className="container mx-auto px-6 py-8">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-8 text-center">
              <div>
                <div className="text-3xl font-bold text-cyan-300 mb-2">
                  {DEMO_TEMPLATES.length}+
                </div>
                <Text className="text-gray-400 font-sans">
                  Production-Ready Templates
                </Text>
              </div>
              <div>
                <div className="text-3xl font-bold text-cyan-300 mb-2">
                  {Object.keys(TEMPLATE_FRAMEWORKS).length}+
                </div>
                <Text className="text-gray-400 font-sans">
                  Supported Frameworks
                </Text>
              </div>
              <div>
                <div className="text-3xl font-bold text-cyan-300 mb-2">
                  30s
                </div>
                <Text className="text-gray-400 font-sans">
                  Average Deploy Time
                </Text>
              </div>
            </div>
          </div>
        </div>
      </div>
  )
}

export default function TemplatesPage() {
  return (
    <AppLayout>
      <TemplatesPageContent />
    </AppLayout>
  )
}