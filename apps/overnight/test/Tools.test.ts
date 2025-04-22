import { it, describe, expect } from 'vitest';
import { TOOL_SCHEMAS } from '../src/Tools.js';

describe('Tool Schema', () => {
  it('should use the correct format for Anthropic tools', () => {
    // Check if the tools array is defined and not empty
    expect(TOOL_SCHEMAS).toBeDefined();
    expect(TOOL_SCHEMAS.length).toBeGreaterThan(0);
    
    // Check each tool in the schema
    for (const tool of TOOL_SCHEMAS) {
      // Verify the tool has the expected type
      expect(tool.type).toBe('custom');
      
      // Verify the function property is defined
      expect(tool.function).toBeDefined();
      expect(typeof tool.function.name).toBe('string');
      expect(typeof tool.function.description).toBe('string');
      
      // Verify the parameters property is defined with correct format
      expect(tool.function.parameters).toBeDefined();
      expect(tool.function.parameters.type).toBe('object');
      expect(tool.function.parameters.properties).toBeDefined();
      expect(Array.isArray(tool.function.parameters.required)).toBe(true);
    }
    
    // Check the specific tools
    const fileContentTool = TOOL_SCHEMAS.find(
      tool => tool.function.name === 'GetGitHubFileContent'
    );
    expect(fileContentTool).toBeDefined();
    expect(fileContentTool?.function.parameters.properties.owner).toBeDefined();
    expect(fileContentTool?.function.parameters.properties.repo).toBeDefined();
    expect(fileContentTool?.function.parameters.properties.path).toBeDefined();
    expect(fileContentTool?.function.parameters.properties.ref).toBeDefined();
    
    const issueTool = TOOL_SCHEMAS.find(
      tool => tool.function.name === 'GetGitHubIssue'
    );
    expect(issueTool).toBeDefined();
    expect(issueTool?.function.parameters.properties.owner).toBeDefined();
    expect(issueTool?.function.parameters.properties.repo).toBeDefined();
    expect(issueTool?.function.parameters.properties.issueNumber).toBeDefined();
  });
});