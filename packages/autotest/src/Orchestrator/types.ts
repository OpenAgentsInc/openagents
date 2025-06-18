import type { InteractionStep } from "../Testing/types.js"

export interface ProjectConfig {
  readonly root: string
  readonly startCommand: string
  readonly port?: number
  readonly readyPattern?: RegExp
  readonly env?: Record<string, string>
}

export interface TestingConfig {
  readonly baseUrl?: string
  readonly routes: ReadonlyArray<string>
  readonly interactions?: ReadonlyArray<RouteInteraction>
  readonly timeout?: number
}

export interface RouteInteraction {
  readonly route: string
  readonly actions: ReadonlyArray<InteractionStep | string>
}

export interface MonitoringConfig {
  readonly captureConsole?: boolean
  readonly captureNetwork?: boolean
  readonly captureErrors?: boolean
  readonly screenshotOnError?: boolean
}

export interface OrchestratorConfig {
  readonly project: ProjectConfig
  readonly testing: TestingConfig
  readonly monitoring?: MonitoringConfig
}

export interface RouteTestResult {
  readonly route: string
  readonly success: boolean
  readonly duration: number
  readonly errors: ReadonlyArray<TestError>
  readonly screenshots: ReadonlyArray<string>
  readonly console: ReadonlyArray<ConsoleMessage>
  readonly network: ReadonlyArray<NetworkRequest>
}

export interface TestError {
  readonly type: "navigation" | "interaction" | "console" | "network" | "assertion"
  readonly message: string
  readonly stack?: string
  readonly timestamp: Date
}

export interface ConsoleMessage {
  readonly type: "log" | "warn" | "error" | "info"
  readonly text: string
  readonly timestamp: Date
}

export interface NetworkRequest {
  url: string
  method: string
  status?: number
  duration?: number
  error?: string
}

export interface TestReport {
  readonly startedAt: Date
  readonly completedAt: Date
  readonly duration: number
  readonly serverLogs: ReadonlyArray<string>
  readonly routes: ReadonlyArray<RouteTestResult>
  readonly summary: TestSummary
  readonly suggestedFixes?: ReadonlyArray<SuggestedFix>
}

export interface TestSummary {
  readonly totalRoutes: number
  readonly passedRoutes: number
  readonly failedRoutes: number
  readonly totalErrors: number
  readonly errorsByType: Record<string, number>
}

export interface SuggestedFix {
  readonly issue: string
  readonly description: string
  readonly file?: string
  readonly line?: number
  readonly suggestion: string
}