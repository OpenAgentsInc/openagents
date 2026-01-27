import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type ImageProps = {
  readonly base64?: string
  readonly uint8Array?: Uint8Array
  readonly mediaType?: string
  readonly className?: string
  readonly alt?: string
  readonly src?: string
}

export const Image = ({ base64, mediaType, className, alt, src }: ImageProps): TemplateResult => {
  const derivedSrc = src ?? (base64 && mediaType ? `data:${mediaType};base64,${base64}` : "")
  return html`
    <img
      alt="${alt ?? ""}"
      class="${cx("h-auto max-w-full overflow-hidden rounded-md", className)}"
      src="${derivedSrc}"
    />
  `
}
