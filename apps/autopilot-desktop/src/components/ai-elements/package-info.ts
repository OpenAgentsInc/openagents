import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { cx, type AIChildren } from "./utils.js"

export type ChangeType = "major" | "minor" | "patch" | "added" | "removed"

export type PackageInfoProps = {
  readonly name: string
  readonly currentVersion?: string
  readonly newVersion?: string
  readonly changeType?: ChangeType
  readonly className?: string
  readonly children?: AIChildren
}

export const PackageInfo = ({ name, currentVersion, newVersion, changeType, className, children }: PackageInfoProps): TemplateResult => html`
  <div class="${cx("rounded-lg border bg-background p-4", className)}">
    ${children ?? html`
      ${PackageInfoHeader({ children: html`${PackageInfoName({ name })}${changeType ? PackageInfoChangeType({ changeType }) : ""}` })}
      ${(currentVersion || newVersion) ? PackageInfoVersion({ currentVersion, newVersion }) : ""}
    `}
  </div>
`

export type PackageInfoHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PackageInfoHeader = ({ className, children }: PackageInfoHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between gap-2", className)}">${children ?? ""}</div>
`

export type PackageInfoNameProps = {
  readonly className?: string
  readonly name?: string
  readonly children?: AIChildren
}

export const PackageInfoName = ({ className, name, children }: PackageInfoNameProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2", className)}">
    <span class="size-4 text-muted-foreground">pkg</span>
    <span class="font-medium font-mono text-sm">${children ?? name ?? ""}</span>
  </div>
`

const changeTypeStyles: Record<ChangeType, string> = {
  major: "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
  minor: "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
  patch: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
  added: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
  removed: "bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400",
}

export type PackageInfoChangeTypeProps = {
  readonly className?: string
  readonly changeType?: ChangeType
  readonly children?: AIChildren
}

export const PackageInfoChangeType = ({ className, changeType, children }: PackageInfoChangeTypeProps): TemplateResult => {
  if (!changeType) {
    return html``
  }
  return Badge({
    className: cx("gap-1", changeTypeStyles[changeType], className),
    variant: "secondary",
    children: children ?? changeType,
  })
}

export type PackageInfoVersionProps = {
  readonly className?: string
  readonly currentVersion?: string
  readonly newVersion?: string
}

export const PackageInfoVersion = ({ className, currentVersion, newVersion }: PackageInfoVersionProps): TemplateResult => html`
  <div class="${cx("mt-2 flex items-center gap-2 font-mono text-xs text-muted-foreground", className)}">
    ${currentVersion ? html`<span>${currentVersion}</span>` : ""}
    ${currentVersion && newVersion ? html`<span>-></span>` : ""}
    ${newVersion ? html`<span>${newVersion}</span>` : ""}
  </div>
`

export type PackageInfoDescriptionProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PackageInfoDescription = ({ className, children }: PackageInfoDescriptionProps): TemplateResult => html`
  <p class="${cx("mt-2 text-sm text-muted-foreground", className)}">${children ?? ""}</p>
`

export type PackageInfoContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PackageInfoContent = ({ className, children }: PackageInfoContentProps): TemplateResult => html`
  <div class="${cx("mt-3 space-y-2", className)}">${children ?? ""}</div>
`

export type PackageInfoDependenciesProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PackageInfoDependencies = ({ className, children }: PackageInfoDependenciesProps): TemplateResult => html`
  <div class="${cx("space-y-1", className)}">${children ?? ""}</div>
`

export type PackageInfoDependencyProps = {
  readonly className?: string
  readonly name?: string
  readonly version?: string
  readonly children?: AIChildren
}

export const PackageInfoDependency = ({ className, name, version, children }: PackageInfoDependencyProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between text-xs font-mono", className)}">
    <span>${children ?? name ?? ""}</span>
    ${version ? html`<span class="text-muted-foreground">${version}</span>` : ""}
  </div>
`
