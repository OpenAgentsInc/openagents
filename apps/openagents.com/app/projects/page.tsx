'use client'

import React from 'react'
import Link from 'next/link'
import { AppLayout } from '@/components/AppLayout'
import { Text, GridLines, Dots, FrameCorners, cx } from '@arwes/react'
import { FolderOpen, Plus, Code, Globe, Clock } from 'lucide-react'

// Mock projects for demo
const mockProjects = [
  {
    id: '1',
    name: 'Bitcoin Puns Website',
    slug: 'bitcoin-puns-website',
    description: 'A fun website that generates Bitcoin-related puns',
    framework: 'React',
    status: 'deployed',
    deploymentUrl: 'https://bitcoin-puns-website.openagents.dev',
    lastUpdated: new Date(Date.now() - 2 * 60 * 60 * 1000), // 2 hours ago
  },
  {
    id: '2',
    name: 'Weather Dashboard',
    slug: 'weather-dashboard',
    description: 'Real-time weather tracking with beautiful visualizations',
    framework: 'Vue',
    status: 'deployed',
    deploymentUrl: 'https://weather-dashboard.openagents.dev',
    lastUpdated: new Date(Date.now() - 24 * 60 * 60 * 1000), // 1 day ago
  },
  {
    id: '3',
    name: 'Task Manager Pro',
    slug: 'task-manager-pro',
    description: 'A productivity app for managing daily tasks',
    framework: 'React',
    status: 'generating',
    lastUpdated: new Date(Date.now() - 5 * 60 * 1000), // 5 minutes ago
  },
]

function getStatusColor(status: string) {
  switch (status) {
    case 'deployed':
      return 'text-green-400'
    case 'deploying':
      return 'text-yellow-400'
    case 'generating':
      return 'text-cyan-400'
    case 'error':
      return 'text-red-400'
    default:
      return 'text-gray-400'
  }
}

function getTimeAgo(date: Date) {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000)
  
  if (seconds < 60) return 'just now'
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

export default function ProjectsPage() {
  return (
    <AppLayout>
      {/* Background effects */}
      <div className="fixed inset-0 pointer-events-none">
        <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
        <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
      </div>

      <div className="relative z-10 px-8 py-6">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <Text as="h1" className="text-3xl font-bold text-cyan-300 mb-2 font-sans">
              Projects
            </Text>
            <Text className="text-gray-400 font-sans">
              Your AI-generated applications
            </Text>
          </div>
          
          <Link
            href="/projects/new"
            className={cx(
              'flex items-center gap-2 px-4 py-2',
              'bg-cyan-500/20 hover:bg-cyan-500/30',
              'border border-cyan-500/50',
              'text-cyan-300 hover:text-cyan-200',
              'transition-all duration-200',
              'cursor-pointer'
            )}
          >
            <Plus size={18} />
            <Text className="font-sans">New Project</Text>
          </Link>
        </div>

        {/* Projects Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {mockProjects.map((project) => (
            <Link
              key={project.id}
              href={`/projects/${project.slug}`}
              className="block group"
            >
              <div className="relative h-full">
                <FrameCorners className="h-full">
                  <div className="p-6 h-full flex flex-col">
                    {/* Project Icon */}
                    <div className="mb-4">
                      <FolderOpen className="w-8 h-8 text-cyan-500/60" />
                    </div>
                    
                    {/* Project Name */}
                    <Text className="text-lg font-medium text-cyan-300 mb-2 group-hover:text-cyan-200 transition-colors font-sans">
                      {project.name}
                    </Text>
                    
                    {/* Description */}
                    <Text className="text-sm text-gray-400 mb-4 flex-1 font-sans">
                      {project.description}
                    </Text>
                    
                    {/* Footer */}
                    <div className="space-y-2">
                      {/* Framework & Status */}
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-gray-500 font-mono">{project.framework}</span>
                        <span className={cx(getStatusColor(project.status), 'font-mono')}>
                          {project.status}
                        </span>
                      </div>
                      
                      {/* Deployment URL */}
                      {project.deploymentUrl && (
                        <div className="flex items-center gap-1 text-xs text-gray-500">
                          <Globe size={12} />
                          <span className="truncate font-mono">{project.deploymentUrl.replace('https://', '')}</span>
                        </div>
                      )}
                      
                      {/* Last Updated */}
                      <div className="flex items-center gap-1 text-xs text-gray-500">
                        <Clock size={12} />
                        <span className="font-sans">{getTimeAgo(project.lastUpdated)}</span>
                      </div>
                    </div>
                  </div>
                </FrameCorners>
              </div>
            </Link>
          ))}
          
          {/* Empty State / Add New Project Card */}
          <Link
            href="/projects/new"
            className="block group"
          >
            <div className="relative h-full min-h-[200px]">
              <FrameCorners className="h-full border-dashed opacity-50 group-hover:opacity-100 transition-opacity">
                <div className="p-6 h-full flex flex-col items-center justify-center">
                  <Plus className="w-12 h-12 text-cyan-500/40 group-hover:text-cyan-500/60 transition-colors mb-4" />
                  <Text className="text-gray-500 group-hover:text-gray-400 transition-colors font-sans">
                    Create New Project
                  </Text>
                </div>
              </FrameCorners>
            </div>
          </Link>
        </div>
        
        {/* Demo Notice */}
        <div className="mt-12 p-4 border border-cyan-500/20 bg-cyan-500/5 rounded">
          <Text className="text-sm text-cyan-300/80 font-sans">
            <strong>Demo Mode:</strong> These are sample projects. Click on any project to explore the workspace interface.
          </Text>
        </div>
      </div>
    </AppLayout>
  )
}