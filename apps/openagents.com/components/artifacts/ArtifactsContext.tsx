'use client'

import React, { createContext, useContext, useReducer, useEffect } from 'react'
import { ArtifactParameters } from '@/lib/tools/createArtifactTool'

export interface Artifact {
  id: string
  title: string
  description?: string
  type: 'react' | 'html' | 'javascript' | 'typescript' | 'python' | 'css' | 'json' | 'markdown'
  content: string
  language?: string
  deploymentUrl?: string
  createdAt: Date
  updatedAt: Date
  conversationId?: string
  messageId?: string
}

interface ArtifactsState {
  artifacts: Artifact[]
  currentArtifactId?: string
  isLoading: boolean
  isDeploying: string[] // Array of artifact IDs being deployed
}

type ArtifactsAction =
  | { type: 'ADD_ARTIFACT'; payload: Artifact }
  | { type: 'UPDATE_ARTIFACT'; payload: { id: string; updates: Partial<Artifact> } }
  | { type: 'DELETE_ARTIFACT'; payload: string }
  | { type: 'SET_CURRENT_ARTIFACT'; payload: string | undefined }
  | { type: 'SET_LOADING'; payload: boolean }
  | { type: 'START_DEPLOYMENT'; payload: string }
  | { type: 'FINISH_DEPLOYMENT'; payload: string }
  | { type: 'LOAD_ARTIFACTS'; payload: Artifact[] }
  | { type: 'CLEAR_ARTIFACTS' }
  | { type: 'CREATE_FROM_TOOL'; payload: { params: ArtifactParameters; messageId?: string } }
  | { type: 'UPDATE_FROM_TOOL'; payload: { params: ArtifactParameters; messageId?: string } }

const initialState: ArtifactsState = {
  artifacts: [],
  currentArtifactId: undefined,
  isLoading: false,
  isDeploying: []
}

function artifactsReducer(state: ArtifactsState, action: ArtifactsAction): ArtifactsState {
  switch (action.type) {
    case 'ADD_ARTIFACT':
      return {
        ...state,
        artifacts: [...state.artifacts, action.payload],
        currentArtifactId: action.payload.id
      }
    
    case 'UPDATE_ARTIFACT':
      return {
        ...state,
        artifacts: state.artifacts.map(artifact =>
          artifact.id === action.payload.id
            ? { ...artifact, ...action.payload.updates, updatedAt: new Date() }
            : artifact
        )
      }
    
    case 'DELETE_ARTIFACT':
      const newArtifacts = state.artifacts.filter(a => a.id !== action.payload)
      return {
        ...state,
        artifacts: newArtifacts,
        currentArtifactId: state.currentArtifactId === action.payload
          ? newArtifacts[0]?.id
          : state.currentArtifactId
      }
    
    case 'SET_CURRENT_ARTIFACT':
      return {
        ...state,
        currentArtifactId: action.payload
      }
    
    case 'SET_LOADING':
      return {
        ...state,
        isLoading: action.payload
      }
    
    case 'START_DEPLOYMENT':
      return {
        ...state,
        isDeploying: [...state.isDeploying, action.payload]
      }
    
    case 'FINISH_DEPLOYMENT':
      return {
        ...state,
        isDeploying: state.isDeploying.filter(id => id !== action.payload)
      }
    
    case 'LOAD_ARTIFACTS':
      return {
        ...state,
        artifacts: action.payload,
        currentArtifactId: action.payload[0]?.id
      }
    
    case 'CLEAR_ARTIFACTS':
      return {
        ...state,
        artifacts: [],
        currentArtifactId: undefined
      }
    
    case 'CREATE_FROM_TOOL': {
      const { params, messageId } = action.payload
      const id = params.identifier // Use tool identifier directly
      const artifact: Artifact = {
        id,
        title: params.title,
        description: params.description,
        type: params.type,
        content: params.content,
        language: params.language,
        createdAt: new Date(),
        updatedAt: new Date(),
        messageId
      }
      return {
        ...state,
        artifacts: [...state.artifacts, artifact],
        currentArtifactId: id
      }
    }
    
    case 'UPDATE_FROM_TOOL': {
      const { params, messageId } = action.payload
      return {
        ...state,
        artifacts: state.artifacts.map(artifact =>
          artifact.id === params.identifier
            ? {
                ...artifact,
                title: params.title,
                description: params.description || artifact.description,
                content: params.content,
                language: params.language || artifact.language,
                updatedAt: new Date(),
                messageId: messageId || artifact.messageId
              }
            : artifact
        )
      }
    }
    
    default:
      return state
  }
}

interface ArtifactsContextType {
  state: ArtifactsState
  actions: {
    addArtifact: (artifact: Omit<Artifact, 'id' | 'createdAt' | 'updatedAt'>) => string
    updateArtifact: (id: string, updates: Partial<Artifact>) => void
    deleteArtifact: (id: string) => void
    setCurrentArtifact: (id: string | undefined) => void
    deployArtifact: (id: string) => Promise<void>
    clearArtifacts: () => void
    navigateToNext: () => void
    navigateToPrevious: () => void
    getCurrentArtifact: () => Artifact | undefined
    isDeployingArtifact: (id: string) => boolean
    // New tool-based methods
    createArtifactFromTool: (params: ArtifactParameters, messageId?: string) => string
    updateArtifactFromTool: (params: ArtifactParameters, messageId?: string) => void
    getArtifactByIdentifier: (identifier: string) => Artifact | undefined
  }
}

const ArtifactsContext = createContext<ArtifactsContextType | undefined>(undefined)

export function ArtifactsProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(artifactsReducer, initialState)
  const isInitialLoad = React.useRef(true)

  // Load artifacts from localStorage on mount
  useEffect(() => {
    const savedArtifacts = localStorage.getItem('openagents-artifacts')
    if (savedArtifacts) {
      try {
        const artifacts = JSON.parse(savedArtifacts).map((a: any) => ({
          ...a,
          createdAt: new Date(a.createdAt),
          updatedAt: new Date(a.updatedAt)
        }))
        dispatch({ type: 'LOAD_ARTIFACTS', payload: artifacts })
      } catch (error) {
        console.error('Failed to load artifacts from localStorage:', error)
      }
    }
    isInitialLoad.current = false
  }, [])

  // Save artifacts to localStorage whenever they change (but not on initial load)
  useEffect(() => {
    if (!isInitialLoad.current) {
      if (state.artifacts.length > 0) {
        localStorage.setItem('openagents-artifacts', JSON.stringify(state.artifacts))
      } else {
        localStorage.removeItem('openagents-artifacts')
      }
    }
  }, [state.artifacts])

  const actions = {
    addArtifact: (artifactData: Omit<Artifact, 'id' | 'createdAt' | 'updatedAt'>) => {
      const id = `artifact-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
      const artifact: Artifact = {
        ...artifactData,
        id,
        createdAt: new Date(),
        updatedAt: new Date()
      }
      dispatch({ type: 'ADD_ARTIFACT', payload: artifact })
      return id
    },

    updateArtifact: (id: string, updates: Partial<Artifact>) => {
      dispatch({ type: 'UPDATE_ARTIFACT', payload: { id, updates } })
    },

    deleteArtifact: (id: string) => {
      dispatch({ type: 'DELETE_ARTIFACT', payload: id })
    },

    setCurrentArtifact: (id: string | undefined) => {
      dispatch({ type: 'SET_CURRENT_ARTIFACT', payload: id })
    },

    deployArtifact: async (id: string) => {
      const artifact = state.artifacts.find(a => a.id === id)
      if (!artifact) return

      dispatch({ type: 'START_DEPLOYMENT', payload: id })
      
      try {
        // Simulate deployment process
        await new Promise(resolve => setTimeout(resolve, 2000))
        
        // Generate mock deployment URL
        const deploymentUrl = `https://${artifact.title.toLowerCase().replace(/\s+/g, '-')}-${id.slice(-6)}.openagents.dev`
        
        dispatch({ 
          type: 'UPDATE_ARTIFACT', 
          payload: { 
            id, 
            updates: { deploymentUrl } 
          } 
        })
      } catch (error) {
        console.error('Deployment failed:', error)
        throw error
      } finally {
        dispatch({ type: 'FINISH_DEPLOYMENT', payload: id })
      }
    },

    clearArtifacts: () => {
      dispatch({ type: 'CLEAR_ARTIFACTS' })
      localStorage.removeItem('openagents-artifacts')
    },

    navigateToNext: () => {
      const currentIndex = state.artifacts.findIndex(a => a.id === state.currentArtifactId)
      if (currentIndex < state.artifacts.length - 1) {
        dispatch({ type: 'SET_CURRENT_ARTIFACT', payload: state.artifacts[currentIndex + 1].id })
      }
    },

    navigateToPrevious: () => {
      const currentIndex = state.artifacts.findIndex(a => a.id === state.currentArtifactId)
      if (currentIndex > 0) {
        dispatch({ type: 'SET_CURRENT_ARTIFACT', payload: state.artifacts[currentIndex - 1].id })
      }
    },

    getCurrentArtifact: () => {
      return state.artifacts.find(a => a.id === state.currentArtifactId)
    },

    isDeployingArtifact: (id: string) => {
      return state.isDeploying.includes(id)
    },

    // New tool-based methods
    createArtifactFromTool: (params: ArtifactParameters, messageId?: string) => {
      dispatch({ type: 'CREATE_FROM_TOOL', payload: { params, messageId } })
      return params.identifier
    },

    updateArtifactFromTool: (params: ArtifactParameters, messageId?: string) => {
      dispatch({ type: 'UPDATE_FROM_TOOL', payload: { params, messageId } })
    },

    getArtifactByIdentifier: (identifier: string) => {
      return state.artifacts.find(a => a.id === identifier)
    }
  }

  return (
    <ArtifactsContext.Provider value={{ state, actions }}>
      {children}
    </ArtifactsContext.Provider>
  )
}

export function useArtifacts() {
  const context = useContext(ArtifactsContext)
  if (context === undefined) {
    throw new Error('useArtifacts must be used within an ArtifactsProvider')
  }
  return context
}

// Helper hook for current artifact
export function useCurrentArtifact() {
  const { state, actions } = useArtifacts()
  return {
    artifact: actions.getCurrentArtifact(),
    setCurrentArtifact: actions.setCurrentArtifact,
    navigateNext: actions.navigateToNext,
    navigatePrevious: actions.navigateToPrevious
  }
}

// Helper hook for artifact operations
export function useArtifactOperations() {
  const { actions } = useArtifacts()
  return {
    addArtifact: actions.addArtifact,
    updateArtifact: actions.updateArtifact,
    deleteArtifact: actions.deleteArtifact,
    deployArtifact: actions.deployArtifact,
    clearArtifacts: actions.clearArtifacts
  }
}