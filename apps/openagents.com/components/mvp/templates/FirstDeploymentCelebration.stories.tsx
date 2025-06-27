import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameKranox, Dots } from '@arwes/react'
import { DeploymentUrl } from '../atoms/DeploymentUrl.stories'
import { CopyButton } from '../atoms/CopyButton.stories'

// Confetti component
const Confetti = ({ active }: { active: boolean }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    if (!active || !canvasRef.current) return

    const canvas = canvasRef.current
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    canvas.width = window.innerWidth
    canvas.height = window.innerHeight

    const particles: Array<{
      x: number
      y: number
      vx: number
      vy: number
      color: string
      size: number
      angle: number
      angleSpeed: number
    }> = []

    const colors = ['#00ffff', '#ff00ff', '#ffff00', '#00ff00', '#ff0099']

    // Create particles
    for (let i = 0; i < 150; i++) {
      particles.push({
        x: Math.random() * canvas.width,
        y: -20,
        vx: (Math.random() - 0.5) * 4,
        vy: Math.random() * 3 + 2,
        color: colors[Math.floor(Math.random() * colors.length)],
        size: Math.random() * 4 + 2,
        angle: Math.random() * Math.PI * 2,
        angleSpeed: (Math.random() - 0.5) * 0.2
      })
    }

    let animationId: number

    const animate = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height)

      particles.forEach((particle, index) => {
        particle.x += particle.vx
        particle.y += particle.vy
        particle.vy += 0.1 // gravity
        particle.angle += particle.angleSpeed

        ctx.save()
        ctx.translate(particle.x, particle.y)
        ctx.rotate(particle.angle)
        ctx.fillStyle = particle.color
        ctx.fillRect(-particle.size / 2, -particle.size / 2, particle.size, particle.size)
        ctx.restore()

        // Remove particles that fall off screen
        if (particle.y > canvas.height) {
          particles.splice(index, 1)
        }
      })

      if (particles.length > 0) {
        animationId = requestAnimationFrame(animate)
      }
    }

    animate()

    return () => {
      if (animationId) {
        cancelAnimationFrame(animationId)
      }
    }
  }, [active])

  if (!active) return null

  return (
    <canvas
      ref={canvasRef}
      className="fixed inset-0 pointer-events-none z-50"
      style={{ opacity: 0.8 }}
    />
  )
}

// Achievement badges
const AchievementBadge = ({ 
  title, 
  description, 
  icon,
  delay = 0 
}: { 
  title: string
  description: string
  icon: React.ReactNode
  delay?: number
}) => {
  return (
    <Animator active={true} duration={{ enter: 0.5, delay }}>
      <Animated animated={[['opacity', 0, 1], ['scale', 0.8, 1]]}>
        <div className="bg-gradient-to-br from-cyan-500/20 to-purple-500/20 border border-cyan-500/50 rounded-lg p-4 text-center">
          <div className="text-cyan-400 mb-2 flex justify-center">
            {icon}
          </div>
          <Text as="h4" className="text-white font-bold mb-1">
            {title}
          </Text>
          <Text as="p" className="text-gray-400 text-sm">
            {description}
          </Text>
        </div>
      </Animated>
    </Animator>
  )
}

// Share button component
const ShareButton = ({ 
  platform, 
  url, 
  text 
}: { 
  platform: 'twitter' | 'linkedin' | 'copy'
  url: string
  text: string
}) => {
  const handleShare = () => {
    switch (platform) {
      case 'twitter':
        window.open(`https://twitter.com/intent/tweet?text=${encodeURIComponent(text)}&url=${encodeURIComponent(url)}`, '_blank')
        break
      case 'linkedin':
        window.open(`https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(url)}`, '_blank')
        break
      case 'copy':
        navigator.clipboard.writeText(`${text} ${url}`)
        break
    }
  }

  const icons = {
    twitter: (
      <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
        <path d="M23.953 4.57a10 10 0 01-2.825.775 4.958 4.958 0 002.163-2.723c-.951.555-2.005.959-3.127 1.184a4.92 4.92 0 00-8.384 4.482C7.69 8.095 4.067 6.13 1.64 3.162a4.822 4.822 0 00-.666 2.475c0 1.71.87 3.213 2.188 4.096a4.904 4.904 0 01-2.228-.616v.06a4.923 4.923 0 003.946 4.827 4.996 4.996 0 01-2.212.085 4.936 4.936 0 004.604 3.417 9.867 9.867 0 01-6.102 2.105c-.39 0-.779-.023-1.17-.067a13.995 13.995 0 007.557 2.209c9.053 0 13.998-7.496 13.998-13.985 0-.21 0-.42-.015-.63A9.935 9.935 0 0024 4.59z"/>
      </svg>
    ),
    linkedin: (
      <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
        <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/>
      </svg>
    ),
    copy: (
      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
      </svg>
    )
  }

  const labels = {
    twitter: 'Share on Twitter',
    linkedin: 'Share on LinkedIn',
    copy: 'Copy link'
  }

  return (
    <button
      onClick={handleShare}
      className="flex items-center gap-2 px-4 py-2 bg-gray-800/50 text-gray-300 border border-gray-700 rounded hover:bg-gray-700/50 hover:text-white transition-all duration-300"
    >
      {icons[platform]}
      <span className="text-sm">{labels[platform]}</span>
    </button>
  )
}

// Main component
export interface FirstDeploymentCelebrationProps {
  projectName?: string
  deploymentUrl?: string
  deploymentTime?: number
  showConfetti?: boolean
  showAchievements?: boolean
  showShareOptions?: boolean
  showNextSteps?: boolean
  onNextAction?: (action: 'explore' | 'edit' | 'new' | 'gallery') => void
  userName?: string
  isFirstProject?: boolean
  animated?: boolean
  className?: string
}

export const FirstDeploymentCelebration = ({
  projectName = 'Bitcoin Puns Website',
  deploymentUrl = 'https://bitcoin-puns-xyz.openagents.dev',
  deploymentTime = 23,
  showConfetti = true,
  showAchievements = true,
  showShareOptions = true,
  showNextSteps = true,
  onNextAction,
  userName = 'developer',
  isFirstProject = true,
  animated = true,
  className = ''
}: FirstDeploymentCelebrationProps) => {
  const [active, setActive] = useState(false)
  const [confettiActive, setConfettiActive] = useState(false)
  const [soundPlayed, setSoundPlayed] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    if (active && showConfetti) {
      setConfettiActive(true)
      const timer = setTimeout(() => setConfettiActive(false), 5000)
      return () => clearTimeout(timer)
    }
  }, [active, showConfetti])

  // Play success sound
  useEffect(() => {
    if (active && !soundPlayed) {
      setSoundPlayed(true)
      // In a real app, you'd play an actual sound here
      // const audio = new Audio('/sounds/success.mp3')
      // audio.play()
    }
  }, [active, soundPlayed])

  const achievements = [
    {
      title: 'Speed Demon',
      description: `Deployed in ${deploymentTime} seconds`,
      icon: <svg className="w-8 h-8" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M12.395 2.553a1 1 0 00-1.45-.385c-.345.23-.614.558-.822.88-.214.33-.403.713-.57 1.116-.334.804-.614 1.768-.84 2.734a31.365 31.365 0 00-.613 3.58 2.64 2.64 0 01-.945-1.067c-.328-.68-.398-1.534-.398-2.654A1 1 0 005.05 6.05 6.981 6.981 0 003 11a7 7 0 1011.95-4.95c-.592-.591-.98-.985-1.348-1.467-.363-.476-.724-1.063-1.207-2.03zM12.12 15.12A3 3 0 017 13s.879.5 2.5.5c0-1 .5-4 1.25-4.5.5 1 .786 1.293 1.371 1.879A2.99 2.99 0 0113 13a2.99 2.99 0 01-.879 2.121z" clipRule="evenodd" /></svg>
    },
    {
      title: 'Global Reach',
      description: 'Live in 320+ locations',
      icon: <svg className="w-8 h-8" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM4.332 8.027a6.012 6.012 0 011.912-2.706C6.512 5.73 6.974 6 7.5 6A1.5 1.5 0 019 7.5V8a2 2 0 004 0 2 2 0 011.523-1.943A5.977 5.977 0 0116 10c0 .34-.028.675-.083 1H15a2 2 0 00-2 2v2.197A5.973 5.973 0 0110 16v-2a2 2 0 00-2-2 2 2 0 01-2-2 2 2 0 00-1.668-1.973z" clipRule="evenodd" /></svg>
    },
    {
      title: 'Full-Stack Dev',
      description: 'You\'re officially a builder!',
      icon: <svg className="w-8 h-8" fill="currentColor" viewBox="0 0 20 20"><path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" /></svg>
    }
  ]

  const shareText = isFirstProject 
    ? `I just deployed my first app in ${deploymentTime} seconds with @OpenAgents! 🚀`
    : `Just deployed "${projectName}" in ${deploymentTime} seconds with @OpenAgents! ⚡`

  const content = (
    <div className={cx('relative text-center space-y-8', className)}>
      {/* Confetti */}
      <Confetti active={confettiActive} />

      {/* Main Success Message */}
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.8, 1]]}>
          <div className="space-y-4">
            <div className="inline-flex items-center justify-center w-24 h-24 bg-gradient-to-br from-green-500/20 to-cyan-500/20 border-2 border-green-500 rounded-full mb-4">
              <svg className="w-12 h-12 text-green-400" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
              </svg>
            </div>
            
            <Text as="h1" className="text-4xl font-bold text-white">
              🎉 YOUR APP IS LIVE!
            </Text>
            
            {isFirstProject && (
              <Text as="p" className="text-2xl text-cyan-300">
                Congratulations, you're now a full-stack developer!
              </Text>
            )}
          </div>
        </Animated>
      </Animator>

      {/* Deployment Info */}
      <Animator active={active} duration={{ delay: 0.3 }}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          <div className="inline-block relative text-left">
            <FrameKranox />
            <div className="relative p-6 space-y-4">
              <div>
                <Text as="p" className="text-gray-400 text-sm mb-1">Project Name</Text>
                <Text as="p" className="text-white text-lg font-bold">{projectName}</Text>
              </div>
              
              <div>
                <Text as="p" className="text-gray-400 text-sm mb-1">Live URL</Text>
                <div className="flex items-center gap-2">
                  <DeploymentUrl url={deploymentUrl} />
                  <CopyButton text={deploymentUrl} size="small" />
                </div>
              </div>
              
              <div className="flex items-center gap-6 pt-2">
                <div>
                  <Text as="p" className="text-gray-400 text-sm">Deploy Time</Text>
                  <Text as="p" className="text-cyan-300 text-xl font-bold">{deploymentTime} seconds</Text>
                </div>
                <div>
                  <Text as="p" className="text-gray-400 text-sm">Locations</Text>
                  <Text as="p" className="text-cyan-300 text-xl font-bold">320+ edge</Text>
                </div>
              </div>
            </div>
          </div>
        </Animated>
      </Animator>

      {/* Achievements */}
      {showAchievements && (
        <div className="grid grid-cols-3 gap-4 max-w-2xl mx-auto">
          {achievements.map((achievement, index) => (
            <AchievementBadge
              key={achievement.title}
              {...achievement}
              delay={0.5 + index * 0.1}
            />
          ))}
        </div>
      )}

      {/* Share Options */}
      {showShareOptions && (
        <Animator active={active} duration={{ delay: 0.8 }}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="space-y-3">
              <Text as="p" className="text-gray-400">
                Share your achievement with the world!
              </Text>
              <div className="flex items-center justify-center gap-3">
                <ShareButton platform="twitter" url={deploymentUrl} text={shareText} />
                <ShareButton platform="linkedin" url={deploymentUrl} text={shareText} />
                <ShareButton platform="copy" url={deploymentUrl} text={shareText} />
              </div>
            </div>
          </Animated>
        </Animator>
      )}

      {/* Next Steps */}
      {showNextSteps && (
        <Animator active={active} duration={{ delay: 1 }}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="space-y-4">
              <Text as="h3" className="text-xl text-white font-bold">
                What's next?
              </Text>
              <div className="grid grid-cols-2 gap-4 max-w-2xl mx-auto">
                <button
                  onClick={() => onNextAction?.('edit')}
                  className="p-4 bg-gray-800/50 border border-gray-700 rounded-lg hover:bg-gray-700/50 hover:border-cyan-500/50 transition-all duration-300 group"
                >
                  <svg className="w-8 h-8 text-cyan-400 mx-auto mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                  </svg>
                  <Text as="h4" className="text-white font-bold mb-1">Edit This App</Text>
                  <Text as="p" className="text-gray-400 text-sm">Customize the code</Text>
                </button>
                
                <button
                  onClick={() => onNextAction?.('new')}
                  className="p-4 bg-cyan-500/10 border border-cyan-500/50 rounded-lg hover:bg-cyan-500/20 transition-all duration-300 group"
                >
                  <svg className="w-8 h-8 text-cyan-400 mx-auto mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 4v16m8-8H4" />
                  </svg>
                  <Text as="h4" className="text-white font-bold mb-1">Build Another</Text>
                  <Text as="p" className="text-gray-400 text-sm">Try a different idea</Text>
                </button>
                
                <button
                  onClick={() => onNextAction?.('explore')}
                  className="p-4 bg-gray-800/50 border border-gray-700 rounded-lg hover:bg-gray-700/50 hover:border-purple-500/50 transition-all duration-300 group"
                >
                  <svg className="w-8 h-8 text-purple-400 mx-auto mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M7 4v16M17 4v16M3 8h4m10 0h4M3 16h4m10 0h4" />
                  </svg>
                  <Text as="h4" className="text-white font-bold mb-1">Browse Templates</Text>
                  <Text as="p" className="text-gray-400 text-sm">Get inspired</Text>
                </button>
                
                <button
                  onClick={() => onNextAction?.('gallery')}
                  className="p-4 bg-gray-800/50 border border-gray-700 rounded-lg hover:bg-gray-700/50 hover:border-yellow-500/50 transition-all duration-300 group"
                >
                  <svg className="w-8 h-8 text-yellow-400 mx-auto mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                  <Text as="h4" className="text-white font-bold mb-1">Public Gallery</Text>
                  <Text as="p" className="text-gray-400 text-sm">See what others built</Text>
                </button>
              </div>
            </div>
          </Animated>
        </Animator>
      )}
    </div>
  )

  if (!animated) return content

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      {content}
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Templates/FirstDeploymentCelebration',
  component: FirstDeploymentCelebration,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Enhanced deployment success screen optimized for first-time users. Features confetti, achievements, social sharing, and clear next steps to maintain engagement momentum.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the deployed project'
    },
    deploymentUrl: {
      control: 'text',
      description: 'Live deployment URL'
    },
    deploymentTime: {
      control: { type: 'number', min: 10, max: 120 },
      description: 'Deployment time in seconds'
    },
    showConfetti: {
      control: 'boolean',
      description: 'Show confetti animation'
    },
    showAchievements: {
      control: 'boolean',
      description: 'Show achievement badges'
    },
    showShareOptions: {
      control: 'boolean',
      description: 'Show social sharing options'
    },
    showNextSteps: {
      control: 'boolean',
      description: 'Show next action buttons'
    },
    isFirstProject: {
      control: 'boolean',
      description: 'Is this the user\'s first project?'
    }
  }
} satisfies Meta<typeof FirstDeploymentCelebration>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const ReturningUser: Story = {
  args: {
    isFirstProject: false,
    projectName: 'Todo App with Drag & Drop'
  }
}

export const NoConfetti: Story = {
  args: {
    showConfetti: false
  }
}

export const MinimalVersion: Story = {
  args: {
    showConfetti: false,
    showAchievements: false,
    showShareOptions: false,
    showNextSteps: false
  }
}

export const FastDeploy: Story = {
  args: {
    deploymentTime: 15,
    projectName: 'Lightning Fast App'
  }
}

export const NoAnimation: Story = {
  args: {
    animated: false,
    showConfetti: false
  }
}

export const WithActionHandler: Story = {
  render: () => {
    const [lastAction, setLastAction] = useState<string>()
    
    return (
      <div className="space-y-6">
        <FirstDeploymentCelebration
          onNextAction={(action) => setLastAction(action)}
        />
        {lastAction && (
          <div className="text-center">
            <Text as="p" className="text-cyan-300">
              User selected: {lastAction}
            </Text>
          </div>
        )}
      </div>
    )
  }
}

export const CustomProject: Story = {
  args: {
    projectName: 'AI-Powered Chat Interface',
    deploymentUrl: 'https://ai-chat-demo.openagents.dev',
    deploymentTime: 42
  }
}

export const Playground: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    deploymentUrl: 'https://bitcoin-puns-xyz.openagents.dev',
    deploymentTime: 23,
    showConfetti: true,
    showAchievements: true,
    showShareOptions: true,
    showNextSteps: true,
    isFirstProject: true
  }
}