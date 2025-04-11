import { ToolContext } from "../toolContext";
import { addIssueComment, IssueCommentSchema } from "./operations/issues";
import { tool } from "ai";

export const addIssueCommentTool = (context: ToolContext) => tool({
  description: "Add a comment to an existing issue",
  parameters: IssueCommentSchema,
  execute: async (args) => {
    const { owner, repo, issue_number, body } = args;
    return addIssueComment(owner, repo, issue_number, body, context.githubToken);
  },
});
