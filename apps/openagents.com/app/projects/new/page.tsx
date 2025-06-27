'use client'

import React, { useState } from 'react'
import { useRouter } from 'next/navigation'
import { AppLayout } from '@/components/AppLayout'
import { ProjectImport } from '@/components/workspace/ProjectImport'
import { Text, GridLines, Dots, FrameCorners, cx } from '@arwes/react'
import { ArrowLeft, Plus, Upload } from 'lucide-react'
import Link from 'next/link'

export default function NewProjectPage() {
  const router = useRouter()
  const [projectName, setProjectName] = useState('')
  const [description, setDescription] = useState('')
  const [createMode, setCreateMode] = useState<'scratch' | 'import'>('scratch')
  
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    // Convert name to slug
    const slug = projectName.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')
    // In a real app, this would create the project via API
    // For demo, just navigate to the project workspace
    router.push(`/projects/${slug}`)
  }

  const handleImport = (files: any[]) => {
    // Generate project name from first file or use default
    const defaultName = files.length > 0 ? 
      files[0].name.split('.')[0].replace(/[-_]/g, ' ') : 
      'Imported Project'
    
    const slug = defaultName.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')
    
    // In a real app, this would process the files and create the project
    console.log('Importing files:', files)
    router.push(`/projects/${slug}`)
  }
  
  return (
    <AppLayout>
      {/* Background effects */}
      <div className="fixed inset-0 pointer-events-none">
        <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
        <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
      </div>

      <div className="relative z-10 px-8 py-6 max-w-2xl mx-auto">
        {/* Back link */}
        <Link
          href="/projects"
          className="inline-flex items-center gap-2 text-cyan-500 hover:text-cyan-300 transition-colors mb-8"
        >
          <ArrowLeft size={18} />
          <Text className="font-sans">Back to Projects</Text>
        </Link>

        {/* Header */}
        <Text as="h1" className="text-3xl font-bold text-cyan-300 mb-6 font-sans">
          Create New Project
        </Text>

        {/* Mode Toggle */}
        <div className="flex gap-4 mb-8">
          <button
            onClick={() => setCreateMode('scratch')}
            className={cx(
              'flex items-center gap-2 px-4 py-2 rounded border transition-all',
              createMode === 'scratch'
                ? 'bg-cyan-500/20 border-cyan-500/50 text-cyan-300'
                : 'border-gray-500/30 text-gray-400 hover:text-gray-300 hover:border-gray-400/50'
            )}
          >
            <Plus size={16} />
            <Text className="font-sans">Start from Scratch</Text>
          </button>
          
          <button
            onClick={() => setCreateMode('import')}
            className={cx(
              'flex items-center gap-2 px-4 py-2 rounded border transition-all',
              createMode === 'import'
                ? 'bg-cyan-500/20 border-cyan-500/50 text-cyan-300'
                : 'border-gray-500/30 text-gray-400 hover:text-gray-300 hover:border-gray-400/50'
            )}
          >
            <Upload size={16} />
            <Text className="font-sans">Import Files</Text>
          </button>
        </div>

        {/* Create from Scratch Form */}
        {createMode === 'scratch' && (
          <form onSubmit={handleSubmit} className="space-y-6">
          <div>
            <label htmlFor="name" className="block mb-2">
              <Text className="text-sm text-cyan-400 font-sans">Project Name</Text>
            </label>
            <input
              id="name"
              type="text"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              placeholder="My Awesome App"
              required
              className={cx(
                'w-full px-4 py-2 bg-black/50',
                'border border-cyan-500/30 focus:border-cyan-500/60',
                'text-cyan-300 placeholder-gray-500 font-sans',
                'outline-none transition-colors'
              )}
            />
          </div>

          <div>
            <label htmlFor="description" className="block mb-2">
              <Text className="text-sm text-cyan-400 font-sans">Description</Text>
            </label>
            <textarea
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe what you want to build..."
              rows={4}
              className={cx(
                'w-full px-4 py-2 bg-black/50',
                'border border-cyan-500/30 focus:border-cyan-500/60',
                'text-cyan-300 placeholder-gray-500 font-sans',
                'outline-none transition-colors resize-none'
              )}
            />
          </div>

          <div className="flex gap-4">
            <button
              type="submit"
              disabled={!projectName.trim()}
              className={cx(
                'px-6 py-2',
                'bg-cyan-500/20 hover:bg-cyan-500/30',
                'border border-cyan-500/50',
                'text-cyan-300 hover:text-cyan-200',
                'transition-all duration-200',
                'disabled:opacity-50 disabled:cursor-not-allowed'
              )}
            >
              <Text className="font-sans">Create Project</Text>
            </button>
            
            <Link
              href="/projects"
              className={cx(
                'px-6 py-2',
                'border border-gray-500/50',
                'text-gray-400 hover:text-gray-300',
                'transition-all duration-200'
              )}
            >
              <Text className="font-sans">Cancel</Text>
            </Link>
          </div>
          </form>
        )}

        {/* Import Files */}
        {createMode === 'import' && (
          <ProjectImport
            onImport={handleImport}
            onCancel={() => router.push('/projects')}
          />
        )}

        {/* Info */}
        {createMode === 'scratch' && (
          <div className="mt-12 p-4 border border-cyan-500/20 bg-cyan-500/5 rounded">
            <Text className="text-sm text-cyan-300/80 font-sans">
              <strong>Tip:</strong> Once created, you can start chatting with AI to build your application. Just describe what you want and watch it come to life!
            </Text>
          </div>
        )}

        {createMode === 'import' && (
          <div className="mt-8 p-4 border border-cyan-500/20 bg-cyan-500/5 rounded">
            <Text className="text-sm text-cyan-300/80 font-sans">
              <strong>Import Tips:</strong> Drag and drop your project files or click to browse. Supports common web development files like .js, .jsx, .ts, .tsx, .html, .css, and more. The AI will analyze your code and help you extend it!
            </Text>
          </div>
        )}
      </div>
    </AppLayout>
  )
}