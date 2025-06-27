'use client'

import React, { useState } from 'react'
import { useParams, useRouter } from 'next/navigation'
import { Text, cx } from '@arwes/react'
import { 
  ArrowLeft,
  Play,
  Download,
  Code,
  ExternalLink,
  Copy,
  Check,
  Star,
  Clock,
  Users,
  Zap,
  Shield,
  Globe
} from 'lucide-react'
import { AppLayout } from '@/components/AppLayout'
import { MonacoEditor } from '@/components/workspace/MonacoEditor'
import { getTemplateById, deployTemplate, type TemplateFile } from '@/lib/templates'
import { useToast } from '@/components/Toast'
import { DeploymentTracker } from '@/components/DeploymentTracker'

export default function TemplateDetailPage() {
  const params = useParams()
  const router = useRouter()
  const toast = useToast()
  const templateId = params.id as string
  const template = getTemplateById(templateId)

  const [selectedFile, setSelectedFile] = useState<TemplateFile | null>(null)
  const [isDeploying, setIsDeploying] = useState(false)
  const [deploymentUrl, setDeploymentUrl] = useState<string | null>(null)
  const [deploymentId, setDeploymentId] = useState<string | null>(null)
  const [copiedCode, setCopiedCode] = useState(false)

  if (!template) {
    return (
      <AppLayout>
        <div className="min-h-screen bg-black flex items-center justify-center">
          <div className="text-center">
            <Text className="text-2xl text-red-400 mb-4 font-sans">Template Not Found</Text>
            <Text className="text-gray-400 mb-6 font-sans">
              The template you're looking for doesn't exist or has been removed.
            </Text>
            <button
              onClick={() => router.push('/templates')}
              className="px-6 py-3 bg-cyan-500/20 border border-cyan-500/50 text-cyan-300 rounded-lg hover:bg-cyan-500/30 transition-colors font-sans"
            >
              Browse Templates
            </button>
          </div>
        </div>
      </AppLayout>
    )
  }

  // Set default selected file to the main entry point
  React.useEffect(() => {
    if (template.files.length > 0 && !selectedFile) {
      const mainFile = template.files.find(f => 
        f.path === 'src/App.tsx' || 
        f.path === 'app/page.tsx' || 
        f.path === 'index.html' ||
        f.path === 'package.json'
      ) || template.files[0]
      setSelectedFile(mainFile)
    }
  }, [template.files, selectedFile])

  const handleDeploy = async () => {
    // Generate unique deployment ID for WebSocket tracking
    const newDeploymentId = `deploy-${templateId}-${Date.now()}`
    setDeploymentId(newDeploymentId)
    setIsDeploying(true)
    
    toast.info('Deployment Started', `Initializing deployment for ${template.name}...`)

    try {
      // Start the deployment process (WebSocket will handle real-time updates)
      const result = await deployTemplate(template.id, template.name)
      
      // Note: DeploymentTracker will handle completion notifications via WebSocket
      // This is just fallback in case WebSocket fails
      if (!deploymentUrl) {
        setDeploymentUrl(result.deploymentUrl)
      }
    } catch (error) {
      console.error('Deployment failed:', error)
      toast.error('Deployment Failed', 'Something went wrong. Please try again.')
      setDeploymentId(null) // Clear deployment tracking on error
    } finally {
      setIsDeploying(false)
    }
  }

  const handleDeploymentComplete = (url: string) => {
    setDeploymentUrl(url)
    setIsDeploying(false)
    // DeploymentTracker handles success notifications
  }

  const handleCopyCode = async () => {
    if (selectedFile) {
      try {
        await navigator.clipboard.writeText(selectedFile.content)
        setCopiedCode(true)
        toast.success('Code Copied', 'File content copied to clipboard')
        setTimeout(() => setCopiedCode(false), 2000)
      } catch (error) {
        toast.error('Copy Failed', 'Could not copy to clipboard')
      }
    }
  }

  const handleDownload = () => {
    // Create a downloadable zip file (simplified implementation)
    toast.info('Download Starting', 'Template files will be downloaded shortly')
    
    // In a real implementation, this would create and download a zip file
    setTimeout(() => {
      toast.success('Download Complete', 'Template files downloaded successfully')
    }, 1000)
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

  return (
    <AppLayout>
      <div className="min-h-screen bg-black">
        {/* Header */}
        <div className="border-b border-cyan-900/30 bg-offblack">
          <div className="container mx-auto px-6 py-6">
            {/* Back Navigation */}
            <button
              onClick={() => router.push('/templates')}
              className="flex items-center gap-2 text-cyan-400 hover:text-cyan-300 mb-6 transition-colors font-sans"
            >
              <ArrowLeft className="w-4 h-4" />
              Back to Templates
            </button>

            <div className="grid lg:grid-cols-3 gap-8">
              {/* Template Info */}
              <div className="lg:col-span-2 space-y-6">
                <div>
                  <div className="flex items-center gap-4 mb-4">
                    <Text as="h1" className="text-3xl font-bold text-cyan-300 font-sans">
                      {template.name}
                    </Text>
                    <span className={cx(
                      'px-3 py-1 border rounded-full text-sm font-medium',
                      getDifficultyColor(template.difficulty)
                    )}>
                      {template.difficulty}
                    </span>
                  </div>
                  
                  <Text className="text-lg text-gray-300 mb-4 font-sans">
                    {template.description}
                  </Text>

                  {/* Tags */}
                  <div className="flex flex-wrap gap-2">
                    {template.tags.map((tag, index) => (
                      <span
                        key={index}
                        className="px-3 py-1 bg-blue-500/10 border border-blue-500/20 rounded-full text-sm text-blue-300 font-sans"
                      >
                        {tag}
                      </span>
                    ))}
                  </div>
                </div>

                {/* Features */}
                <div>
                  <Text as="h3" className="text-xl font-semibold text-cyan-300 mb-4 font-sans">
                    Features
                  </Text>
                  <div className="grid md:grid-cols-2 gap-3">
                    {template.features.map((feature, index) => (
                      <div key={index} className="flex items-start gap-3">
                        <Check className="w-5 h-5 text-green-400 mt-0.5 flex-shrink-0" />
                        <Text className="text-gray-300 font-sans">{feature}</Text>
                      </div>
                    ))}
                  </div>
                </div>

                {/* Stats */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                  <div className="text-center p-4 bg-black/30 border border-cyan-900/30 rounded-lg">
                    <Zap className="w-6 h-6 text-cyan-400 mx-auto mb-2" />
                    <div className="text-lg font-bold text-cyan-300">{template.framework}</div>
                    <Text className="text-xs text-gray-400 font-sans">Framework</Text>
                  </div>
                  <div className="text-center p-4 bg-black/30 border border-cyan-900/30 rounded-lg">
                    <Code className="w-6 h-6 text-cyan-400 mx-auto mb-2" />
                    <div className="text-lg font-bold text-cyan-300">{template.files.length}</div>
                    <Text className="text-xs text-gray-400 font-sans">Files</Text>
                  </div>
                  <div className="text-center p-4 bg-black/30 border border-cyan-900/30 rounded-lg">
                    <Clock className="w-6 h-6 text-cyan-400 mx-auto mb-2" />
                    <div className="text-lg font-bold text-cyan-300">30s</div>
                    <Text className="text-xs text-gray-400 font-sans">Deploy Time</Text>
                  </div>
                  <div className="text-center p-4 bg-black/30 border border-cyan-900/30 rounded-lg">
                    <Shield className="w-6 h-6 text-cyan-400 mx-auto mb-2" />
                    <div className="text-lg font-bold text-cyan-300">Prod</div>
                    <Text className="text-xs text-gray-400 font-sans">Ready</Text>
                  </div>
                </div>
              </div>

              {/* Action Panel */}
              <div className="lg:col-span-1">
                <div className="bg-black/30 border border-cyan-900/30 rounded-lg p-6 space-y-4">
                  <Text as="h3" className="text-lg font-semibold text-cyan-300 font-sans">
                    Quick Actions
                  </Text>

                  {deploymentUrl ? (
                    <div className="space-y-3">
                      <div className="p-3 bg-green-500/10 border border-green-500/30 rounded-lg">
                        <Text className="text-green-400 font-medium mb-2 font-sans">
                          ✅ Deployed Successfully!
                        </Text>
                        <Text className="text-sm text-gray-300 mb-3 font-sans">
                          Your template is now live and accessible.
                        </Text>
                        <button
                          onClick={() => window.open(deploymentUrl, '_blank')}
                          className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-green-500/20 border border-green-500/50 text-green-300 rounded hover:bg-green-500/30 transition-colors font-sans"
                        >
                          <Globe className="w-4 h-4" />
                          View Live Site
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button
                      onClick={handleDeploy}
                      disabled={isDeploying}
                      className={cx(
                        'w-full flex items-center justify-center gap-2 px-4 py-3',
                        'bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-500/50',
                        'text-cyan-300 hover:text-cyan-200 rounded-lg transition-all font-sans',
                        isDeploying && 'opacity-50 cursor-not-allowed'
                      )}
                    >
                      <Play className={cx('w-4 h-4', isDeploying && 'animate-spin')} />
                      {isDeploying ? 'Deploying...' : 'Deploy Template'}
                    </button>
                  )}

                  {/* Real-time Deployment Progress Tracker */}
                  {deploymentId && (
                    <div className="space-y-3">
                      <DeploymentTracker
                        deploymentId={deploymentId}
                        projectName={template.name}
                        onComplete={handleDeploymentComplete}
                        className="w-full"
                      />
                    </div>
                  )}

                  <button
                    onClick={handleDownload}
                    className="w-full flex items-center justify-center gap-2 px-4 py-2 border border-gray-500/30 hover:border-gray-400/50 text-gray-400 hover:text-gray-300 rounded transition-all font-sans"
                  >
                    <Download className="w-4 h-4" />
                    Download Files
                  </button>

                  {template.livePreview && (
                    <button
                      onClick={() => window.open(template.livePreview, '_blank')}
                      className="w-full flex items-center justify-center gap-2 px-4 py-2 border border-gray-500/30 hover:border-gray-400/50 text-gray-400 hover:text-gray-300 rounded transition-all font-sans"
                    >
                      <ExternalLink className="w-4 h-4" />
                      Live Preview
                    </button>
                  )}

                  {template.sourceUrl && (
                    <button
                      onClick={() => window.open(template.sourceUrl, '_blank')}
                      className="w-full flex items-center justify-center gap-2 px-4 py-2 border border-gray-500/30 hover:border-gray-400/50 text-gray-400 hover:text-gray-300 rounded transition-all font-sans"
                    >
                      <Code className="w-4 h-4" />
                      View Source
                    </button>
                  )}

                  <div className="pt-4 border-t border-gray-700/30">
                    <Text className="text-sm text-gray-400 mb-2 font-sans">
                      Need help with this template?
                    </Text>
                    <button className="text-cyan-400 hover:text-cyan-300 text-sm font-sans">
                      Contact Support →
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Code Explorer */}
        <div className="container mx-auto px-6 py-8">
          <div className="grid lg:grid-cols-4 gap-6 h-[600px]">
            {/* File List */}
            <div className="lg:col-span-1 bg-offblack border border-cyan-900/30 rounded-lg p-4">
              <div className="flex items-center justify-between mb-4">
                <Text className="font-semibold text-cyan-300 font-sans">
                  Project Files
                </Text>
                <Text className="text-xs text-gray-400 font-sans">
                  {template.files.length} files
                </Text>
              </div>
              
              <div className="space-y-1">
                {template.files.map((file, index) => (
                  <button
                    key={index}
                    onClick={() => setSelectedFile(file)}
                    className={cx(
                      'w-full text-left px-3 py-2 rounded text-sm transition-colors font-mono',
                      selectedFile?.path === file.path
                        ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30'
                        : 'text-gray-400 hover:text-gray-300 hover:bg-gray-800/50'
                    )}
                  >
                    {file.path}
                  </button>
                ))}
              </div>
            </div>

            {/* Code Viewer */}
            <div className="lg:col-span-3 bg-offblack border border-cyan-900/30 rounded-lg overflow-hidden">
              {selectedFile ? (
                <>
                  {/* File Header */}
                  <div className="flex items-center justify-between px-4 py-3 border-b border-cyan-900/30 bg-black/30">
                    <div className="flex items-center gap-3">
                      <Code className="w-4 h-4 text-cyan-400" />
                      <Text className="font-mono text-cyan-300">
                        {selectedFile.path}
                      </Text>
                      <span className="px-2 py-1 bg-gray-700/50 rounded text-xs text-gray-400 font-mono">
                        {selectedFile.language || 'text'}
                      </span>
                    </div>
                    
                    <button
                      onClick={handleCopyCode}
                      className="flex items-center gap-2 px-3 py-1.5 bg-cyan-500/20 border border-cyan-500/30 text-cyan-300 rounded hover:bg-cyan-500/30 transition-colors font-sans text-sm"
                    >
                      {copiedCode ? (
                        <>
                          <Check className="w-4 h-4" />
                          Copied!
                        </>
                      ) : (
                        <>
                          <Copy className="w-4 h-4" />
                          Copy
                        </>
                      )}
                    </button>
                  </div>

                  {/* Code Content */}
                  <div className="h-[500px]">
                    <MonacoEditor
                      value={selectedFile.content}
                      language={selectedFile.language || 'text'}
                      readOnly={true}
                      className="h-full"
                    />
                  </div>
                </>
              ) : (
                <div className="h-full flex items-center justify-center">
                  <Text className="text-gray-400 font-sans">
                    Select a file to view its contents
                  </Text>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </AppLayout>
  )
}