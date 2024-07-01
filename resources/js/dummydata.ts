export const initialMessage = `Please implement the following diagram in the v2 branch of our GitHub repo openagentsinc/openagents.

<image>
# Diagram Description: Agentic Loop and Building Blocks

The image shows a diagram with two main sections: "Agentic Loop" on the left and "Building Blocks" on the right. The background is black, and the text is in white.

## Agentic Loop

The Agentic Loop section contains six boxes arranged vertically, each with dashed borders. The steps are:

1. "User asks AutoDev to do something"
2. "AutoDev finds relevant context via Greptile/Ragie"
3. "AutoDev makes a plan"
4. "AutoDev edits files"
5. "User is prompted to confirm"
6. "Changes are merged"

## Building Blocks

The Building Blocks section contains six boxes arranged vertically, with a mix of solid and dashed borders. The components are:

1. "Index & query codebases" (solid border)
2. "Read GitHub repo" (solid border)
3. "Planner" (dashed border)
4. "Make code edits" (dashed border)
5. "Write to GitHub repo" (dashed border)
6. "Manage artifacts" (dashed border)

The diagram suggests a process flow where the Agentic Loop represents the high-level steps of an automated development process, while the Building Blocks represent the underlying components or actions that support each step in the loop.
</image>`;

export const examplePlan = `
Current Focus: Read the README and any additional documentation to understand the project setup and requirements.

Plan:

-[x] Clone the repository from the provided GitHub link.
-[ ] Read the README and any additional documentation to understand the project setup and requirements.
-[ ] Identify the package manager aand install necessary dependencies.
-[ ] Set up the development environment according to the project's instructions.
-[ ] Run the project locally to ensure it's working correctly.
-[ ] Test the project UI to confirm functionality.`;
