import { makeAssistantTool } from "@openagentsinc/assistant-ui-runtime";
import { z } from "zod";

export const CalculatorTool = makeAssistantTool({
  toolName: "calculator",
  description: "Perform basic mathematical calculations. Supports addition, subtraction, multiplication, and division.",
  parameters: z.object({
    operation: z.enum(["add", "subtract", "multiply", "divide"]).describe("The mathematical operation to perform"),
    a: z.number().describe("The first number"),
    b: z.number().describe("The second number"),
  }),
  execute: async ({ operation, a, b }) => {
    let result: number;

    switch (operation) {
      case "add":
        result = a + b;
        break;
      case "subtract":
        result = a - b;
        break;
      case "multiply":
        result = a * b;
        break;
      case "divide":
        if (b === 0) {
          throw new Error("Cannot divide by zero");
        }
        result = a / b;
        break;
    }

    return {
      operation,
      a,
      b,
      result,
      expression: `${a} ${operation === "add" ? "+" : operation === "subtract" ? "-" : operation === "multiply" ? "×" : "÷"} ${b} = ${result}`,
    };
  },
});
