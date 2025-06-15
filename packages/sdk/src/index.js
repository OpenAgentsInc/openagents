import { Effect, Console } from "effect"

export const helloWorld = Console.log("Hello from OpenAgents SDK!")

export const runHelloWorld = () => Effect.runSync(helloWorld)