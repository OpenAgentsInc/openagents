import { Effect, Queue, Schema as S, Stream } from 'effect'
import * as Mount from 'foldkit/mount'

import { SelectedCustomerSiteElementContext } from './message'
import {
  SiteElementContext,
  safeSiteElementContext,
} from './site-element-context'

export const SITE_ELEMENT_TARGET_MESSAGE_TYPE = 'openagents.site.elementTarget'

export type SitePreviewBridgePayload = Readonly<{
  type: typeof SITE_ELEMENT_TARGET_MESSAGE_TYPE
  selector: string
  tag: string
  text?: string | null
  attributes?: ReadonlyArray<Readonly<{ name: string; value: string }>>
}>

const recordValue = (value: unknown): Record<string, unknown> | null =>
  typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' ? value : undefined

const attributesValue = (
  value: unknown,
): ReadonlyArray<Readonly<{ name: string; value: string }>> => {
  if (!Array.isArray(value)) {
    return []
  }

  return value.flatMap(attribute => {
    const record = recordValue(attribute)
    const name = stringValue(record?.name)
    const attributeValue = stringValue(record?.value)

    return name === undefined || attributeValue === undefined
      ? []
      : [{ name, value: attributeValue }]
  })
}

export const siteElementContextFromBridgePayload = (
  payload: unknown,
): SiteElementContext | null => {
  const record = recordValue(payload)

  if (record?.type !== SITE_ELEMENT_TARGET_MESSAGE_TYPE) {
    return null
  }

  const selector = stringValue(record.selector)
  const tag = stringValue(record.tag)

  if (selector === undefined || tag === undefined) {
    return null
  }

  const text =
    record.text === null ? null : stringValue(record.text)

  return safeSiteElementContext({
    attributes: attributesValue(record.attributes),
    selector,
    tag,
    ...(text === undefined ? {} : { text }),
  })
}

export const InstallSitePreviewElementTargetBridge = Mount.defineStream(
  'InstallSitePreviewElementTargetBridge',
  {
    allowedOrigin: S.String,
  },
  SelectedCustomerSiteElementContext,
)(({ allowedOrigin }) =>
  _element =>
    Stream.callback<typeof SelectedCustomerSiteElementContext.Type>(queue =>
      Effect.gen(function* () {
        yield* Effect.acquireRelease(
          Effect.sync(() => {
            const handler = (event: MessageEvent) => {
              if (event.origin !== allowedOrigin) {
                return
              }

              const context = siteElementContextFromBridgePayload(event.data)

              if (context !== null) {
                Queue.offerUnsafe(
                  queue,
                  SelectedCustomerSiteElementContext({ context }),
                )
              }
            }

            window.addEventListener('message', handler)

            return handler
          }),
          handler =>
            Effect.sync(() => {
              window.removeEventListener('message', handler)
            }),
        )

        return yield* Effect.never
      }),
    ),
)
