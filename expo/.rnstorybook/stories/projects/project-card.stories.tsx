import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { ProjectCard } from '@/components/projects/ProjectCard'
import type { Project } from '@/lib/projects-store'
import { Colors } from '@/constants/theme'

const meta = { title: 'Projects/ProjectCard' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

const project: Project = {
  id: 'proj_1',
  name: 'OpenAgents',
  workingDir: '/Users/you/code/openagents',
  repo: { provider: 'github', remote: 'OpenAgentsInc/openagents', url: 'https://github.com/OpenAgentsInc/openagents', branch: 'main' },
  agentFile: '.openagents/agent.yaml',
  instructions: 'Mobile command center for coding agents.',
  runningAgents: 1,
  attentionCount: 2,
  lastActivity: Date.now() - 1000 * 60 * 10,
  createdAt: Date.now() - 1000 * 60 * 60,
  updatedAt: Date.now() - 1000 * 60 * 5,
}

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ProjectCard project={project} />
    </View>
  ),
}

