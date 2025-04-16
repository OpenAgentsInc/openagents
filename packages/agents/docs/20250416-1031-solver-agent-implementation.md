# Solver Agent Implementation Guide

**Date:** April 16, 2025  
**Author:** Claude Code

## Overview

This document details the implementation of the Solver agent, a new agent type in the OpenAgents platform designed to handle mathematical, logical, and analytical problem-solving tasks. Unlike the Coder agent which focuses on software development tasks, the Solver agent specializes in breaking down complex problems, showing solution steps, and verifying its reasoning.

## Agent Purpose & Capabilities

The Solver agent is designed to:

1. Break down complex problems into manageable steps
2. Apply mathematical and logical reasoning rigorously
3. Show detailed work and explanations for solutions
4. Verify solutions using multiple approaches
5. Handle numerical calculations accurately
6. Explain problem-solving strategies clearly

## Technical Implementation

### 1. State Model

The Solver agent's state is defined by the `SolverState` interface:

```typescript
export interface SolverState {
  messages: UIMessage[];
  githubToken?: string;
  currentProblem?: Problem;
  steps?: SolutionStep[];
  observations?: string[];
  scratchpad?: string;
}

export interface Problem {
  id: string;
  description: string;
  type: 'math' | 'logic' | 'reasoning' | 'other';
  constraints?: string[];
  status: 'unsolved' | 'in-progress' | 'solved' | 'failed';
  created: Date;
  updated?: Date;
  completed?: Date;
}

export interface SolutionStep {
  id: string;
  description: string;
  content: string;
  type: 'assumption' | 'theorem' | 'definition' | 'calculation' | 'proof';
  verified: boolean;
  created: Date;
}
```

This state model tracks:
- The conversation history with the user
- The current problem being solved
- Solution steps with verification status
- Agent observations
- A scratchpad for agent thought processes

### 2. Agent Methods

Key methods of the Solver agent include:

```typescript
class Solver extends Agent<Env, SolverState> {
  // Core methods
  private updateState(partialState: Partial<SolverState>)
  async addAgentObservation(observation: string)
  private async updateScratchpad(thought: string)
  async onMessage(connection: Connection, message: WSMessage)
  
  // Main inference method
  @unstable_callable({ description: "Generate an AI response for problem-solving", streaming: true })
  async infer()
}
```

### 3. Specialized Tools

The Solver agent includes specialized tools for mathematical and logical operations:

```typescript
// Evaluate mathematical expressions
export const evaluateExpression = tool({
  description: "Evaluate a mathematical expression",
  parameters: z.object({
    expression: z.string().describe("The mathematical expression to evaluate")
  }),
  execute: async ({ expression }) => {
    // Implementation for safely evaluating mathematical expressions
  }
});

// Verify mathematical or logical proofs
export const verifyProof = tool({
  description: "Verify a mathematical or logical proof",
  parameters: z.object({
    proof: z.string().describe("The proof to verify"),
    type: z.enum(["mathematical", "logical"]).describe("Type of proof"),
    theorems: z.array(z.string()).optional().describe("Referenced theorems or axioms")
  }),
  execute: async ({ proof, type, theorems }) => {
    // Implementation for proof verification
  }
});
```

### 4. System Prompt

The Solver agent uses a specialized system prompt that emphasizes methodical problem-solving:

```typescript
export function getSolverSystemPrompt(options: SystemPromptOptions): string {
  // ...

  // Base system prompt
  let systemPrompt = `You are an autonomous problem-solving agent specializing in mathematical, logical, and analytical challenges.

PRIMARY FUNCTIONS:
1. Break down complex problems into manageable steps
2. Apply mathematical and logical reasoning
3. Show detailed work and explanations
4. Verify solutions with multiple approaches
5. Handle numerical calculations accurately
6. Explain problem-solving strategies

GUIDELINES:
1. ANALYZE problems thoroughly before solving
2. SHOW all work and intermediate steps
3. VERIFY solutions with different methods
4. EXPLAIN reasoning clearly and concisely
5. USE appropriate mathematical notation
6. CHECK edge cases and assumptions
7. MAINTAIN solution accuracy`;

  // ...
  
  return systemPrompt;
}
```

## Usage Examples

### Example 1: Mathematical Problem Solving

```
User: Solve the quadratic equation 2x² - 5x + 3 = 0

Solver Agent:
I'll solve this quadratic equation step by step.

Step 1: Identify the coefficients.
For the standard form ax² + bx + c = 0:
a = 2, b = -5, c = 3

Step 2: Calculate the discriminant.
Δ = b² - 4ac
Δ = (-5)² - 4(2)(3)
Δ = 25 - 24
Δ = 1

Step 3: Apply the quadratic formula.
x = (-b ± √Δ) / 2a
x = (5 ± √1) / 4
x = (5 ± 1) / 4

So x = 6/4 = 3/2 or x = 4/4 = 1

Verification: Let's check both solutions.
For x = 3/2:
2(3/2)² - 5(3/2) + 3 = 2(9/4) - 15/2 + 3 = 9/2 - 15/2 + 3 = -6/2 + 3 = 0 ✓

For x = 1:
2(1)² - 5(1) + 3 = 2 - 5 + 3 = 0 ✓

The solutions are x = 3/2 and x = 1.
```

### Example 2: Logical Reasoning

```
User: If all roses are flowers and some flowers fade quickly, can we conclude that some roses fade quickly?

Solver Agent:
Let's analyze this logical statement carefully using set theory and syllogisms.

Premises:
1. All roses are flowers. (Universal statement: ∀x(Rose(x) → Flower(x)))
2. Some flowers fade quickly. (Existential statement: ∃x(Flower(x) ∧ FadeQuickly(x)))

Question: Can we conclude that some roses fade quickly? (∃x(Rose(x) ∧ FadeQuickly(x)))

Analysis:
From premise 1, we know that roses are a subset of flowers.
From premise 2, we know that the set of "flowers that fade quickly" is non-empty.

However, we don't know whether the intersection of "roses" and "flowers that fade quickly" is non-empty. The "flowers that fade quickly" could be non-roses.

Visual representation:
- All roses are in the flowers circle
- Some flowers are in the "fade quickly" circle
- But we don't know if any roses are in the "fade quickly" circle

This is a classic example of the fallacy of the undistributed middle term.

Conclusion: No, we cannot logically conclude that some roses fade quickly based on the given premises. Additional information would be needed to make this conclusion.
```

## Integration with UI

The Solver agent can be integrated with the existing UI by adding a selector for agent type. When the "Solver" agent is selected, the interface should adapt to:

1. Show a math/logic-oriented prompt placeholder
2. Potentially include specialized input methods for mathematical notation
3. Display solution steps in a structured format
4. Provide tools for interacting with the solution (e.g., step-by-step navigation)

## Future Enhancements

1. **Specialized Math Rendering**: Add support for LaTeX or MathML rendering
2. **Interactive Proofs**: Allow users to interactively build and verify proofs
3. **External Tool Integration**: Connect to external computation engines for complex calculations
4. **Visualization**: Add support for diagrams and graphs for visual problem-solving
5. **Domain-Specific Modules**: Extend to support specialized domains like physics, statistics, economics, etc.

## Conclusion

The Solver agent represents a significant expansion of the OpenAgents platform's capabilities beyond software development into formal problem-solving. By following this implementation guide, developers can further enhance and extend the Solver agent's capabilities to support a wide range of mathematical, logical, and analytical problem-solving tasks.