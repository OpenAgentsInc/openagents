import { z } from 'zod'
import { ArtifactParameters, shouldCreateArtifact } from './createArtifactTool'

/**
 * Interface for parsed artifact from XML tags
 */
export interface ParsedArtifact {
  identifier: string
  title: string
  type: string
  content: string
  language?: string
  operation?: 'create' | 'update'
  description?: string
}

/**
 * Parses artifact tags from AI response content
 * Format: <artifact identifier="id" type="type" title="title">content</artifact>
 * 
 * This provides fallback support for models that don't support tool calls
 */
export function parseArtifactTags(content: string): ParsedArtifact[] {
  const artifacts: ParsedArtifact[] = []
  
  // Regex to match artifact tags with attributes
  // Supports: identifier, type, title (required) + language, operation, description (optional)
  const artifactRegex = /<artifact\s+([^>]+)>([\s\S]*?)<\/artifact>/g
  
  let match
  while ((match = artifactRegex.exec(content)) !== null) {
    const attributesStr = match[1]
    const artifactContent = match[2].trim()
    
    try {
      const attributes = parseAttributes(attributesStr)
      
      // Validate required attributes
      if (!attributes.identifier || !attributes.type || !attributes.title) {
        console.warn('Artifact tag missing required attributes:', attributes)
        continue
      }
      
      // Validate content meets artifact criteria
      if (!shouldCreateArtifact(artifactContent, attributes.type)) {
        console.warn('Artifact content does not meet criteria:', attributes.identifier)
        continue
      }
      
      artifacts.push({
        identifier: attributes.identifier,
        title: attributes.title,
        type: attributes.type,
        content: artifactContent,
        language: attributes.language,
        operation: attributes.operation as 'create' | 'update' || 'create',
        description: attributes.description
      })
    } catch (error) {
      console.warn('Failed to parse artifact tag:', error)
      continue
    }
  }
  
  return artifacts
}

/**
 * Parses attribute string from XML tag
 * Handles quoted values and basic attribute parsing
 */
function parseAttributes(attributesStr: string): Record<string, string> {
  const attributes: Record<string, string> = {}
  
  // Match attribute="value" patterns
  const attrRegex = /(\w+)=["']([^"']*?)["']/g
  
  let match
  while ((match = attrRegex.exec(attributesStr)) !== null) {
    attributes[match[1]] = match[2]
  }
  
  return attributes
}

/**
 * Converts parsed artifact to tool parameters format
 * Validates and transforms XML artifacts to match tool schema
 */
export function convertArtifactToParameters(artifact: ParsedArtifact): ArtifactParameters | null {
  try {
    // Create parameters object matching tool schema
    const params = {
      identifier: artifact.identifier,
      title: artifact.title,
      type: artifact.type,
      content: artifact.content,
      language: artifact.language,
      operation: artifact.operation || 'create',
      description: artifact.description
    }
    
    // Validate against our Zod schema (import it properly)
    // For now, do basic validation
    if (!params.identifier.match(/^[a-z0-9]+(-[a-z0-9]+)*$/)) {
      console.warn('Invalid identifier format:', params.identifier)
      return null
    }
    
    if (!['react', 'html', 'javascript', 'typescript', 'python', 'css', 'json', 'markdown'].includes(params.type)) {
      console.warn('Unsupported artifact type:', params.type)
      return null
    }
    
    return params as ArtifactParameters
  } catch (error) {
    console.warn('Failed to convert artifact to parameters:', error)
    return null
  }
}

/**
 * Processes AI response content for artifacts
 * Returns both tool calls (if any) and parsed XML artifacts
 */
export function processArtifactsFromResponse(content: string): {
  xmlArtifacts: ParsedArtifact[]
  validParameters: ArtifactParameters[]
} {
  const xmlArtifacts = parseArtifactTags(content)
  const validParameters = xmlArtifacts
    .map(convertArtifactToParameters)
    .filter((params): params is ArtifactParameters => params !== null)
  
  return {
    xmlArtifacts,
    validParameters
  }
}

/**
 * Checks if content contains artifact tags
 * Quick check to avoid unnecessary parsing
 */
export function hasArtifactTags(content: string): boolean {
  return content.includes('<artifact') && content.includes('</artifact>')
}