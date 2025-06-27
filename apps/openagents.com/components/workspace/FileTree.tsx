import React, { useState, useMemo } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  FolderIcon, 
  FileIcon, 
  ChevronRightIcon, 
  ChevronDownIcon,
  FileTextIcon,
  FileCodeIcon,
  ImageIcon,
  FileJsonIcon,
  SearchIcon,
  PlusIcon,
  MoreVerticalIcon
} from 'lucide-react'

// Types
export interface FileNode {
  id: string
  name: string
  path: string
  type: 'file' | 'folder'
  extension?: string
  children?: FileNode[]
}

interface FileTreeProps {
  files: FileNode[]
  selectedPath?: string
  onSelectFile?: (path: string) => void
  onCreateFile?: (parentPath: string) => void
  onRenameFile?: (path: string, newName: string) => void
  onDeleteFile?: (path: string) => void
  className?: string
}

// Get appropriate icon for file type
function getFileIcon(extension?: string) {
  if (!extension) return FileIcon
  
  const iconMap: Record<string, React.ComponentType<{ className?: string }>> = {
    // Code files
    'js': FileCodeIcon,
    'jsx': FileCodeIcon,
    'ts': FileCodeIcon,
    'tsx': FileCodeIcon,
    'css': FileCodeIcon,
    'scss': FileCodeIcon,
    'html': FileCodeIcon,
    'py': FileCodeIcon,
    'go': FileCodeIcon,
    'rs': FileCodeIcon,
    
    // Text files
    'md': FileTextIcon,
    'txt': FileTextIcon,
    'log': FileTextIcon,
    
    // Data files
    'json': FileJsonIcon,
    'yaml': FileJsonIcon,
    'yml': FileJsonIcon,
    'toml': FileJsonIcon,
    
    // Images
    'png': ImageIcon,
    'jpg': ImageIcon,
    'jpeg': ImageIcon,
    'gif': ImageIcon,
    'svg': ImageIcon,
    'webp': ImageIcon,
  }
  
  return iconMap[extension] || FileIcon
}

// File tree node component
function FileTreeNode({
  node,
  level = 0,
  selectedPath,
  expandedFolders,
  onToggleFolder,
  onSelectFile,
  onContextMenu,
}: {
  node: FileNode
  level: number
  selectedPath?: string
  expandedFolders: Set<string>
  onToggleFolder: (path: string) => void
  onSelectFile?: (path: string) => void
  onContextMenu?: (e: React.MouseEvent, node: FileNode) => void
}) {
  const isExpanded = expandedFolders.has(node.path)
  const isSelected = selectedPath === node.path
  const [isHovered, setIsHovered] = useState(false)
  
  const handleClick = () => {
    if (node.type === 'folder') {
      onToggleFolder(node.path)
    } else {
      onSelectFile?.(node.path)
    }
  }
  
  const Icon = node.type === 'folder' 
    ? (isExpanded ? ChevronDownIcon : ChevronRightIcon)
    : getFileIcon(node.extension)
  
  return (
    <>
      <div
        className={cx(
          'group flex items-center gap-1 px-2 py-1 cursor-pointer transition-all',
          'hover:bg-cyan-500/10',
          isSelected && 'bg-cyan-500/20',
          isHovered && 'bg-cyan-500/5'
        )}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={handleClick}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        onContextMenu={(e) => onContextMenu?.(e, node)}
      >
        <Icon className={cx(
          'w-4 h-4 flex-shrink-0',
          node.type === 'folder' ? 'text-cyan-400' : 'text-cyan-300/70'
        )} />
        
        <Text className={cx(
          'text-sm truncate flex-1 font-mono',
          isSelected ? 'text-cyan-300' : 'text-gray-300'
        )}>
          {node.name}
        </Text>
        
        {isHovered && (
          <MoreVerticalIcon className="w-4 h-4 text-gray-400 opacity-0 group-hover:opacity-100 transition-opacity" />
        )}
      </div>
      
      {node.type === 'folder' && isExpanded && node.children && (
        <div>
          {node.children.map(child => (
            <FileTreeNode
              key={child.id}
              node={child}
              level={level + 1}
              selectedPath={selectedPath}
              expandedFolders={expandedFolders}
              onToggleFolder={onToggleFolder}
              onSelectFile={onSelectFile}
              onContextMenu={onContextMenu}
            />
          ))}
        </div>
      )}
    </>
  )
}

// Search input component
function SearchInput({ 
  value, 
  onChange 
}: { 
  value: string
  onChange: (value: string) => void 
}) {
  return (
    <div className="relative">
      <SearchIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Search files..."
        className={cx(
          'w-full pl-8 pr-2 py-1.5 bg-black/50 border border-cyan-900/30',
          'text-sm text-cyan-300 placeholder-gray-500 font-mono',
          'focus:outline-none focus:border-cyan-500/50',
          'transition-colors'
        )}
      />
    </div>
  )
}

// Filter file tree based on search query
function filterTree(nodes: FileNode[], query: string): FileNode[] {
  if (!query) return nodes
  
  const lowerQuery = query.toLowerCase()
  
  return nodes.reduce<FileNode[]>((filtered, node) => {
    const nameMatches = node.name.toLowerCase().includes(lowerQuery)
    
    if (node.type === 'file') {
      if (nameMatches) {
        filtered.push(node)
      }
    } else if (node.children) {
      const filteredChildren = filterTree(node.children, query)
      if (filteredChildren.length > 0 || nameMatches) {
        filtered.push({
          ...node,
          children: filteredChildren
        })
      }
    }
    
    return filtered
  }, [])
}

// Main FileTree component
export function FileTree({
  files,
  selectedPath,
  onSelectFile,
  onCreateFile,
  onRenameFile,
  onDeleteFile,
  className = ''
}: FileTreeProps) {
  const [searchQuery, setSearchQuery] = useState('')
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set(['/']) // Root expanded by default
  )
  const [contextMenu, setContextMenu] = useState<{
    x: number
    y: number
    node: FileNode
  } | null>(null)
  
  const filteredFiles = useMemo(
    () => filterTree(files, searchQuery),
    [files, searchQuery]
  )
  
  const handleToggleFolder = (path: string) => {
    setExpandedFolders(prev => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }
  
  const handleContextMenu = (e: React.MouseEvent, node: FileNode) => {
    e.preventDefault()
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      node
    })
  }
  
  const closeContextMenu = () => {
    setContextMenu(null)
  }
  
  return (
    <div className={cx('h-full flex flex-col bg-black/50', className)}>
      {/* Header */}
      <div className="flex items-center justify-between p-2 border-b border-cyan-900/30">
        <Text className="text-sm font-medium text-cyan-300 font-sans">Files</Text>
        <button
          onClick={() => onCreateFile?.('/')}
          className="p-1 text-gray-400 hover:text-cyan-300 transition-colors"
          title="New file"
        >
          <PlusIcon className="w-4 h-4" />
        </button>
      </div>
      
      {/* Search */}
      <div className="p-2 border-b border-cyan-900/30">
        <SearchInput value={searchQuery} onChange={setSearchQuery} />
      </div>
      
      {/* File tree */}
      <div 
        className="flex-1 overflow-y-auto overflow-x-hidden"
        onClick={closeContextMenu}
      >
        {filteredFiles.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-500 text-sm font-sans">
            {searchQuery ? 'No files found' : 'No files yet'}
          </div>
        ) : (
          <div className="py-1">
            {filteredFiles.map(node => (
              <FileTreeNode
                key={node.id}
                node={node}
                level={0}
                selectedPath={selectedPath}
                expandedFolders={expandedFolders}
                onToggleFolder={handleToggleFolder}
                onSelectFile={onSelectFile}
                onContextMenu={handleContextMenu}
              />
            ))}
          </div>
        )}
      </div>
      
      {/* Context menu */}
      {contextMenu && (
        <div
          className={cx(
            'fixed z-50 bg-black border border-cyan-500/30',
            'shadow-lg shadow-cyan-500/10 py-1 min-w-[150px]'
          )}
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            className="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-cyan-500/20 hover:text-cyan-300 transition-colors font-sans"
            onClick={() => {
              onRenameFile?.(contextMenu.node.path, contextMenu.node.name)
              closeContextMenu()
            }}
          >
            Rename
          </button>
          <button
            className="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-cyan-500/20 hover:text-cyan-300 transition-colors font-sans"
            onClick={() => {
              onDeleteFile?.(contextMenu.node.path)
              closeContextMenu()
            }}
          >
            Delete
          </button>
          {contextMenu.node.type === 'folder' && (
            <>
              <div className="h-px bg-cyan-900/30 my-1" />
              <button
                className="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-cyan-500/20 hover:text-cyan-300 transition-colors font-sans"
                onClick={() => {
                  onCreateFile?.(contextMenu.node.path)
                  closeContextMenu()
                }}
              >
                New File
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}