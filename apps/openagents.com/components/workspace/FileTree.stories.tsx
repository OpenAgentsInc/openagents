import type { Meta, StoryObj } from '@storybook/nextjs'
import React from 'react'
import { FileTree, FileNode } from './FileTree'

// Sample file tree data
const sampleFiles: FileNode[] = [
  {
    id: '1',
    name: 'src',
    path: '/src',
    type: 'folder',
    children: [
      {
        id: '2',
        name: 'components',
        path: '/src/components',
        type: 'folder',
        children: [
          {
            id: '3',
            name: 'Button.tsx',
            path: '/src/components/Button.tsx',
            type: 'file',
            extension: 'tsx'
          },
          {
            id: '4',
            name: 'Card.tsx',
            path: '/src/components/Card.tsx',
            type: 'file',
            extension: 'tsx'
          },
          {
            id: '5',
            name: 'Header.tsx',
            path: '/src/components/Header.tsx',
            type: 'file',
            extension: 'tsx'
          }
        ]
      },
      {
        id: '6',
        name: 'hooks',
        path: '/src/hooks',
        type: 'folder',
        children: [
          {
            id: '7',
            name: 'useAuth.ts',
            path: '/src/hooks/useAuth.ts',
            type: 'file',
            extension: 'ts'
          },
          {
            id: '8',
            name: 'useTheme.ts',
            path: '/src/hooks/useTheme.ts',
            type: 'file',
            extension: 'ts'
          }
        ]
      },
      {
        id: '9',
        name: 'App.tsx',
        path: '/src/App.tsx',
        type: 'file',
        extension: 'tsx'
      },
      {
        id: '10',
        name: 'index.css',
        path: '/src/index.css',
        type: 'file',
        extension: 'css'
      },
      {
        id: '11',
        name: 'main.tsx',
        path: '/src/main.tsx',
        type: 'file',
        extension: 'tsx'
      }
    ]
  },
  {
    id: '12',
    name: 'public',
    path: '/public',
    type: 'folder',
    children: [
      {
        id: '13',
        name: 'favicon.ico',
        path: '/public/favicon.ico',
        type: 'file',
        extension: 'ico'
      },
      {
        id: '14',
        name: 'logo.svg',
        path: '/public/logo.svg',
        type: 'file',
        extension: 'svg'
      }
    ]
  },
  {
    id: '15',
    name: 'package.json',
    path: '/package.json',
    type: 'file',
    extension: 'json'
  },
  {
    id: '16',
    name: 'README.md',
    path: '/README.md',
    type: 'file',
    extension: 'md'
  },
  {
    id: '17',
    name: 'tsconfig.json',
    path: '/tsconfig.json',
    type: 'file',
    extension: 'json'
  },
  {
    id: '18',
    name: 'vite.config.ts',
    path: '/vite.config.ts',
    type: 'file',
    extension: 'ts'
  }
]

const meta = {
  title: 'Workspace/FileTree',
  component: FileTree,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'File tree component for browsing and managing project files with search, context menus, and file type icons.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    files: {
      control: 'object',
      description: 'Array of file nodes representing the file tree structure'
    },
    selectedPath: {
      control: 'text',
      description: 'Currently selected file path'
    },
    onSelectFile: {
      action: 'selectFile',
      description: 'Callback when a file is selected'
    },
    onCreateFile: {
      action: 'createFile',
      description: 'Callback when creating a new file'
    },
    onRenameFile: {
      action: 'renameFile',
      description: 'Callback when renaming a file'
    },
    onDeleteFile: {
      action: 'deleteFile',
      description: 'Callback when deleting a file'
    }
  }
} satisfies Meta<typeof FileTree>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    files: sampleFiles,
    selectedPath: '/src/App.tsx'
  }
}

export const Empty: Story = {
  args: {
    files: []
  }
}

export const SingleFolder: Story = {
  args: {
    files: [
      {
        id: '1',
        name: 'src',
        path: '/src',
        type: 'folder',
        children: [
          {
            id: '2',
            name: 'index.js',
            path: '/src/index.js',
            type: 'file',
            extension: 'js'
          }
        ]
      }
    ]
  }
}

export const DeepNesting: Story = {
  args: {
    files: [
      {
        id: '1',
        name: 'src',
        path: '/src',
        type: 'folder',
        children: [
          {
            id: '2',
            name: 'components',
            path: '/src/components',
            type: 'folder',
            children: [
              {
                id: '3',
                name: 'common',
                path: '/src/components/common',
                type: 'folder',
                children: [
                  {
                    id: '4',
                    name: 'Button',
                    path: '/src/components/common/Button',
                    type: 'folder',
                    children: [
                      {
                        id: '5',
                        name: 'Button.tsx',
                        path: '/src/components/common/Button/Button.tsx',
                        type: 'file',
                        extension: 'tsx'
                      },
                      {
                        id: '6',
                        name: 'Button.test.tsx',
                        path: '/src/components/common/Button/Button.test.tsx',
                        type: 'file',
                        extension: 'tsx'
                      },
                      {
                        id: '7',
                        name: 'Button.module.css',
                        path: '/src/components/common/Button/Button.module.css',
                        type: 'file',
                        extension: 'css'
                      }
                    ]
                  }
                ]
              }
            ]
          }
        ]
      }
    ],
    selectedPath: '/src/components/common/Button/Button.tsx'
  }
}

export const MixedFileTypes: Story = {
  args: {
    files: [
      {
        id: '1',
        name: 'project',
        path: '/project',
        type: 'folder',
        children: [
          {
            id: '2',
            name: 'script.py',
            path: '/project/script.py',
            type: 'file',
            extension: 'py'
          },
          {
            id: '3',
            name: 'data.json',
            path: '/project/data.json',
            type: 'file',
            extension: 'json'
          },
          {
            id: '4',
            name: 'config.yaml',
            path: '/project/config.yaml',
            type: 'file',
            extension: 'yaml'
          },
          {
            id: '5',
            name: 'notes.md',
            path: '/project/notes.md',
            type: 'file',
            extension: 'md'
          },
          {
            id: '6',
            name: 'image.png',
            path: '/project/image.png',
            type: 'file',
            extension: 'png'
          },
          {
            id: '7',
            name: 'styles.scss',
            path: '/project/styles.scss',
            type: 'file',
            extension: 'scss'
          }
        ]
      }
    ]
  }
}

export const LargeProject: Story = {
  args: {
    files: [
      {
        id: '1',
        name: 'src',
        path: '/src',
        type: 'folder',
        children: Array.from({ length: 20 }, (_, i) => ({
          id: `file-${i}`,
          name: `Component${i}.tsx`,
          path: `/src/Component${i}.tsx`,
          type: 'file' as const,
          extension: 'tsx'
        }))
      },
      {
        id: '2',
        name: 'tests',
        path: '/tests',
        type: 'folder',
        children: Array.from({ length: 20 }, (_, i) => ({
          id: `test-${i}`,
          name: `Component${i}.test.tsx`,
          path: `/tests/Component${i}.test.tsx`,
          type: 'file' as const,
          extension: 'tsx'
        }))
      }
    ]
  }
}

export const Interactive: Story = {
  args: {
    files: sampleFiles
  },
  render: (args) => {
    const [selectedPath, setSelectedPath] = React.useState<string>('/src/App.tsx')
    
    return (
      <div className="h-[600px] w-[300px] border border-cyan-900/30">
        <FileTree
          {...args}
          selectedPath={selectedPath}
          onSelectFile={(path) => {
            setSelectedPath(path)
            console.log('Selected file:', path)
          }}
        />
      </div>
    )
  }
}

export const WithContainer: Story = {
  args: {
    files: sampleFiles,
    selectedPath: '/src/components/Button.tsx'
  },
  render: (args) => (
    <div className="h-[600px] w-[300px] bg-black border border-cyan-500/20">
      <FileTree {...args} />
    </div>
  )
}

export const Playground: Story = {
  args: {
    files: sampleFiles,
    selectedPath: undefined
  }
}