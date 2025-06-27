'use client'

import React, { useState, useCallback, useRef } from 'react'
import { Upload, FileText, Folder, X, Check, AlertCircle } from 'lucide-react'
import { cx } from '@arwes/react'

interface FileItem {
  name: string
  size: number
  type: string
  content?: string
}

interface ProjectImportProps {
  onImport?: (files: FileItem[]) => void
  onCancel?: () => void
  className?: string
}

export function ProjectImport({ onImport, onCancel, className }: ProjectImportProps) {
  const [isDragOver, setIsDragOver] = useState(false)
  const [files, setFiles] = useState<FileItem[]>([])
  const [isProcessing, setIsProcessing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragOver(false)
  }, [])

  const processFiles = async (fileList: FileList) => {
    setIsProcessing(true)
    setError(null)

    try {
      const filePromises = Array.from(fileList).map(async (file) => {
        if (file.size > 10 * 1024 * 1024) { // 10MB limit
          throw new Error(`File "${file.name}" is too large (max 10MB)`)
        }

        const content = await file.text()
        return {
          name: file.name,
          size: file.size,
          type: file.type || 'text/plain',
          content
        }
      })

      const processedFiles = await Promise.all(filePromises)
      setFiles(processedFiles)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to process files')
    } finally {
      setIsProcessing(false)
    }
  }

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault()
    setIsDragOver(false)

    const droppedFiles = e.dataTransfer.files
    if (droppedFiles.length > 0) {
      await processFiles(droppedFiles)
    }
  }, [])

  const handleFileSelect = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFiles = e.target.files
    if (selectedFiles && selectedFiles.length > 0) {
      await processFiles(selectedFiles)
    }
  }, [])

  const handleImport = () => {
    if (files.length > 0 && onImport) {
      onImport(files)
    }
  }

  const removeFile = (index: number) => {
    setFiles(prev => prev.filter((_, i) => i !== index))
  }

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
  }

  const getFileIcon = (fileName: string) => {
    const ext = fileName.split('.').pop()?.toLowerCase()
    
    if (['js', 'jsx', 'ts', 'tsx', 'json', 'html', 'css', 'md'].includes(ext || '')) {
      return <FileText size={16} className="text-cyan-400" />
    }
    
    return <FileText size={16} className="text-gray-400" />
  }

  return (
    <div className={cx('bg-black border border-cyan-900/30 rounded-lg p-6', className)}>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h3 className="text-cyan-500 font-mono text-lg uppercase tracking-wider">
            Import Project
          </h3>
          <p className="text-gray-400 text-sm mt-1" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
            Drag and drop files or browse to import your project
          </p>
        </div>
        {onCancel && (
          <button
            onClick={onCancel}
            className="text-gray-500 hover:text-gray-300 transition-colors"
          >
            <X size={20} />
          </button>
        )}
      </div>

      {/* Drop Zone */}
      {files.length === 0 && (
        <div
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          className={cx(
            'border-2 border-dashed rounded-lg p-12 text-center transition-all cursor-pointer',
            isDragOver
              ? 'border-cyan-400 bg-cyan-400/5'
              : 'border-gray-600 hover:border-gray-500 hover:bg-gray-900/20'
          )}
          onClick={() => fileInputRef.current?.click()}
        >
          <input
            ref={fileInputRef}
            type="file"
            multiple
            onChange={handleFileSelect}
            className="hidden"
            accept=".js,.jsx,.ts,.tsx,.json,.html,.css,.md,.txt"
          />
          
          <div className="flex flex-col items-center">
            <Upload size={48} className={cx('mb-4', isDragOver ? 'text-cyan-400' : 'text-gray-500')} />
            
            <div className="text-lg mb-2" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
              {isDragOver ? (
                <span className="text-cyan-400">Drop files here</span>
              ) : (
                <span className="text-gray-300">Drag files here or click to browse</span>
              )}
            </div>
            
            <div className="text-sm text-gray-500" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
              Supports: .js, .jsx, .ts, .tsx, .json, .html, .css, .md
            </div>
            <div className="text-xs text-gray-600 mt-1" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
              Max file size: 10MB
            </div>
          </div>
        </div>
      )}

      {/* Processing State */}
      {isProcessing && (
        <div className="flex items-center justify-center py-12">
          <div className="text-center">
            <div className="animate-spin mb-4">
              <Upload size={32} className="text-cyan-400" />
            </div>
            <div className="text-cyan-400" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
              Processing files...
            </div>
          </div>
        </div>
      )}

      {/* Error State */}
      {error && (
        <div className="bg-red-950/20 border border-red-900/30 rounded-lg p-4 mb-4">
          <div className="flex items-center gap-2">
            <AlertCircle size={16} className="text-red-400" />
            <span className="text-red-400 text-sm" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
              {error}
            </span>
          </div>
        </div>
      )}

      {/* File List */}
      {files.length > 0 && (
        <div className="space-y-4">
          <div className="flex items-center gap-2 mb-4">
            <Folder size={16} className="text-cyan-400" />
            <span className="text-cyan-400 font-mono text-sm uppercase tracking-wider">
              {files.length} file{files.length !== 1 ? 's' : ''} ready to import
            </span>
          </div>

          <div className="max-h-64 overflow-y-auto space-y-2">
            {files.map((file, index) => (
              <div
                key={index}
                className="flex items-center gap-3 p-3 bg-gray-900/30 border border-gray-700/30 rounded-lg"
              >
                {getFileIcon(file.name)}
                
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-gray-200 truncate" style={{ fontFamily: 'var(--font-titillium), sans-serif' }}>
                    {file.name}
                  </div>
                  <div className="text-xs text-gray-500" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>
                    {formatFileSize(file.size)}
                  </div>
                </div>

                <button
                  onClick={() => removeFile(index)}
                  className="text-gray-500 hover:text-red-400 transition-colors"
                >
                  <X size={16} />
                </button>
              </div>
            ))}
          </div>

          {/* Actions */}
          <div className="flex items-center justify-between pt-4 border-t border-gray-700/30">
            <button
              onClick={() => setFiles([])}
              className="text-gray-500 hover:text-gray-300 text-sm font-mono uppercase tracking-wider transition-colors"
            >
              Clear All
            </button>

            <div className="flex gap-3">
              <button
                onClick={() => fileInputRef.current?.click()}
                className="px-4 py-2 bg-gray-800 hover:bg-gray-700 text-gray-300 text-sm font-mono uppercase tracking-wider rounded transition-colors"
              >
                Add More
              </button>
              
              <button
                onClick={handleImport}
                className="px-6 py-2 bg-cyan-600 hover:bg-cyan-500 text-white text-sm font-mono uppercase tracking-wider rounded transition-colors flex items-center gap-2"
              >
                <Check size={16} />
                Import Project
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}