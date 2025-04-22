import { Effect, Layer, Console } from "effect";
import * as readline from "node:readline/promises";
import { GitHubTools, GitHubToolsLive, FileContentParams, IssueParams, gitHubClientLayers } from "./AiService.js";
import { GitHubConfigTag, createGitHubHttpExecutor } from "./github/Client.js";
import { TOOL_SCHEMAS } from "./Tools.js";
import {
  ContentBlockToolUse,
  MessageCreateParamsWithTools,
  ExtendedContentBlock,
  ExtendedMessageParam,
  ToolDefinition,
  ExtendedMessage
} from "./types.js";
import * as dotenv from 'dotenv';
import { Anthropic } from '@anthropic-ai/sdk';

// Load environment variables
dotenv.config();

// --- Conversation History ---
const conversation: ExtendedMessageParam[] = []; // Simple in-memory history

// --- Create GitHub Layers ---
const createGitHubConfig = () => {
  const githubToken = process.env.GITHUB_TOKEN;

  return Layer.succeed(
    GitHubConfigTag,
    { baseUrl: "https://api.github.com", token: githubToken }
  );
};

// Helper function to create the full program layer
const createProgramLayer = () => {
  // Define all necessary layers for this specific operation
  const baseGitHubLayer = Layer.provide(createGitHubHttpExecutor, createGitHubConfig()); // Base HTTP executor and config
  const fullClientLayer = Layer.provide(gitHubClientLayers, baseGitHubLayer); // Provide executor to clients
  return Layer.provide(GitHubToolsLive, fullClientLayer); // Provide clients to tools layer
};

// --- AI interaction logic ---
const processUserMessage = async (userMessage: string) => {
  // Add to conversation history
  conversation.push({ role: "user", content: userMessage });

  console.log(`User: ${userMessage}`);
  console.log("Assistant: ");

  // Create Anthropic client directly (outside Effect)
  const anthropic = new Anthropic({
    apiKey: process.env.ANTHROPIC_API_KEY || "dummy-key",
  });

  // Set up the Effect program that will handle GitHub tools
  const program = Effect.gen(function* () {
    // Get the GitHubTools service instance from the Effect context
    const githubTools = yield* GitHubTools;

    // Create a helper function that will execute the GitHub tools
    const executeGitHubTool = (toolName: string, toolInput: Record<string, unknown>, toolUseId: string) => {
      return Effect.gen(function* () {
        yield* Console.log(`\n>>> Tool Call Requested: ${toolName}`);
        yield* Console.log(`>>> Tool Input: ${JSON.stringify(toolInput, null, 2)}`);

        let toolResultEffect: Effect.Effect<string, string>;

        // Execute the correct tool Effect based on the name
        if (toolName === "GetGitHubFileContent") {
          // Need to cast the dynamic toolInput to a properly typed parameter
          const fileParams: FileContentParams = {
            owner: String(toolInput.owner),
            repo: String(toolInput.repo),
            path: String(toolInput.path),
            ref: toolInput.ref ? String(toolInput.ref) : undefined
          };
          toolResultEffect = githubTools.getFileContent(fileParams);
        } else if (toolName === "GetGitHubIssue") {
          // Need to cast the dynamic toolInput to a properly typed parameter
          const issueParams: IssueParams = {
            owner: String(toolInput.owner),
            repo: String(toolInput.repo),
            issueNumber: Number(toolInput.issueNumber)
          };
          toolResultEffect = githubTools.getIssue(issueParams);
        } else {
          yield* Console.warn(`Unknown tool called: ${toolName}`);
          toolResultEffect = Effect.fail(`Unknown tool: ${toolName}`);
        }

        // Execute the tool Effect and handle success/failure
        return yield* Effect.match(toolResultEffect, {
          onFailure: (errorString) => {
            console.error(`<<< Tool Execution Failed: ${errorString}`);
            return {
              toolUseId,
              toolResult: `Error executing tool ${toolName}: ${errorString}`
            };
          },
          onSuccess: (successString) => {
            console.log(`<<< Tool Execution Succeeded: ${successString.substring(0, 100)}...`);
            return {
              toolUseId,
              toolResult: successString
            };
          }
        });
      });
    };

    // Return the function that can execute GitHub tools
    return executeGitHubTool;
  });

  try {
    // Run the Effect program to get the tool execution function
    const programLayer = createProgramLayer();
    const executeGitHubTool = await Effect.runPromise(Effect.provide(program, programLayer));

    // --- Initial API Call ---
    console.log("--- Sending Request to Anthropic (Initial) ---");
    const initialMessages = conversation.map(msg => ({ role: msg.role, content: msg.content }));
    console.log("Messages:", JSON.stringify(initialMessages, null, 2));
    console.log("Tools:", JSON.stringify(TOOL_SCHEMAS, null, 2));
    console.log("---------------------------------------------");

    try {
      // Make the initial call to Anthropic (outside Effect)
      // Need to cast since Anthropic SDK doesn't support tools yet in TypeScript
      const createParams: MessageCreateParamsWithTools = {
        model: "claude-3-5-sonnet-latest",
        system: "You are a helpful coding assistant specialized in GitHub repositories. Use the provided tools to fetch GitHub files and issues when requested.",
        messages: initialMessages,
        tools: TOOL_SCHEMAS as ToolDefinition[],
        max_tokens: 1000,
      };

      // Use proper type casting to match SDK requirements
      const response = await anthropic.messages.create({
        ...createParams,
        model: createParams.model,
        system: createParams.system,
        max_tokens: createParams.max_tokens,
        messages: createParams.messages,
        tools: createParams.tools
      }) as ExtendedMessage;

      console.log("--- Received Response from Anthropic (Initial) ---");
      console.log(JSON.stringify(response, null, 2));
      console.log("-----------------------------------------------");

      // Process the response
      if (response.content && response.content.length > 0) {
        const firstItem = response.content[0] as ExtendedContentBlock;

        if (firstItem.type === "text") {
          // Simple text response
          process.stdout.write(firstItem.text);
          conversation.push({ role: "assistant", content: firstItem.text });
          console.log("\n--- Interaction Complete (Text Response) ---");

        } else if (firstItem.type === "tool_use") {
          // Tool call - need to extract details and execute the tool
          const typedItem = firstItem as ContentBlockToolUse;
          const toolUse = typedItem.tool_use;
          const toolName = toolUse.name;
          const toolInput = toolUse.input;
          const toolUseId = toolUse.id;

          // Execute the tool using our Effect wrapper
          const { toolResult } = await Effect.runPromise(
            Effect.provide(
              executeGitHubTool(toolName, toolInput, toolUseId),
              programLayer
            )
          );

          // --- Follow-up API Call with Tool Result ---
          // Create the proper continuation structure for Anthropic
          const userToolResult = {
            role: "user" as const,
            content: [
              {
                type: "tool_result" as const,
                tool_use_id: toolUseId,
                content: toolResult,
              }
            ]
          };

          const messagesForFollowup = [
            ...initialMessages, // Original conversation
            { role: "assistant" as const, content: [firstItem] }, // Assistant's tool_use message
            userToolResult // User message containing tool result
          ];

          console.log("--- Sending Request to Anthropic (Follow-up) ---");
          console.log("Messages:", JSON.stringify(messagesForFollowup, null, 2));
          console.log("----------------------------------------------");

          try {
            // Cast result to our extended message type since Anthropic SDK types don't include tools
            const followupResponse = await anthropic.messages.create({
              model: "claude-3-5-sonnet-latest",
              messages: messagesForFollowup,
              max_tokens: 1000,
            }) as unknown as ExtendedMessage;

            console.log("--- Received Response from Anthropic (Follow-up) ---");
            console.log(JSON.stringify(followupResponse, null, 2));
            console.log("-------------------------------------------------");

            // Process follow-up response
            if (followupResponse.content && followupResponse.content.length > 0) {
              const followupContent = followupResponse.content[0] as ExtendedContentBlock;
              if (followupContent.type === "text") {
                process.stdout.write(followupContent.text);
                conversation.push({ role: "assistant", content: followupContent.text });
                console.log("\n--- Interaction Complete (Tool Follow-up) ---");
              } else {
                console.warn("Follow-up response was not text:", followupContent.type);
              }
            } else {
              console.warn("Follow-up response had no content.");
            }
          } catch (error) {
            console.error("Anthropic follow-up API call failed:", error);
          }
        } else {
          console.warn("Unknown response content type:", firstItem.type);
        }
      } else {
        console.warn("Initial response had no content.");
      }
    } catch (error) {
      console.error("Anthropic API call failed:", error);
    }
  } catch (error) {
    console.error("Error during Effect program execution:", error);
  }
};

// --- CLI interface ---
export const startCLI = async (): Promise<void> => {
  console.log("🤖 GitHub Agent CLI - Type your questions about GitHub repositories");
  console.log("Type 'exit' or 'quit' to end the session");
  console.log("-".repeat(60));

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  let running = true;

  while (running) {
    const userInput = await rl.question("\n> ");

    if (userInput.toLowerCase() === 'exit' || userInput.toLowerCase() === 'quit') {
      console.log("Goodbye! 👋");
      running = false;
    } else if (userInput.trim() !== '') {
      await processUserMessage(userInput);
    }
  }

  rl.close();
};

// Start CLI if run directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startCLI().catch(console.error);
}
