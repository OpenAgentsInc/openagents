import { it, describe, expect } from 'vitest';
import { TOOL_SCHEMAS } from '../src/Tools.js';

describe('Tool Schema', () => {
  it('should use the correct format for Anthropic tools', () => {
    // Check if the tools array is defined and not empty
    expect(TOOL_SCHEMAS).toBeDefined();
    expect(TOOL_SCHEMAS.length).toBeGreaterThan(0);
    
    // Check each tool in the schema
    for (const tool of TOOL_SCHEMAS) {
      // Verify the tool name and description
      expect(typeof tool.name).toBe('string');
      expect(typeof tool.description).toBe('string');
      
      // Verify the input_schema property is defined with correct format
      expect(tool.input_schema).toBeDefined();
      expect(tool.input_schema.type).toBe('object');
      expect(tool.input_schema.properties).toBeDefined();
      expect(Array.isArray(tool.input_schema.required)).toBe(true);
    }
    
    // Check the specific tools
    const fileContentTool = TOOL_SCHEMAS.find(
      tool => tool.name === 'GetGitHubFileContent'
    );
    expect(fileContentTool).toBeDefined();
    expect(fileContentTool?.input_schema.properties.owner).toBeDefined();
    expect(fileContentTool?.input_schema.properties.repo).toBeDefined();
    expect(fileContentTool?.input_schema.properties.path).toBeDefined();
    expect(fileContentTool?.input_schema.properties.ref).toBeDefined();
    
    const issueTool = TOOL_SCHEMAS.find(
      tool => tool.name === 'GetGitHubIssue'
    );
    expect(issueTool).toBeDefined();
    expect(issueTool?.input_schema.properties.owner).toBeDefined();
    expect(issueTool?.input_schema.properties.repo).toBeDefined();
    expect(issueTool?.input_schema.properties.issueNumber).toBeDefined();
  });
});