import type { ToolContext } from "../toolContext";
import { addIssueComment, IssueCommentSchema } from "./operations/issues";
import { tool } from "ai";

// @ts-ignore - Ignoring type mismatch issues due to version differences in tool schema
export const addIssueCommentTool = (context: ToolContext) => tool({
  description: "Add a comment to an existing issue",
  parameters: IssueCommentSchema,
  execute: async (args) => {
    const { owner, repo, issue_number, body } = args;
    
    // Add check for GitHub token
    if (!context.githubToken) {
      throw new Error("GitHub token is required in ToolContext to add an issue comment.");
    }
    
    return addIssueComment(owner, repo, issue_number, body, context.githubToken);
  },
});
