# Issue Data Integration for Solver Agent

## Current Architecture Analysis

### Solver Agent Implementation

The Solver agent is designed to analyze and implement solutions for GitHub and Linear issues. The current implementation consists of:

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
- Source (GitHub, Linear)
- Current state (open, closed, etc.)

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

1. **GitHub Token Handling**: GitHub tokens should never be logged or exposed publicly
2. **Permission Boundaries**: The Solver agent should only access issues the user has permission to view
3. **Data Sanitization**: All data passed to the agent should be sanitized to prevent prompt injection

## Conclusion

The recommended approach is to start with client-side data passing (Option 1) for immediate implementation, as it requires minimal changes to the existing architecture while providing the Solver agent with the context it needs.

In the longer term, moving to a worker-to-worker communication model (Option 2) would provide a more robust architecture that could better handle evolving data needs and increasing system complexity.