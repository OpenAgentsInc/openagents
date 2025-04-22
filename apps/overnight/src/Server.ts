import { Effect, Layer } from "effect";
import * as readline from "node:readline/promises";
import { GitHubTools, GitHubToolsLive } from "./AiService.js";
import { GitHubConfigTag, createGitHubHttpExecutor } from "./github/Client.js";
import { githubFileClientLayer } from "./github/FileClient.js";
import { githubIssueClientLayer } from "./github/IssueClient.js";
import * as dotenv from 'dotenv';
import { Anthropic } from '@anthropic-ai/sdk';

// Load environment variables
dotenv.config();

// --- Conversation History ---
interface Message { 
  role: "user" | "assistant"; 
  content: string; 
}

const conversation: Message[] = []; // Simple in-memory history

// --- AI interaction logic ---
const processUserMessage = async (userMessage: string) => {
  // Get GitHub token from env
  const githubToken = process.env.GITHUB_TOKEN;
  
  // Create GitHub config layer first
  const githubConfigLayer = Layer.succeed(
    GitHubConfigTag,
    { baseUrl: "https://api.github.com", token: githubToken }
  );
  
  // Create the GitHub executor layer properly with config as a dependency
  const executorLayer = createGitHubHttpExecutor;
  
  // Create the GitHub client layers that depend on the executor
  const clientLayers = Layer.merge(
    githubFileClientLayer,
    githubIssueClientLayer
  );
  
  // Create the complete GitHub layer 
  const githubLayer = Layer.provide(
    clientLayers,
    Layer.provide(
      executorLayer,
      githubConfigLayer
    )
  );
  
  // Define full layer with GitHub tools
  const toolsLayer = Layer.mergeAll(
    GitHubToolsLive,
    githubLayer
  );
  
  conversation.push({ role: "user", content: userMessage });
  
  // Setup Effect environment with GitHub Tools
  const program = Effect.gen(function*() {
    // Get tools service
    yield* GitHubTools;
    
    console.log(`User: ${userMessage}`);
    console.log("Assistant: ");
  });
  
  // Function to handle the API call outside of Effect
  async function callAnthropicAPI() {
    try {
      // Create Anthropic client
      const anthropic = new Anthropic({
        apiKey: process.env.ANTHROPIC_API_KEY || "dummy-key"
      });

      // System message
      const systemMessage = "You are a helpful coding assistant specialized in GitHub repositories.";
      
      // Call Anthropic API
      const response = await anthropic.messages.create({
        model: "claude-3-haiku-20240307",
        system: systemMessage,
        messages: conversation.map(msg => ({
          role: msg.role,
          content: msg.content
        })),
        max_tokens: 1000
      });
      
      // Process the response
      if (response.content && response.content.length > 0) {
        const firstItem = response.content[0];
        
        if (firstItem.type === "text") {
          // Simple text response
          process.stdout.write(firstItem.text);
          conversation.push({ role: "assistant", content: firstItem.text });
        }
      }
    } catch (error) {
      console.error("Error in AI processing:", error);
    }
  }
  
  // Run the Effect program with the GitHub tools layer
  await Effect.runPromise(Effect.provide(program, toolsLayer));
  
  // Call the Anthropic API
  await callAnthropicAPI();
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