import type { Meta, StoryObj } from '@storybook/nextjs'
import { ProjectImport } from './ProjectImport'

const meta = {
  title: 'Features/Workspace/ProjectImport',
  component: ProjectImport,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Drag-and-drop file upload component for importing project files. Supports multiple file types with preview and validation.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    onImport: { action: 'import' },
    onCancel: { action: 'cancel' }
  }
} satisfies Meta<typeof ProjectImport>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const WithActions: Story = {
  args: {
    onImport: (files) => {
      console.log('Importing files:', files)
      alert(`Importing ${files.length} file(s)`)
    },
    onCancel: () => {
      console.log('Import cancelled')
      alert('Import cancelled')
    }
  }
}

export const FullWidth: Story = {
  args: {
    onImport: (files) => console.log('Files imported:', files)
  },
  render: (args) => (
    <div className="w-full max-w-4xl">
      <ProjectImport {...args} />
    </div>
  )
}

export const InModal: Story = {
  args: {
    onImport: (files) => console.log('Files imported:', files),
    onCancel: () => console.log('Modal closed')
  },
  render: (args) => (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center p-4">
      <div className="max-w-2xl w-full">
        <ProjectImport {...args} />
      </div>
    </div>
  )
}

export const InDarkContainer: Story = {
  args: {
    onImport: (files) => console.log('Files imported:', files)
  },
  render: (args) => (
    <div className="bg-black p-8 min-h-[600px]">
      <ProjectImport {...args} />
    </div>
  )
}