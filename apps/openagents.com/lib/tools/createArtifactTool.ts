import { tool } from 'ai'
import { z } from 'zod'

/**
 * Zod schema for artifact creation/update parameters
 * Based on Claude Artifacts patterns with proper validation
 */
export const artifactParametersSchema = z.object({
  /**
   * Kebab-case identifier for the artifact (e.g., "bitcoin-tracker", "todo-app")
   * Used for updates and referencing. Must be unique within conversation.
   */
  identifier: z
    .string()
    .min(3)
    .max(50)
    .regex(/^[a-z0-9]+(-[a-z0-9]+)*$/, 'Must be kebab-case (lowercase, hyphens only)')
    .describe('Kebab-case identifier for the artifact'),

  /**
   * Human-readable title for the artifact
   */
  title: z
    .string()
    .min(3)
    .max(100)
    .describe('Brief, descriptive title for the artifact'),

  /**
   * Type of artifact being created
   */
  type: z
    .enum(['react', 'html', 'javascript', 'typescript', 'python', 'css', 'json', 'markdown'])
    .describe('Type of artifact content'),

  /**
   * Complete code/content for the artifact
   * Must be substantial (>15 lines for code) and self-contained
   */
  content: z
    .string()
    .min(50)
    .describe('Complete code or content for the artifact'),

  /**
   * Optional language specification for syntax highlighting
   */
  language: z
    .string()
    .optional()
    .describe('Specific language for syntax highlighting (e.g., tsx, jsx)'),

  /**
   * Operation type - create new artifact or update existing
   */
  operation: z
    .enum(['create', 'update'])
    .default('create')
    .describe('Whether to create a new artifact or update an existing one'),

  /**
   * Optional description of what the artifact does
   */
  description: z
    .string()
    .optional()
    .describe('Optional description of the artifact functionality')
})

/**
 * Type for artifact creation parameters
 */
export type ArtifactParameters = z.infer<typeof artifactParametersSchema>

/**
 * Result type for artifact tool execution
 */
export interface ArtifactToolResult {
  success: boolean
  artifactId: string
  operation: 'create' | 'update'
  error?: string
}

/**
 * Validates if content meets artifact creation criteria
 * Based on Claude Artifacts guidelines
 */
export function shouldCreateArtifact(content: string, type: string): boolean {
  // Special handling for different content types
  switch (type) {
    case 'react':
    case 'javascript':
    case 'typescript':
    case 'python':
      // Code files should have at least 15 lines
      const lineCount = content.split('\n').length
      if (lineCount < 15) {
        return false
      }
      break
      
    case 'css':
    case 'json':
    case 'markdown':
      // Non-code files can be shorter but still need substance
      if (content.length < 50) {
        return false
      }
      break
      
    case 'html':
      // HTML should have some structure
      if (content.length < 100) {
        return false
      }
      break
  }

  // Additional validation for React components
  if (type === 'react') {
    const hasExport = content.includes('export default') || content.includes('export {')
    const hasFunction = content.includes('function ') || content.includes('=>') 
    const hasJSX = content.includes('<') && content.includes('>')
    
    // Must be a complete React component
    return hasExport && hasFunction && hasJSX
  }

  // For other types, passed initial checks
  return true
}

/**
 * Creates the artifact creation/update tool
 * This tool is called by AI models to create substantial, self-contained content
 */
export const createArtifactTool = tool({
  description: `Create or update a code artifact that will be displayed in the artifacts panel.

Use this tool for substantial, self-contained content that users might modify or reuse:
- Complete React components (>15 lines)
- Full HTML pages or applications  
- Standalone scripts or programs
- Complete documentation or guides

DO NOT use for:
- Brief code snippets or examples
- Explanatory code that's part of conversation
- Simple one-off demonstrations
- Content dependent on conversation context`,

  parameters: artifactParametersSchema as any,

  execute: async (params, options): Promise<ArtifactToolResult> => {
    try {
      // Validate content meets artifact criteria
      if (!shouldCreateArtifact(params.content, params.type)) {
        return {
          success: false,
          artifactId: '',
          operation: params.operation,
          error: 'Content does not meet artifact criteria (too brief or incomplete)'
        }
      }

      // Note: This will be integrated with ArtifactsContext in the next phase
      // For now, return success to enable testing
      return {
        success: true,
        artifactId: params.identifier,
        operation: params.operation
      }
    } catch (error) {
      return {
        success: false,
        artifactId: '',
        operation: params.operation,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      }
    }
  }
})

/**
 * Type for the complete tool (used in chat endpoints)
 */
export type CreateArtifactTool = typeof createArtifactTool