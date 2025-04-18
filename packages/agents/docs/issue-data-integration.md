# Issue Data Integration for Solver Agent

## Current Architecture Analysis

### Solver Agent Implementation

The Solver agent is designed to analyze and implement solutions for issues in our OpenAgents Projects system. The current implementation consists of:

1. **Agent Class**: `packages/agents/src/agents/solver/index.ts` - Implements the core Solver agent functionality, extending the OpenAgent class.
2. **Agent State**: `packages/agents/src/agents/solver/types.ts` - Defines the SolverState interface that extends BaseAgentState.
3. **System Prompt**: `packages/agents/src/agents/solver/prompts.ts` - Generates system prompts based on current issue context.
4. **Agent Tools**: `packages/agents/src/agents/solver/tools.ts` - Provides tools for fetching issue details, updating status, and creating implementation plans.

### Frontend Integration

The frontend integration is primarily handled in:
- `apps/website/app/components/agent/solver-connector.tsx` - UI component for connecting to the Solver agent
- `apps/website/app/routes/issues/$id.tsx` - Issue detail page that includes the SolverConnector component

### Data Flow

Currently, the Solver agent receives issue data in the following manner:
1. The GitHub token is set via `agent.setGithubToken()`
2. Repository context is set via `agent.setRepositoryContext()`
3. Current issue is set via `agent.setCurrentIssue()`
4. After initialization, the agent can be queried through `handleSubmit()` or `sharedInfer()`

## Data Requirements for Complete Context

For the Solver agent to have comprehensive context about an issue, it needs:

### Issue Data (Already Implemented)
- Basic information (id, title, description, status)
- Metadata (labels, assignee, creation date)
- Current state (open, in progress, completed, etc.)

### Project Data (To Be Added)
- Project information (name, description, status)
- Project metadata (start date, priority, health)
- Project ownership (owner, team association)

### Team Data (To Be Added)
- Team information (name, icon, color)
- Team structure (members, roles)
- Workflow preferences

## Integration Options

### Option 1: Client-side Data Passing (Simple Approach)

**Description:**
Pass all required issue, project, and team data from the client alongside the GitHub token.

**Implementation:**
1. Enhance the `formattedIssue` object in solver-connector.tsx to include project and team details
2. Update `setCurrentIssue()` method to accept and store expanded issue data
3. Modify the system prompt generation to include project and team context

**Pros:**
- Simple implementation
- No additional server-side integration needed
- Data is already available on the client

**Cons:**
- Increases payload size for WebSocket messages
- Duplicates data that might be available in other services
- Client may not have access to all relevant project data

### Option 2: Worker-to-Worker Communication (Advanced Approach)

**Description:**
Connect the Solver agent directly to the Projects Dashboard service (both running as Cloudflare Workers) to fetch issue, project, and team data directly.

**Implementation:**
1. Implement a new tool that allows the Solver agent to query the Projects service
2. Create a secure authentication mechanism between workers
3. Pass only essential identifiers from the client (issue ID, GitHub token)
4. Have the Solver agent fetch complete context data directly

**Pros:**
- More robust architecture
- Reduces client-side payload size
- Provides access to the latest data
- Allows for future expansion of data needs

**Cons:**
- More complex implementation
- Requires cross-worker authentication
- Adds a dependency between services

## Recommendation

For the immediate implementation, **Option 1 (Client-side Data Passing)** is recommended based on the following considerations:

1. **Development Speed**: Can be implemented quickly with minimal changes
2. **Current Architecture**: Aligns with the existing pattern of passing data from client to agent
3. **Data Availability**: The issue detail page already has all the needed data for project and team context

However, in the longer term, a migration to **Option 2 (Worker-to-Worker Communication)** would provide a more robust architecture, especially as the system grows.

## Implementation Plan for Option 1

### 1. Enhance SolverConnector Component

Update `apps/website/app/components/agent/solver-connector.tsx` to include project and team data:

```typescript
// Create formatted project object
const formattedProject = issue.project ? {
  id: issue.project.id,
  name: issue.project.name,
  status: issue.project.status,
  priority: issue.project.priority,
  health: issue.project.health,
  percentComplete: issue.project.percentComplete,
  startDate: issue.project.startDate,
  owner: issue.project.owner ? {
    id: issue.project.owner.id,
    name: issue.project.owner.name
  } : undefined
} : undefined;

// Create formatted team object
const formattedTeam = issue.team ? {
  id: issue.team.id,
  name: issue.team.name,
  key: issue.team.key,
  color: issue.team.color,
  icon: issue.team.icon,
  owner: issue.team.owner ? {
    id: issue.team.owner.id,
    name: issue.team.owner.name
  } : undefined
} : undefined;

// Update the current agent methods to also set project and team context
if (agent.setCurrentIssue) {
  console.log("Setting current issue context...");
  await agent.setCurrentIssue(formattedIssue, formattedProject, formattedTeam);
}
```

### 2. Update Solver Agent State Type

Update `packages/agents/src/agents/solver/types.ts` to include project and team data:

```typescript
export interface SolverState extends BaseAgentState {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject; // Add this
  currentTeam?: BaseTeam; // Add this
  implementationSteps?: ImplementationStep[];
  issueComments?: IssueComment[];
}
```

### 3. Enhance Solver Agent Class

Update `packages/agents/src/agents/solver/index.ts` to handle the additional data:

```typescript
// Add method to set issue with project and team context
async setCurrentIssue(issue: BaseIssue, project?: BaseProject, team?: BaseTeam) {
  console.log("Setting current issue:", issue.id);
  console.log("With project context:", project?.name);
  console.log("With team context:", team?.name);
  
  try {
    await this.setState({
      ...this.state,
      currentIssue: issue,
      currentProject: project,
      currentTeam: team
    });
    return true;
  } catch (error) {
    console.error("Error setting current issue:", error);
    throw error;
  }
}
```

### 4. Update System Prompt Generation

Enhance `packages/agents/src/agents/solver/prompts.ts` to include project and team details:

```typescript
// Extract values from state
const {
  currentIssue,
  currentProject, // Add this
  currentTeam,    // Add this
  currentRepoOwner,
  currentRepoName,
  currentBranch,
  implementationSteps,
  observations,
  scratchpad,
  workingFilePath
} = state;

// ... later in the function ...

// Add project context if available
if (currentProject) {
  systemPrompt += `\n\nPROJECT CONTEXT:
Name: ${currentProject.name}
Status: ${currentProject.status}
Priority: ${currentProject.priority || 'Not set'}
Health: ${currentProject.health || 'Not set'}
Progress: ${currentProject.percentComplete || 0}% complete
${currentProject.startDate ? `Start Date: ${new Date(currentProject.startDate).toLocaleDateString()}` : ''}`;
}

// Add team context if available
if (currentTeam) {
  systemPrompt += `\n\nTEAM CONTEXT:
Name: ${currentTeam.name}
Key: ${currentTeam.key || 'Not set'}
${currentTeam.owner ? `Lead: ${currentTeam.owner.name}` : ''}`;
}
```

## Implementation Plan for Option 2 (Future Consideration)

For a more robust long-term solution:

1. Create a secure authentication mechanism between workers using symmetric keys or tokens
2. Implement a new tool in the Solver agent for querying the Projects service
3. Create API endpoints in the Projects service for fetching complete issue contexts
4. Update the SolverConnector to pass only essential identifiers

This approach would be implemented after Option 1 is working successfully and the system architecture has matured.

## Security Considerations

Both approaches need to ensure:

1. **Token Handling**: Authentication tokens should never be logged or exposed publicly
2. **Permission Boundaries**: The Solver agent should only access issues the user has permission to view
3. **Data Sanitization**: All data passed to the agent should be sanitized to prevent prompt injection

## Conclusion

The recommended approach is to start with client-side data passing (Option 1) for immediate implementation, as it requires minimal changes to the existing architecture while providing the Solver agent with the context it needs.

In the longer term, moving to a worker-to-worker communication model (Option 2) would provide a more robust architecture that could better handle evolving data needs and increasing system complexity.

## Implementation Log

### Phase 1 Implementation - Client-Side Data Passing (April 17, 2025)

The following changes were implemented to enable passing project and team context from the client to the Solver agent:

#### 1. Enhanced State Types

Updated `packages/agents/src/agents/solver/types.ts` to include project and team data in the agent state:

```typescript
export interface SolverState extends BaseAgentState {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject; // Added project context
  currentTeam?: BaseTeam;       // Added team context
  implementationSteps?: ImplementationStep[];
  issueComments?: IssueComment[];
}
```

Added necessary imports:

```typescript
import type { 
  BaseIssue, 
  BaseProject, 
  BaseTeam, 
  ImplementationStep, 
  IssueComment 
} from "@openagents/core";
```

#### 2. Extended Solver Agent Implementation

Updated `packages/agents/src/agents/solver/index.ts` to add a method for setting issue with project and team context:

```typescript
async setCurrentIssue(issue: BaseIssue, project?: BaseProject, team?: BaseTeam) {
  console.log("Setting current issue:", issue.id);
  console.log("With project context:", project?.name || 'None');
  console.log("With team context:", team?.name || 'None');
  
  try {
    await this.setState({
      ...this.state,
      currentIssue: issue,
      currentProject: project,
      currentTeam: team
    });
    return true;
  } catch (error) {
    console.error("Error setting current issue:", error);
    throw error;
  }
}
```

Added necessary imports:

```typescript
import { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
```

#### 3. Enhanced System Prompt Generation

Updated `packages/agents/src/agents/solver/prompts.ts` to extract project and team data from state:

```typescript
const {
  currentIssue,
  currentProject,
  currentTeam,
  // ... other state properties
} = state;
```

Added sections to include project and team context in the system prompt:

```typescript
// Add project context if available
if (currentProject) {
  systemPrompt += `\n\nPROJECT CONTEXT:
Name: ${currentProject.name}
${currentProject.id ? `ID: ${currentProject.id}` : ''}
${currentProject.status ? `Status: ${currentProject.status}` : ''}
${currentProject.priority ? `Priority: ${currentProject.priority}` : ''}
${currentProject.health ? `Health: ${currentProject.health}` : ''}
${currentProject.percentComplete !== undefined ? `Progress: ${currentProject.percentComplete}% complete` : ''}
${currentProject.startDate ? `Start Date: ${new Date(currentProject.startDate).toLocaleDateString()}` : ''}`;
}

// Add team context if available
if (currentTeam) {
  systemPrompt += `\n\nTEAM CONTEXT:
Name: ${currentTeam.name}
${currentTeam.id ? `ID: ${currentTeam.id}` : ''}
${currentTeam.key ? `Key: ${currentTeam.key}` : ''}
${currentTeam.color ? `Color: ${currentTeam.color}` : ''}`;
}
```

#### 4. Enhanced SolverConnector Component

Updated `apps/website/app/components/agent/solver-connector.tsx` to create formatted project and team objects:

```typescript
// Create formatted project object if available
const formattedProject = issue.project ? {
  id: issue.project.id,
  name: issue.project.name,
  status: issue.project.status || '',
  priority: issue.project.priority || '',
  health: issue.project.health || '',
  percentComplete: issue.project.percentComplete || 0,
  startDate: issue.project.startDate,
  owner: issue.project.owner ? {
    id: issue.project.owner.id,
    name: issue.project.owner.name
  } : undefined
} : undefined;

// Create formatted team object if available
const formattedTeam = issue.team ? {
  id: issue.team.id,
  name: issue.team.name,
  key: issue.team.key || '',
  color: issue.team.color || '',
  icon: issue.team.icon || '',
  owner: issue.team.owner ? {
    id: issue.team.owner.id,
    name: issue.team.owner.name
  } : undefined
} : undefined;
```

Modified the `setCurrentIssue` call to pass project and team data:

```typescript
if (agent.setCurrentIssue) {
  console.log("Step 3: Setting current issue with project and team context...");
  try {
    await agent.setCurrentIssue(formattedIssue, formattedProject, formattedTeam);
    console.log("✓ Current issue set successfully with project and team context");
  } catch (error) {
    console.error("✗ Failed to set current issue with context:", error);
    throw new Error(`Failed to set current issue with context: ${error instanceof Error ? error.message : "Unknown error"}`);
  }
}
```

Also updated the source identifier from "github" to "openagents" to reflect our own system:

```typescript
source: "openagents", // Using our own source identifier
```

#### 5. TypeScript Fixes

Several adjustments were made to fix TypeScript errors and ensure type safety:

- Changed import in `index.ts` to use type-only imports: 
  ```typescript
  import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
  ```

- Updated system prompt generation to use only properties defined in the BaseProject interface:
  ```typescript
  if (currentProject) {
    systemPrompt += `\n\nPROJECT CONTEXT:
  Name: ${currentProject.name}
  ID: ${currentProject.id}
  ${currentProject.color ? `Color: ${currentProject.color}` : ''}
  ${currentProject.icon ? `Icon: ${currentProject.icon}` : ''}`;
  }
  ```

- Updated SolverConnector component to create properly typed project and team objects:
  ```typescript
  // Create formatted project object
  const formattedProject = issue.project ? {
    id: issue.project.id,
    name: issue.project.name,
    color: issue.project.color,
    icon: issue.project.icon
  } : undefined;
  
  // Create formatted team object
  const formattedTeam = issue.team ? {
    id: issue.team.id,
    name: issue.team.name,
    key: issue.team.key || 'default'
  } : undefined;
  ```

#### 6. Testing and Verification

The implementation was verified by:
- Checking TypeScript compilation with no errors
- Reviewing code for consistency with existing patterns
- Ensuring proper error handling in all new methods
- Confirming that all objects adhere to their respective interfaces

These changes enable the Solver agent to have complete context about the issue, project, and team when analyzing and solving issues in our OpenAgents Projects system.

#### 7. Critical Fix - Using Raw Messages Instead of RPC Calls (April 18, 2025)

After initial deployment, we discovered a critical issue:

```
Error: Method setCurrentIssue is not callable
```

This error indicates that the agent's server implementation is rejecting the `setCurrentIssue` method calls from the client. The root cause appears to be that the Cloudflare Agent SDK doesn't allow calling this custom method through the RPC interface.

To resolve this issue, we changed the approach:

1. Instead of using `agent.setCurrentIssue()` to call a method on the server, we're now using `agent.sendRawMessage()` to send a WebSocket message directly:

```typescript
// Format in the same way the agent expects to receive state updates
const contextMessage = {
  type: "set_context",
  issue: formattedIssue,
  project: formattedProject,
  team: formattedTeam,
  timestamp: new Date().toISOString()
};

// Send the raw message
agent.sendRawMessage(contextMessage);
```

2. We added a new message handler in the Solver agent to process this message type:

```typescript
case "set_context":
  // Handle context setting message with issue, project and team data
  console.log("Received context data from client");
  console.log("Issue data:", parsedMessage.issue ? `ID: ${parsedMessage.issue.id}` : 'None');
  console.log("Project data:", parsedMessage.project ? `ID: ${parsedMessage.project.id}` : 'None');
  console.log("Team data:", parsedMessage.team ? `ID: ${parsedMessage.team.id}` : 'None');
  
  try {
    // Update the agent's state with the new context
    await this.setState({
      ...this.state,
      currentIssue: parsedMessage.issue,
      currentProject: parsedMessage.project,
      currentTeam: parsedMessage.team
    });
    
    console.log("Context set successfully");
    console.log("State after context update:", JSON.stringify({
      hasIssue: !!this.state.currentIssue,
      hasProject: !!this.state.currentProject,
      hasTeam: !!this.state.currentTeam
    }));
  } catch (error) {
    console.error("Error setting context:", error);
  }
  break;
```

3. We also fixed the "View System Prompt" functionality to use the same raw message approach with a slight delay to allow processing:

```typescript
// Use raw message sending directly
const contextMessage = {
  type: "set_context",
  issue: formattedIssue,
  project: formattedProject,
  team: formattedTeam,
  timestamp: new Date().toISOString()
};

// Send the raw message
agent.sendRawMessage(contextMessage);

// Wait a short time for the agent to process the context
setTimeout(() => {
  fetchSystemPrompt();
}, 300);
```

This approach aligns with how the other agent interactions (like Test Message, Add Observation, etc.) are already implemented, providing a more consistent communication pattern.

#### 9. OpenAgents Branding - Removing GitHub/Linear References (April 18, 2025)

We updated the agent's prompt and tools to properly reference OpenAgents Projects instead of GitHub/Linear:

1. Updated the system prompt intro to mention OpenAgents Projects:

```typescript
let systemPrompt = `You are an autonomous issue-solving agent designed to analyze, plan, and implement solutions for OpenAgents Projects issues. You work methodically to resolve issues in software projects.

// ...

You are a 'Solver' agent running in the OpenAgents project management web interface.`;
```

2. Updated the GUIDELINES to reference OpenAgents Projects:

```typescript
systemPrompt += `\n\nGUIDELINES:
1. FOLLOW A METHODICAL APPROACH to issue resolution - understand, plan, implement, test
2. USE TOOLS to gather information and interact with OpenAgents Projects issues
// ...
`;
```

3. Modified the tool descriptions:

```typescript
export const getIssueDetails = tool({
  description: "Fetch details about an issue from OpenAgents Projects",
  parameters: z.object({
    source: z.enum(["openagents"]).describe("The source platform for the issue - use 'openagents'"),
    // ...
  }),
```

4. Implemented OpenAgents Projects-specific tool functionality:

```typescript
// OpenAgents Projects implementation
else if (source === "openagents") {
  // For now, return the current issue from agent state
  const currentIssue = agent.state.currentIssue;
  if (currentIssue && currentIssue.id) {
    return {
      ...currentIssue,
      message: "Retrieved issue from OpenAgents Projects"
    };
  }
  // ...
}
```

These changes ensure the agent properly references our own issue tracking system rather than external services.

#### 8. Additional Fix - Core Hook Update (April 18, 2025)

After testing the implementation, we discovered the `useOpenAgent` hook in `packages/core/src/agents/useOpenAgent.ts` was not passing project and team data to the agent. We made the following fixes:

1. Updated the `setCurrentIssue` method in the hook to accept and pass project and team parameters:

```typescript
/**
 * Sets the current issue for the Solver agent with optional project and team context
 */
const setCurrentIssue = useCallback(async (issue: any, project?: any, team?: any): Promise<void> => {
  if (type === 'solver') {
    try {
      console.log(`[useOpenAgent ${agentName}] Setting current issue:`, issue.id);
      console.log(`[useOpenAgent ${agentName}] With project:`, project?.name || 'None');
      console.log(`[useOpenAgent ${agentName}] With team:`, team?.name || 'None');
      
      if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
        // Pass all three parameters to the agent
        await cloudflareAgent.call('setCurrentIssue', [issue, project, team]);
        console.log(`[useOpenAgent ${agentName}] Current issue set successfully with project and team context`);
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot set current issue: Agent not available or call not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to set current issue:`, error);
      // Mark connection as error
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }
      throw error;
    }
  }
}, [cloudflareAgent, type, agentName, connectionStatus]);
```

2. Updated the `OpenAgent` type interface to reflect the updated method signature:

```typescript
export type OpenAgent = {
  // ...other methods
  setCurrentIssue?: (issue: any, project?: any, team?: any) => Promise<void>; // Updated to include project and team
  // ...other methods
}
```

This fix completes the data passing chain from the UI component to the agent, ensuring that project and team context is properly transmitted and included in the agent's system prompt.