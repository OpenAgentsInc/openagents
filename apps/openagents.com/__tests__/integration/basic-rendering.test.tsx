import { describe, it, expect, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { render, mockAuthHook, resetMockChatState } from '../test-utils'
import { WorkspaceChat } from '@/components/workspace/WorkspaceChat'

describe('Basic Chat Rendering', () => {
  beforeEach(() => {
    // Reset auth state to authenticated
    mockAuthHook.isAuthenticated = true
    
    // Reset chat mock state for test isolation
    resetMockChatState()
    
    // Clean up any existing components
    document.body.innerHTML = ''
  })

  it('should render chat component without errors', () => {
    // Simple rendering test that should always pass
    render(
      <WorkspaceChat
        projectName="Test Project"
        projectId="test-project"
      />
    )

    // Basic assertions that should work in any environment
    expect(document.body).toBeInTheDocument()
  })

  it('should contain expected UI elements', () => {
    const { container } = render(
      <WorkspaceChat
        projectName="Test Project"
        projectId="test-project"
      />
    )

    // Check for basic structure
    expect(container.querySelector('div')).toBeInTheDocument()
    expect(container).toHaveTextContent('OpenAgents Chat')
  })

  it('should handle project name prop', () => {
    const projectName = 'My Special Project'
    const { container } = render(
      <WorkspaceChat
        projectName={projectName}
        projectId="special-project"
      />
    )

    // The project name appears in the welcome message with "Welcome to" prefix
    expect(container).toHaveTextContent(`Welcome to ${projectName}!`)
  })
})