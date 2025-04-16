# Agent Implementation Guide

This guide provides detailed instructions for implementing new agent types in the OpenAgents framework. It covers best practices, required components, and integration steps.

## Agent Structure Overview

Each agent in the OpenAgents framework consists of several key components:

1. **Agent Class** - The main implementation that extends the base `Agent` class
2. **State Interface** - TypeScript interface defining the agent's state
3. **Tools** - Specialized functions that the agent can use to perform tasks
4. **Prompts** - System prompts and instructions for the agent's behavior

## Implementation Steps

### 1. Define Your Agent's State

Start by defining the state interface for your agent. This should extend the `BaseAgentState` interface:

```typescript
// src/agents/myagent/types.ts
import { BaseAgentState } from '../../common/types';

export interface MyAgentState extends BaseAgentState {
  // Add agent-specific state properties
  currentTask?: Task;
  taskHistory?: Task[];
  preferences?: UserPreferences;
  // Add any other state needed by your agent
}

// Define additional types used by your agent
export interface Task {
  id: string;
  description: string;
  status: 'pending' | 'in-progress' | 'completed' | 'failed';
  created: Date;
  updated?: Date;
  result?: string;
}

export interface UserPreferences {
  language: string;
  detailLevel: 'concise' | 'detailed';
  // Add other user preferences
}
```

### 2. Create Your Agent Class

Implement your agent class by extending the base `Agent` class:

```typescript
// src/agents/myagent/index.ts
import { Agent } from '../../common/agent';
import { generateId, UIMessage } from 'ai';
import { MyAgentState, Task } from './types';
import { myAgentSystemPrompt } from './prompts';
import { 
  fetchDataTool, 
  analyzeDataTool,
  generateReportTool 
} from './tools';

export class MyAgent extends Agent<Env, MyAgentState> {
  // Initialize state with default values
  initialState: MyAgentState = {
    messages: [],
    githubToken: undefined,
    taskHistory: [],
    preferences: {
      language: 'english',
      detailLevel: 'concise'
    }
  };
  
  // Implement agent-specific methods
  async setCurrentTask(task: Task): Promise<void> {
    this.setState({ currentTask: task });
  }
  
  async completeTask(result: string): Promise<void> {
    const state = this.getState();
    if (!state.currentTask) return;
    
    const completedTask = {
      ...state.currentTask,
      status: 'completed' as const,
      updated: new Date(),
      result
    };
    
    this.setState({ 
      currentTask: undefined,
      taskHistory: [...(state.taskHistory || []), completedTask]
    });
  }
  
  // Main inference method (required)
  async infer(token?: string): Promise<AIResponse> {
    // 1. Get current state
    const state = this.getState();
    
    // 2. Set up context
    const context = {
      messages: state.messages,
      currentTask: state.currentTask,
      taskHistory: state.taskHistory || [],
      preferences: state.preferences
    };
    
    // 3. Generate response using LLM
    const tools = [fetchDataTool, analyzeDataTool, generateReportTool];
    const systemPrompt = myAgentSystemPrompt;
    
    const response = await this.callLLM({
      messages: this.formatMessagesForLLM(state.messages),
      tools,
      systemPrompt,
      temperature: 0.2,
      token
    });
    
    // 4. Update state with response
    this.addMessage({
      id: generateId(),
      role: 'assistant',
      content: response.content
    });
    
    return response;
  }
  
  // Helper methods
  private formatMessagesForLLM(messages: UIMessage[]): FormattedMessage[] {
    // Transform UI messages to the format expected by the LLM
    return messages.map(msg => ({
      role: msg.role,
      content: msg.content
    }));
  }
}
```

### 3. Implement Agent Tools

Create specialized tools for your agent's functionality:

```typescript
// src/agents/myagent/tools.ts
import { Tool } from '../../common/types';

export const fetchDataTool: Tool = {
  name: 'fetchData',
  description: 'Fetches data from a specified source',
  parameters: {
    source: {
      type: 'string',
      description: 'The source to fetch data from (API URL, database, etc.)'
    },
    query: {
      type: 'string',
      description: 'The query parameters for the data fetch'
    }
  },
  execute: async ({ source, query }, agent) => {
    try {
      // Implementation to fetch data from the source
      const data = await fetchFromSource(source, query);
      return { success: true, data };
    } catch (error) {
      return { 
        success: false, 
        error: error instanceof Error ? error.message : 'Unknown error' 
      };
    }
  }
};

export const analyzeDataTool: Tool = {
  name: 'analyzeData',
  description: 'Analyzes provided data and returns insights',
  parameters: {
    data: {
      type: 'object',
      description: 'The data to analyze'
    },
    analysisType: {
      type: 'string',
      description: 'The type of analysis to perform'
    }
  },
  execute: async ({ data, analysisType }, agent) => {
    try {
      // Implementation to analyze the data
      const analysis = await analyzeData(data, analysisType);
      return { success: true, analysis };
    } catch (error) {
      return { 
        success: false, 
        error: error instanceof Error ? error.message : 'Unknown error' 
      };
    }
  }
};

export const generateReportTool: Tool = {
  name: 'generateReport',
  description: 'Generates a report from analyzed data',
  parameters: {
    analysis: {
      type: 'object',
      description: 'The analysis results'
    },
    format: {
      type: 'string',
      description: 'The format of the report (markdown, html, etc.)'
    }
  },
  execute: async ({ analysis, format }, agent) => {
    try {
      // Implementation to generate a report
      const report = await generateReport(analysis, format);
      
      // Update agent state
      const state = agent.getState();
      if (state.currentTask) {
        await agent.completeTask(report);
      }
      
      return { success: true, report };
    } catch (error) {
      return { 
        success: false, 
        error: error instanceof Error ? error.message : 'Unknown error' 
      };
    }
  }
};

// Helper functions
async function fetchFromSource(source: string, query: string) {
  // Implementation
}

async function analyzeData(data: any, analysisType: string) {
  // Implementation
}

async function generateReport(analysis: any, format: string) {
  // Implementation
}
```

### 4. Define Agent Prompts

Create system prompts that define your agent's behavior:

```typescript
// src/agents/myagent/prompts.ts

export const myAgentSystemPrompt = `
You are an AI assistant specialized in data analysis and reporting.

Your capabilities include:
- Fetching data from various sources
- Analyzing data to extract insights
- Generating reports based on the analysis
- Adapting to user preferences and needs

Guidelines:
1. Always clarify the user's requirements before starting a task
2. Provide progress updates during complex operations
3. Summarize your findings in a clear, structured manner
4. Adapt your response detail based on the user's preference setting
5. When handling errors, explain the issue and suggest alternatives

Use the available tools to complete tasks:
- fetchData: Use this to retrieve data from specified sources
- analyzeData: Use this to analyze data and extract insights
- generateReport: Use this to create formatted reports from analysis results

Remember to maintain context across interactions and reference previous tasks when relevant.
`;

export const myAgentErrorPrompt = `
I encountered an error while processing your request. Here's what happened:

{error}

Here are some steps we can take to resolve this:
1. {suggestion1}
2. {suggestion2}
3. {suggestion3}

Would you like me to try one of these approaches, or would you prefer a different solution?
`;
```

### 5. Update Server Configuration

Add your agent to the server configuration:

```typescript
// src/server.ts
import { Coder } from './agents/coder';
import { Solver } from './agents/solver';
import { MyAgent } from './agents/myagent';
import { routeAgentRequest } from './common/router';

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const agentConfig = {
      agents: {
        'coder': Coder,
        'solver': Solver,
        'myagent': MyAgent  // Add your new agent here
      },
      cors: true
    };

    return (
      (await routeAgentRequest(request, env, agentConfig)) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
```

### 6. Update the useOpenAgent Hook

Extend the `useOpenAgent` hook to support your new agent type:

```typescript
// packages/core/src/agents/useOpenAgent.ts
import { useAgent } from "agents/react";
import { useState } from "react";
import { generateId, UIMessage } from "ai";

// Update the agent type to include your new agent
type AgentType = 'coder' | 'solver' | 'myagent';

export type OpenAgent = {
  state: AgentState;
  messages: UIMessage[];
  setMessages: (messages: UIMessage[]) => void;
  handleSubmit: (message: string) => void;
  infer: (token: string) => Promise<any>;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
  
  // Add agent-specific methods
  setCurrentIssue?: (issue: any) => Promise<void>;  // For Solver
  setCurrentTask?: (task: any) => Promise<void>;    // For MyAgent
  completeTask?: (result: string) => Promise<void>; // For MyAgent
}

export function useOpenAgent(id: string, type: AgentType = "coder"): OpenAgent {
  // Existing implementation...
  
  // Add methods for your agent
  const setCurrentTask = async (task: any): Promise<void> => {
    if (type === 'myagent') {
      await cloudflareAgent.call('setCurrentTask', [task])
    }
    return
  }
  
  const completeTask = async (result: string): Promise<void> => {
    if (type === 'myagent') {
      await cloudflareAgent.call('completeTask', [result])
    }
    return
  }

  return {
    // Existing properties...
    
    // Conditionally include agent-specific methods
    ...(type === 'solver' ? { setCurrentIssue } : {}),
    ...(type === 'myagent' ? { setCurrentTask, completeTask } : {})
  };
}
```

### 7. Create a UI Component

Create a UI component to interact with your agent:

```typescript
// apps/website/app/components/agent/my-agent-connector.tsx
import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { Terminal, BotIcon, PlayIcon, Power, CheckCircle, AlertTriangle } from "lucide-react";
import { useOpenAgent } from "@openagents/core/agents/useOpenAgent";

interface MyAgentConnectorProps {
  sessionId: string;
  apiToken: string;
}

// Connection states for the agent
type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';

export function MyAgentConnector({ sessionId, apiToken }: MyAgentConnectorProps) {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isStartingAgent, setIsStartingAgent] = useState(false);

  // Use the OpenAgent hook to connect to the agent
  const agent = useOpenAgent(`myagent-${sessionId}`, "myagent");

  // Handle connection to the agent
  const connectToAgent = async () => {
    if (!apiToken) {
      setErrorMessage("API token is required.");
      setConnectionState('error');
      return;
    }

    setConnectionState('connecting');
    setIsStartingAgent(true);

    try {
      // Set API token
      await agent.setGithubToken(apiToken);
      
      // Initial message
      await agent.handleSubmit(`Hello! I need assistance with data analysis.`);
      
      // Start inference
      await agent.infer(apiToken);
      
      setConnectionState('connected');
    } catch (err) {
      console.error("Error connecting to agent:", err);
      setErrorMessage(err instanceof Error ? err.message : "Failed to connect to the agent. Please try again later.");
      setConnectionState('error');
    } finally {
      setIsStartingAgent(false);
    }
  };
  
  // Check if the connect button should be disabled
  const isConnectButtonDisabled = isStartingAgent || !apiToken;

  // Disconnect from the agent
  const disconnectFromAgent = () => {
    agent.setMessages([]);
    setConnectionState('disconnected');
  };

  // Update connection state based on changes in agent state
  useEffect(() => {
    if (agent.messages.length > 0 && connectionState === 'disconnected') {
      setConnectionState('connected');
    }
  }, [agent.messages, connectionState]);

  // Render component UI
  return (
    <Card className="mb-6">
      <CardHeader className="pb-2">
        <div className="flex justify-between items-center">
          <CardTitle className="text-lg flex items-center">
            <BotIcon className="h-5 w-5 mr-2" />
            Data Analysis Agent
          </CardTitle>
          <Badge
            variant={
              connectionState === 'connected' ? "success" :
                connectionState === 'connecting' ? "warning" :
                  connectionState === 'error' ? "destructive" : "secondary"
            }
          >
            {connectionState === 'connected' ? "Connected" :
              connectionState === 'connecting' ? "Connecting..." :
                connectionState === 'error' ? "Error" : "Disconnected"}
          </Badge>
        </div>
      </CardHeader>

      <CardContent>
        {/* Render appropriate content based on connection state */}
        {connectionState === 'disconnected' && (
          <div className="text-center py-6">
            <Terminal className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
            <h3 className="text-lg font-medium mb-2">Data Analysis Agent Disconnected</h3>
            <p className="text-muted-foreground mb-4">
              Connect to get assistance with data analysis, insights, and reporting.
            </p>
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="text-center py-6">
            <Spinner className="h-12 w-12 mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">Connecting to Agent</h3>
            <p className="text-muted-foreground">
              Establishing connection and initializing data analysis capabilities...
            </p>
          </div>
        )}

        {connectionState === 'error' && (
          <div className="text-center py-6">
            <AlertTriangle className="h-12 w-12 mx-auto mb-4 text-destructive" />
            <h3 className="text-lg font-medium mb-2">Connection Error</h3>
            <p className="text-muted-foreground mb-4">
              {errorMessage || "Failed to connect to the agent. Please try again."}
            </p>
          </div>
        )}

        {connectionState === 'connected' && (
          <div className="py-2">
            <div className="flex items-center mb-4">
              <CheckCircle className="h-5 w-5 mr-2 text-green-500" />
              <span className="font-medium">Data Analysis Agent Connected</span>
            </div>

            {agent.messages.length > 1 && (
              <div className="border rounded-md p-3 mb-4 bg-muted/50">
                <h4 className="font-medium mb-2">Latest Update:</h4>
                <p className="text-sm">
                  {agent.messages[agent.messages.length - 1].content.substring(0, 150)}
                  {agent.messages[agent.messages.length - 1].content.length > 150 ? '...' : ''}
                </p>
              </div>
            )}

            <div className="flex flex-col gap-2">
              <Button size="sm" variant="outline" 
                     onClick={() => window.open(`/dashboard/analysis?session=${sessionId}`, '_blank')}>
                <PlayIcon className="h-4 w-4 mr-2" />
                Open Full Analysis Interface
              </Button>
            </div>
          </div>
        )}
      </CardContent>

      <CardFooter className="flex justify-end">
        {connectionState === 'disconnected' && (
          isConnectButtonDisabled ? (
            <Button
              variant="secondary"
              className="opacity-50 cursor-not-allowed"
            >
              <PlayIcon className="h-4 w-4 mr-2" />
              Connect to Agent
            </Button>
          ) : (
            <Button
              onClick={connectToAgent}
            >
              <PlayIcon className="h-4 w-4 mr-2" />
              Connect to Agent
            </Button>
          )
        )}

        {(connectionState === 'connected' || connectionState === 'error') && (
          <Button
            variant="outline"
            onClick={disconnectFromAgent}
          >
            <Power className="h-4 w-4 mr-2" />
            Disconnect
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
```

## Testing Your Agent

### 1. Unit Testing

Create unit tests for your agent's methods and tools:

```typescript
// tests/agents/myagent/index.test.ts
import { MyAgent } from '../../../src/agents/myagent';
import { Task } from '../../../src/agents/myagent/types';

describe('MyAgent', () => {
  let agent: MyAgent;
  
  beforeEach(() => {
    agent = new MyAgent();
  });
  
  test('initialState should be properly initialized', () => {
    expect(agent.initialState).toHaveProperty('messages');
    expect(agent.initialState).toHaveProperty('preferences');
    expect(agent.initialState.preferences?.language).toBe('english');
  });
  
  test('setCurrentTask should update state', async () => {
    const task: Task = {
      id: '123',
      description: 'Test task',
      status: 'pending',
      created: new Date()
    };
    
    await agent.setCurrentTask(task);
    
    const state = agent.getState();
    expect(state.currentTask).toEqual(task);
  });
  
  test('completeTask should update state correctly', async () => {
    const task: Task = {
      id: '123',
      description: 'Test task',
      status: 'pending',
      created: new Date()
    };
    
    // Set current task
    await agent.setCurrentTask(task);
    
    // Complete the task
    await agent.completeTask('Task result');
    
    // Check state updates
    const state = agent.getState();
    expect(state.currentTask).toBeUndefined();
    expect(state.taskHistory).toHaveLength(1);
    expect(state.taskHistory?.[0].id).toBe('123');
    expect(state.taskHistory?.[0].status).toBe('completed');
    expect(state.taskHistory?.[0].result).toBe('Task result');
  });
  
  // Add more tests for your agent methods
});
```

### 2. Integration Testing

Create integration tests for your agent's interactions:

```typescript
// tests/agents/myagent/integration.test.ts
import { MyAgent } from '../../../src/agents/myagent';
import { createMockEnv } from '../../helpers';

describe('MyAgent Integration Tests', () => {
  let agent: MyAgent;
  let mockEnv: any;
  
  beforeEach(() => {
    mockEnv = createMockEnv();
    agent = new MyAgent();
  });
  
  test('infer should generate a response', async () => {
    // Set up state
    agent.setState({
      messages: [
        {
          id: '1',
          role: 'user',
          content: 'Analyze the sales data from Q1 2023'
        }
      ]
    });
    
    // Mock the LLM call
    const mockResponse = {
      content: 'Based on the Q1 2023 data, sales increased by 15% compared to Q4 2022.',
      toolCalls: []
    };
    
    jest.spyOn(agent as any, 'callLLM').mockResolvedValue(mockResponse);
    
    // Run inference
    const response = await agent.infer('mock-token');
    
    // Verify response
    expect(response).toEqual(mockResponse);
    
    // Verify state updates
    const state = agent.getState();
    expect(state.messages).toHaveLength(2);
    expect(state.messages[1].role).toBe('assistant');
    expect(state.messages[1].content).toBe(mockResponse.content);
  });
  
  // Add more integration tests
});
```

## Deployment

After implementing and testing your agent, deploy it by updating the production environment:

1. Build the project:
   ```
   yarn build
   ```

2. Deploy the worker:
   ```
   yarn deploy
   ```

3. Update any client-side configurations to use the new agent type.

## Troubleshooting Common Issues

### Agent Not Found Error

If you receive a "Agent not found" error:
- Verify that your agent is properly registered in the `server.ts` file
- Check that the agent name in the URL path matches the key in the agentConfig

### State Not Persisting

If agent state is not persisting between requests:
- Ensure that you're using the `setState` method and not modifying state directly
- Check that the agent's ID is consistent between requests
- Verify that state is properly initialized

### Tool Execution Failures

If tools are failing to execute:
- Check error handling in the tool implementation
- Verify that tool parameters are properly typed and validated
- Ensure that the agent has the necessary permissions/tokens

### UI Connection Issues

If the UI fails to connect to the agent:
- Check network requests for errors
- Verify that tokens are properly passed
- Ensure that the agent type is correctly specified in the useOpenAgent hook

## Best Practices

Follow these best practices for a successful agent implementation:

1. **Separation of Concerns**: Keep agent logic, tools, and UI components separate
2. **Proper Error Handling**: Implement comprehensive error handling in all tools and methods
3. **Consistent State Updates**: Always use the setState method to update agent state
4. **Thorough Testing**: Test all agent methods, tools, and integration points
5. **Clear Documentation**: Document agent capabilities, methods, and expected behaviors
6. **Type Safety**: Use TypeScript interfaces and types to ensure type safety
7. **Security**: Validate inputs and handle sensitive information securely