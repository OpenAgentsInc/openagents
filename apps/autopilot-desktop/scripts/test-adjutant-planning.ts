#!/usr/bin/env bun
/**
 * Test Script for Adjutant Agent Planning via AI Gateway
 *
 * This script tests the full Adjutant planning pipeline:
 * 1. Starts the AI Gateway server
 * 2. Tests topic decomposition
 * 3. Tests parallel exploration
 * 4. Tests plan synthesis
 * 5. Logs all results to console
 *
 * Usage: bun run scripts/test-adjutant-planning.ts
 */

import { spawn, type Subprocess } from 'bun';
import { sleep } from 'bun';
import { readFile } from 'fs/promises';
import { resolve } from 'path';

// Configuration
const AI_SERVER_PORT = 3001;
const AI_SERVER_HOST = 'localhost';
const AI_GATEWAY_API_KEY = process.env.AI_GATEWAY_API_KEY;
const PRIMARY_MODEL = 'google/gemini-2.5-flash-lite';
const FALLBACK_MODEL = 'openai/gpt-5-nano';
const MAX_FILES_PER_TOPIC = 6;
const MAX_CHARS_PER_FILE = 4000;

// Types
interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

interface ChatCompletionRequest {
  model: string;
  messages: ChatMessage[];
  max_tokens?: number;
  temperature?: number;
}

interface ExplorationTopic {
  name: string;
  focus: string;
  patterns: string[];
}

interface TopicsResponse {
  topics: ExplorationTopic[];
}

interface ExplorationResult {
  topic: string;
  focus: string;
  files_examined: string[];
  key_findings: string;
}

// Utility functions
const log = {
  info: (msg: string) => console.log(`ℹ️  ${msg}`),
  success: (msg: string) => console.log(`✅ ${msg}`),
  error: (msg: string) => console.log(`❌ ${msg}`),
  warn: (msg: string) => console.log(`⚠️  ${msg}`),
  step: (step: number, msg: string) => console.log(`\n🔸 Step ${step}: ${msg}`),
  result: (msg: string) => console.log(`📄 ${msg}`),
};

class AiGatewayTester {
  private baseUrl = `http://${AI_SERVER_HOST}:${AI_SERVER_PORT}`;
  private serverProcess: Subprocess | null = null;
  private repoRoot = process.cwd();

  async startServer(): Promise<void> {
    log.step(1, 'Starting AI Gateway server');

    // Start the bun server
    this.serverProcess = spawn({
      cmd: ['bun', 'run', 'server.ts'],
      cwd: './ai-server',
      env: {
        ...process.env,
        AI_SERVER_PORT: AI_SERVER_PORT.toString(),
        AI_SERVER_HOST,
        AI_GATEWAY_API_KEY,
        NODE_ENV: 'development'
      },
      stdout: 'pipe',
      stderr: 'pipe'
    });

    // Wait for server to be ready
    await this.waitForServer();
    log.success('AI Gateway server is ready');
  }

  async stopServer(): Promise<void> {
    if (this.serverProcess) {
      log.info('Stopping AI Gateway server...');
      this.serverProcess.kill();
      this.serverProcess = null;
      log.success('AI Gateway server stopped');
    }
  }

  private async waitForServer(maxAttempts = 30): Promise<void> {
    for (let i = 0; i < maxAttempts; i++) {
      try {
        const response = await fetch(`${this.baseUrl}/health`);
        if (response.ok) {
          return;
        }
      } catch (error) {
        // Server not ready yet
      }
      await sleep(1000);
      log.info(`Waiting for server... (${i + 1}/${maxAttempts})`);
    }
    throw new Error('Server failed to start within timeout');
  }

  async testHealthCheck(): Promise<void> {
    log.step(2, 'Testing health check endpoint');

    try {
      const response = await fetch(`${this.baseUrl}/health`);
      const health = await response.json();

      log.success('Health check passed');
      log.result(`Status: ${health.status}`);
      log.result(`Uptime: ${health.uptime}s`);
      log.result(`Models: ${health.models.length} available`);
      console.log(JSON.stringify(health, null, 2));
    } catch (error) {
      log.error(`Health check failed: ${error}`);
      throw error;
    }
  }

  async testChatCompletion(): Promise<void> {
    log.step(3, 'Testing basic chat completion');

    const request: ChatCompletionRequest = {
      model: PRIMARY_MODEL,
      messages: [
        {
          role: 'user',
          content: 'Say "Hello from Adjutant!" in exactly those words.'
        }
      ],
      max_tokens: 100,
      temperature: 0.1
    };

    try {
      const response = await fetch(`${this.baseUrl}/v1/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${AI_GATEWAY_API_KEY}`
        },
        body: JSON.stringify(request)
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${await response.text()}`);
      }

      const completion = await response.json();

      log.success('Chat completion successful');
      log.result(`Model: ${completion.model}`);
      log.result(`Response: ${completion.choices[0]?.message.content}`);
      log.result(`Tokens: ${completion.usage?.total_tokens || 0}`);

    } catch (error) {
      log.error(`Chat completion failed: ${error}`);
      throw error;
    }
  }

  async testTopicDecomposition(userPrompt: string): Promise<ExplorationTopic[]> {
    log.step(4, 'Testing topic decomposition with structured output');

    const request = {
      user_prompt: userPrompt,
      file_tree: `src/\n├── main.rs\n├── lib.rs\n└── agent/\n    ├── mod.rs\n    └── adjutant/`,
      model: PRIMARY_MODEL
    };

    try {
      const response = await fetch(`${this.baseUrl}/dspy/topics`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${AI_GATEWAY_API_KEY}`
        },
        body: JSON.stringify(request)
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${await response.text()}`);
      }

      const result = await response.json();

      log.success('Topic decomposition completed with structured output');
      log.result(`Model used: ${result.model}`);
      log.result(`Tokens used: ${result.usage?.totalTokens || 0}`);
      log.result(`Generated ${result.topics.length} exploration topics:`);

      result.topics.forEach((topic: ExplorationTopic, i: number) => {
        log.result(`  ${i + 1}. ${topic.name}: ${topic.focus}`);
        log.result(`     Patterns: [${topic.patterns.join(', ')}]`);
      });

      return result.topics;

    } catch (error) {
      log.error(`Topic decomposition failed: ${error}`);
      throw error;
    }
  }

  async testParallelExploration(topics: ExplorationTopic[]): Promise<ExplorationResult[]> {
    log.step(5, `Testing parallel exploration (${topics.length} agents)`);

    const explorationPromises = topics.map(async (topic, index) => {
      const agentNum = index + 1;
      log.info(`[Agent ${agentNum}] Starting: ${topic.name}`);

      const filesExamined = await this.gatherFilesForTopic(topic.patterns);
      const fileContext = await this.buildFileContext(filesExamined);

      const systemPrompt = `You are an exploration agent investigating: ${topic.name}
Focus: ${topic.focus}
Use the provided context to analyze and find relevant information.
Respond with your findings in a clear, structured format.`;

      const request: ChatCompletionRequest = {
        model: PRIMARY_MODEL,
        messages: [
          { role: 'system', content: systemPrompt },
          {
            role: 'user',
            content: `Explore the codebase to understand: ${topic.name}
Focus on: ${topic.focus}
Suggested search patterns: ${topic.patterns.join(', ')}

Files examined:
${filesExamined.length ? filesExamined.join('\n') : 'No files matched.'}

File context:
${fileContext || 'No file context available.'}

Provide a concise summary of relevant code patterns and implementation details.`
          }
        ],
        max_tokens: 2048,
        temperature: 0.5
      };

      try {
        const response = await fetch(`${this.baseUrl}/v1/chat/completions`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${AI_GATEWAY_API_KEY}`
          },
          body: JSON.stringify(request)
        });

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${await response.text()}`);
        }

        const completion = await response.json();
        const findings = completion.choices[0]?.message.content || '';

        log.success(`[Agent ${agentNum}] Completed: ${topic.name}`);
        log.result(`[Agent ${agentNum}] Findings: ${findings.substring(0, 200)}...`);

        return {
          topic: topic.name,
          focus: topic.focus,
          files_examined: filesExamined,
          key_findings: findings
        };

      } catch (error) {
        log.error(`[Agent ${agentNum}] Failed: ${error}`);
        throw error;
      }
    });

    // Execute all explorations in parallel
    const results = await Promise.all(explorationPromises);

    log.success('Parallel exploration completed');
    log.result(`Exploration complete:`);
    results.forEach(result => {
      log.result(`  [${result.topic}] ${result.files_examined.length} files examined`);
    });

    return results;
  }

  private gatherFilesForTopic(patterns: string[]): string[] {
    const files: string[] = [];
    for (const pattern of patterns) {
      if (files.length >= MAX_FILES_PER_TOPIC) {
        break;
      }
      const matches = this.rgFilesForPattern(pattern);
      for (const match of matches) {
        if (files.length >= MAX_FILES_PER_TOPIC) {
          break;
        }
        if (!files.includes(match)) {
          files.push(match);
        }
      }
    }

    if (files.length === 0) {
      files.push(...this.rgListRepoFiles(MAX_FILES_PER_TOPIC));
    }

    return files.slice(0, MAX_FILES_PER_TOPIC);
  }

  private rgFilesForPattern(pattern: string): string[] {
    try {
      const proc = Bun.spawnSync({
        cmd: ['rg', '-l', '-i', '--no-messages', pattern, '.'],
        cwd: this.repoRoot,
        stdout: 'pipe',
        stderr: 'pipe'
      });

      if (proc.exitCode !== 0 && proc.exitCode !== 1) {
        return [];
      }

      return new TextDecoder()
        .decode(proc.stdout)
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean);
    } catch {
      return [];
    }
  }

  private rgListRepoFiles(limit: number): string[] {
    try {
      const proc = Bun.spawnSync({
        cmd: ['rg', '--files', '--no-messages'],
        cwd: this.repoRoot,
        stdout: 'pipe',
        stderr: 'pipe'
      });

      if (proc.exitCode !== 0) {
        return [];
      }

      return new TextDecoder()
        .decode(proc.stdout)
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean)
        .slice(0, limit);
    } catch {
      return [];
    }
  }

  private async buildFileContext(files: string[]): Promise<string> {
    const chunks: string[] = [];

    for (const file of files.slice(0, MAX_FILES_PER_TOPIC)) {
      try {
        const fullPath = resolve(this.repoRoot, file);
        const content = await readFile(fullPath, 'utf-8');
        const snippet = content.slice(0, MAX_CHARS_PER_FILE);
        chunks.push(`FILE: ${file}\n${snippet}`);
      } catch {
        // Skip unreadable files
      }
    }

    return chunks.join('\n\n');
  }

  async testPlanSynthesis(userPrompt: string, explorationResults: ExplorationResult[]): Promise<string> {
    log.step(6, 'Testing plan synthesis');

    // Build context from exploration results
    let combinedFindings = '';
    explorationResults.forEach(result => {
      combinedFindings += `## ${result.topic}\n`;
      combinedFindings += `Focus: ${result.focus}\n`;
      combinedFindings += `Files examined: ${result.files_examined.join(', ')}\n`;
      combinedFindings += `\nFindings:\n${result.key_findings}\n\n`;
    });

    const systemPrompt = `You are a software architect creating implementation plans.
Based on exploration findings, write a clear, actionable plan.
Use markdown format with sections for Objective, Context, Steps, and Files.`;

    const request: ChatCompletionRequest = {
      model: PRIMARY_MODEL,
      messages: [
        { role: 'system', content: systemPrompt },
        {
          role: 'user',
          content: `User request: ${userPrompt}

Exploration findings:
${combinedFindings}

Write a comprehensive implementation plan based on these findings.`
        }
      ],
      max_tokens: 8192,
      temperature: 0.7
    };

    try {
      const response = await fetch(`${this.baseUrl}/v1/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${AI_GATEWAY_API_KEY}`
        },
        body: JSON.stringify(request)
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${await response.text()}`);
      }

      const completion = await response.json();
      const plan = completion.choices[0]?.message.content || '';

      log.success('Plan synthesis completed');
      log.result(`Plan length: ${plan.length} characters`);
      log.result(`Tokens used: ${completion.usage?.total_tokens || 0}`);

      return plan;

    } catch (error) {
      log.error(`Plan synthesis failed: ${error}`);
      throw error;
    }
  }

  async testDspyEndpoint(): Promise<void> {
    log.step(7, 'Testing DSPy-specific endpoint');

    const dspyRequest = {
      signature_type: 'planning',
      inputs: {
        user_prompt: 'Add user authentication system',
        file_tree: 'src/\n├── main.rs\n└── lib.rs',
        repo_context: 'Rust web application'
      },
      model: PRIMARY_MODEL
    };

    try {
      const response = await fetch(`${this.baseUrl}/dspy/predict`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${AI_GATEWAY_API_KEY}`
        },
        body: JSON.stringify(dspyRequest)
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${await response.text()}`);
      }

      const prediction = await response.json();

      log.success('DSPy prediction successful');
      log.result(`Model used: ${prediction.model}`);
      log.result(`Signature type: ${prediction.signature_type}`);
      log.result(`Prediction: ${prediction.prediction.substring(0, 300)}...`);

    } catch (error) {
      log.error(`DSPy endpoint failed: ${error}`);
      // Don't throw - this is optional
    }
  }

  async runFullPlanningTest(userPrompt: string): Promise<void> {
    console.log('🚀 Starting Adjutant Agent Planning Test');
    console.log('=====================================');
    console.log(`User Prompt: "${userPrompt}"`);
    console.log(`Primary Model: ${PRIMARY_MODEL}`);
    console.log(`Fallback Model: ${FALLBACK_MODEL}`);
    console.log(`AI Gateway: ${this.baseUrl}`);
    console.log('=====================================\n');

    try {
      // Step 1: Start server
      await this.startServer();

      // Step 2: Health check
      await this.testHealthCheck();

      // Step 3: Basic chat test
      await this.testChatCompletion();

      // Step 4: Topic decomposition
      const topics = await this.testTopicDecomposition(userPrompt);

      // Step 5: Parallel exploration
      const explorationResults = await this.testParallelExploration(topics);

      // Step 6: Plan synthesis
      const implementationPlan = await this.testPlanSynthesis(userPrompt, explorationResults);

      // Step 7: Test DSPy endpoint
      await this.testDspyEndpoint();

      // Final results
      console.log('\n🎯 FINAL IMPLEMENTATION PLAN');
      console.log('=============================');
      console.log(implementationPlan);
      console.log('\n=============================');

      log.success('Adjutant planning test completed successfully! 🎉');

    } catch (error) {
      log.error(`Test failed: ${error}`);
      process.exit(1);
    } finally {
      await this.stopServer();
    }
  }
}

// Test scenarios
const TEST_SCENARIOS = [
  'Add user authentication with JWT tokens',
  'Implement real-time chat functionality',
  'Add dark mode toggle to the UI',
  'Refactor the database layer to use async/await',
  'Add file upload functionality with progress tracking'
];

async function main() {
  const tester = new AiGatewayTester();

  // Get test prompt from command line args or use default
  const userPrompt = process.argv[2] || TEST_SCENARIOS[0];

  console.log('\n🧪 Available test scenarios:');
  TEST_SCENARIOS.forEach((scenario, i) => {
    const marker = scenario === userPrompt ? '👉' : '  ';
    console.log(`${marker} ${i + 1}. ${scenario}`);
  });
  console.log();

  if (process.argv[2] && !TEST_SCENARIOS.includes(process.argv[2])) {
    console.log(`🎯 Using custom prompt: "${userPrompt}"`);
  }

  // Run the test
  await tester.runFullPlanningTest(userPrompt);
}

// Error handling
process.on('SIGINT', async () => {
  log.warn('Received SIGINT, shutting down gracefully...');
  process.exit(0);
});

process.on('SIGTERM', async () => {
  log.warn('Received SIGTERM, shutting down gracefully...');
  process.exit(0);
});

// Run the test
if (import.meta.main) {
  main().catch(error => {
    log.error(`Test script failed: ${error}`);
    process.exit(1);
  });
}

export { AiGatewayTester };
