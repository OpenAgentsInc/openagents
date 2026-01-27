import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Progress } from "../ui/progress.js"
import { cx, type AIChildren } from "./utils.js"

export type TestResultsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TestResults = ({ className, children }: TestResultsProps): TemplateResult => html`
  <div class="${cx("overflow-hidden rounded-lg border bg-background", className)}">${children ?? ""}</div>
`

export type TestResultsHeaderProps = { readonly className?: string; readonly children?: AIChildren }
export const TestResultsHeader = ({ className, children }: TestResultsHeaderProps): TemplateResult =>
  html`<div class="${cx("flex items-center justify-between border-b px-4 py-3", className)}">${children ?? ""}</div>`

export type TestResultsSummaryProps = { readonly className?: string; readonly children?: AIChildren }
export const TestResultsSummary = ({ className, children }: TestResultsSummaryProps): TemplateResult =>
  html`<div class="${cx("flex items-center gap-2 text-sm", className)}">${children ?? "Test Results"}</div>`

export type TestResultsDurationProps = { readonly className?: string; readonly children?: AIChildren }
export const TestResultsDuration = ({ className, children }: TestResultsDurationProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "0s"}</span>`

export type TestResultsProgressProps = { readonly className?: string; readonly value?: number }
export const TestResultsProgress = ({ className, value = 0 }: TestResultsProgressProps): TemplateResult =>
  Progress({ className: cx("h-1", className), value })

export type TestResultsContentProps = { readonly className?: string; readonly children?: AIChildren }
export const TestResultsContent = ({ className, children }: TestResultsContentProps): TemplateResult =>
  html`<div class="${cx("divide-y", className)}">${children ?? ""}</div>`

export type TestSuiteProps = { readonly className?: string; readonly children?: AIChildren }
export const TestSuite = ({ className, children }: TestSuiteProps): TemplateResult =>
  html`<div class="${cx("px-4 py-3", className)}">${children ?? ""}</div>`

export type TestSuiteNameProps = { readonly className?: string; readonly children?: AIChildren }
export const TestSuiteName = ({ className, children }: TestSuiteNameProps): TemplateResult =>
  html`<div class="${cx("text-sm font-medium", className)}">${children ?? "Suite"}</div>`

export type TestSuiteStatsProps = { readonly className?: string; readonly children?: AIChildren }
export const TestSuiteStats = ({ className, children }: TestSuiteStatsProps): TemplateResult =>
  html`<div class="${cx("text-xs text-muted-foreground", className)}">${children ?? "0 passed"}</div>`

export type TestSuiteContentProps = { readonly className?: string; readonly children?: AIChildren }
export const TestSuiteContent = ({ className, children }: TestSuiteContentProps): TemplateResult =>
  html`<div class="${cx("mt-2 space-y-2", className)}">${children ?? ""}</div>`

export type TestProps = { readonly className?: string; readonly children?: AIChildren }
export const Test = ({ className, children }: TestProps): TemplateResult =>
  html`<div class="${cx("flex items-start justify-between gap-3 text-sm", className)}">${children ?? ""}</div>`

export type TestStatusProps = { readonly className?: string; readonly children?: AIChildren }
export const TestStatus = ({ className, children }: TestStatusProps): TemplateResult =>
  html`<span class="${cx("text-xs uppercase tracking-wide text-muted-foreground", className)}">${children ?? "pass"}</span>`

export type TestNameProps = { readonly className?: string; readonly children?: AIChildren }
export const TestName = ({ className, children }: TestNameProps): TemplateResult =>
  html`<span class="${cx("font-medium", className)}">${children ?? "Test"}</span>`

export type TestDurationProps = { readonly className?: string; readonly children?: AIChildren }
export const TestDuration = ({ className, children }: TestDurationProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "0ms"}</span>`

export type TestErrorProps = { readonly className?: string; readonly children?: AIChildren }
export const TestError = ({ className, children }: TestErrorProps): TemplateResult =>
  html`<div class="${cx("mt-2 rounded-md border border-destructive/40 bg-destructive/10 p-2 text-xs text-destructive", className)}">${children ?? ""}</div>`

export type TestErrorMessageProps = { readonly className?: string; readonly children?: AIChildren }
export const TestErrorMessage = ({ className, children }: TestErrorMessageProps): TemplateResult =>
  html`<div class="${cx("font-medium", className)}">${children ?? "Error"}</div>`

export type TestErrorStackProps = { readonly className?: string; readonly children?: AIChildren }
export const TestErrorStack = ({ className, children }: TestErrorStackProps): TemplateResult =>
  html`<pre class="${cx("mt-1 whitespace-pre-wrap text-[11px]", className)}">${children ?? ""}</pre>`
