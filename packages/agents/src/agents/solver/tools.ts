import { tool } from "ai";
import { z } from "zod";
import { Solver, solverContext } from "./index";

/**
 * Tool to fetch an issue from OpenAgents Projects
 */
export const getIssueDetails = tool({
  description: "Fetch details about an issue from OpenAgents Projects",
  parameters: z.object({
    source: z.enum(["openagents", "github"]).describe("The source platform for the issue"),
    owner: z.string().optional().describe("The owner/organization (if needed)"),
    repo: z.string().optional().describe("The repository name (if needed)"),
    issueNumber: z.number().describe("The issue number/ID"),
    teamId: z.string().optional().describe("The team ID (optional)")
  }),
  execute: async ({ source, owner, repo, issueNumber, teamId }) => {
    console.log(`[getIssueDetails] Fetching ${source} issue #${issueNumber}`);
    
    const agent = solverContext.getStore();
    if (!agent || !(agent instanceof Solver)) {
      throw new Error("No agent found or agent is not a Solver instance");
    }

    // Get GitHub token from agent state
    const token = agent.state.githubToken;
    if (!token) {
      throw new Error("GitHub token is required but not provided");
    }

    try {
      // Fetch issue details based on source
      if (source === "github") {
        if (!owner || !repo) {
          throw new Error("Owner and repo are required for GitHub issues");
        }

        // Construct GitHub API URL
        const url = `https://api.github.com/repos/${owner}/${repo}/issues/${issueNumber}`;
        
        // Make API request
        const response = await fetch(url, {
          headers: {
            'Authorization': `Bearer ${token}`,
            'Accept': 'application/vnd.github.v3+json',
            'User-Agent': 'OpenAgents'
          }
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error(`[getIssueDetails] GitHub API error: ${response.status} ${response.statusText}`, errorText);
          throw new Error(`GitHub API error (${response.status}): ${errorText}`);
        }

        // Define a type for GitHub issue response
        interface GitHubIssue {
          id: number;
          number: number;
          title: string;
          body: string | null;
          state: string;
          html_url: string;
          assignee: { login: string } | null;
          labels: Array<{ name: string }>;
          created_at: string;
          updated_at: string;
        }
        
        const data = await response.json() as GitHubIssue;
        
        // Map the response to our Issue type
        return {
          id: data.id.toString(),
          number: data.number,
          title: data.title,
          description: data.body || "",
          source: "github",
          status: data.state === "open" ? "open" : "closed",
          url: data.html_url,
          assignee: data.assignee ? data.assignee.login : undefined,
          labels: data.labels ? data.labels.map(label => label.name) : [],
          created: new Date(data.created_at),
          updated: new Date(data.updated_at)
        };
      } 
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
        
        return {
          error: "Could not retrieve issue details",
          message: "Issue details not found in agent state"
        };
      }
    } catch (error) {
      console.error(`[getIssueDetails] Error:`, error);
      throw error;
    }
  }
});

/**
 * Tool to update issue status
 */
export const updateIssueStatus = tool({
  description: "Update the status of an issue in OpenAgents Projects",
  parameters: z.object({
    source: z.enum(["openagents", "github"]).describe("The source platform for the issue"),
    owner: z.string().optional().describe("The owner/organization (if needed)"),
    repo: z.string().optional().describe("The repository name (if needed)"),
    issueNumber: z.number().describe("The issue number/ID"),
    status: z.enum(["open", "in_progress", "review", "closed"]).describe("The new status for the issue"),
    comment: z.string().optional().describe("Optional comment to add with the status update")
  }),
  execute: async ({ source, owner, repo, issueNumber, status, comment }) => {
    console.log(`[updateIssueStatus] Updating ${source} issue #${issueNumber} to ${status}`);
    
    const agent = solverContext.getStore();
    if (!agent || !(agent instanceof Solver)) {
      throw new Error("No agent found or agent is not a Solver instance");
    }

    // Get GitHub token from agent state
    const token = agent.state.githubToken;
    if (!token) {
      throw new Error("GitHub token is required but not provided");
    }

    try {
      // Update issue status based on source
      if (source === "github") {
        if (!owner || !repo) {
          throw new Error("Owner and repo are required for GitHub issues");
        }

        // Map our status to GitHub status
        const githubState = status === "closed" ? "closed" : "open";
        
        // Construct GitHub API URL
        const url = `https://api.github.com/repos/${owner}/${repo}/issues/${issueNumber}`;
        
        // Make API request to update status
        const response = await fetch(url, {
          method: 'PATCH',
          headers: {
            'Authorization': `Bearer ${token}`,
            'Accept': 'application/vnd.github.v3+json',
            'Content-Type': 'application/json',
            'User-Agent': 'OpenAgents'
          },
          body: JSON.stringify({ 
            state: githubState,
            // If there's a status other than open/closed, we'll add it as a label
            ...(status !== "open" && status !== "closed" ? {
              labels: [`status:${status}`]
            } : {})
          })
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error(`[updateIssueStatus] GitHub API error: ${response.status} ${response.statusText}`, errorText);
          throw new Error(`GitHub API error (${response.status}): ${errorText}`);
        }

        // If a comment was provided, add it
        if (comment) {
          const commentUrl = `https://api.github.com/repos/${owner}/${repo}/issues/${issueNumber}/comments`;
          
          const commentResponse = await fetch(commentUrl, {
            method: 'POST',
            headers: {
              'Authorization': `Bearer ${token}`,
              'Accept': 'application/vnd.github.v3+json',
              'Content-Type': 'application/json',
              'User-Agent': 'OpenAgents'
            },
            body: JSON.stringify({ body: comment })
          });

          if (!commentResponse.ok) {
            const errorText = await commentResponse.text();
            console.error(`[updateIssueStatus] GitHub comment API error: ${commentResponse.status}`, errorText);
            // Continue despite comment error
          }
        }

        // Define a type for GitHub issue update response
        interface GitHubIssueUpdate {
          id: number;
          number: number;
          updated_at: string;
        }
        
        const data = await response.json() as GitHubIssueUpdate;
        
        return {
          success: true,
          issue: {
            id: data.id.toString(),
            number: data.number,
            status: status,
            updated: new Date(data.updated_at)
          },
          message: `Issue #${issueNumber} updated to ${status}${comment ? " with comment" : ""}`
        };
      } 
      // OpenAgents Projects implementation
      else if (source === "openagents") {
        // For now, update the issue in agent state
        const currentIssue = agent.state.currentIssue;
        if (currentIssue && currentIssue.id) {
          const updatedIssue = {
            ...currentIssue,
            status: status,
            updated: new Date()
          };
          
          // Update agent state
          await agent.setState({
            ...agent.state,
            currentIssue: updatedIssue
          });
          
          return {
            success: true,
            issue: updatedIssue,
            message: `Issue #${issueNumber} updated to ${status}${comment ? " with comment" : ""}`
          };
        }
        
        return {
          error: "Could not update issue status",
          message: "Issue not found in agent state"
        };
      }
    } catch (error) {
      console.error(`[updateIssueStatus] Error:`, error);
      throw error;
    }
  }
});

/**
 * Tool to create an implementation plan for solving an issue
 */
export const createImplementationPlan = tool({
  description: "Create a step-by-step implementation plan for solving an issue",
  parameters: z.object({
    issueTitle: z.string().describe("The title of the issue"),
    issueDescription: z.string().describe("The full description of the issue"),
    repoOwner: z.string().optional().describe("The owner of the repository"),
    repoName: z.string().optional().describe("The name of the repository")
  }),
  execute: async ({ issueTitle, issueDescription, repoOwner, repoName }) => {
    console.log(`[createImplementationPlan] Creating plan for issue: ${issueTitle}`);
    
    const agent = solverContext.getStore();
    if (!agent || !(agent instanceof Solver)) {
      throw new Error("No agent found or agent is not a Solver instance");
    }

    try {
      // In a real implementation, you might use an LLM call here to generate the plan
      // For this demonstration, we'll create a simple plan with predefined steps
      
      const steps = [
        {
          id: "step-1",
          description: "Analyze the issue requirements",
          type: "analysis" as const,
          status: "pending" as const,
          created: new Date(),
          notes: "Understand the scope and requirements from the issue description"
        },
        {
          id: "step-2",
          description: "Research existing solutions",
          type: "research" as const,
          status: "pending" as const,
          created: new Date(),
          dependsOn: ["step-1"],
          notes: "Look for similar patterns or solutions in the codebase"
        },
        {
          id: "step-3",
          description: "Implement solution",
          type: "implementation" as const,
          status: "pending" as const,
          created: new Date(),
          dependsOn: ["step-2"],
          notes: "Write code to address the issue"
        },
        {
          id: "step-4",
          description: "Add tests",
          type: "testing" as const,
          status: "pending" as const,
          created: new Date(),
          dependsOn: ["step-3"],
          notes: "Create tests to verify the solution"
        },
        {
          id: "step-5",
          description: "Update documentation",
          type: "documentation" as const,
          status: "pending" as const,
          created: new Date(),
          dependsOn: ["step-3"],
          notes: "Update relevant documentation for the changes"
        }
      ];
      
      // Get current state first
      const currentState = agent.state;
      
      // Then update with the full state object including required messages
      await agent.setState({
        ...currentState,
        implementationSteps: steps
      });
      
      return {
        success: true,
        plan: {
          issueTitle,
          steps
        },
        message: `Created implementation plan with ${steps.length} steps for: ${issueTitle}`
      };
    } catch (error) {
      console.error(`[createImplementationPlan] Error:`, error);
      throw error;
    }
  }
});

/**
 * Export solver-specific tools
 */
export const solverTools = {
  getIssueDetails,
  updateIssueStatus,
  createImplementationPlan
};