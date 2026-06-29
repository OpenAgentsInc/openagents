import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { File as FileSchema } from 'foldkit/file'

import {
  type ChatApiError,
  errorFromUnknown,
  errorMessageFromUnknown,
  requestBlob,
  requestJson,
} from '../commands/api'
import {
  FailedDownloadThreadFile,
  FailedLoadThreadFileDetail,
  FailedLoadThreadFiles,
  FailedUpdateThreadFileDownload,
  FailedUploadThreadFile,
  SucceededDownloadThreadFile,
  SucceededLoadThreadFileDetail,
  SucceededLoadThreadFiles,
  SucceededUpdateThreadFileDownload,
  SucceededUploadThreadFile,
} from '../message'
import {
  ThreadFileDetailResponse,
  ThreadFileUploadResponse,
  ThreadFilesResponse,
} from '../model'

export const LoadThreadFiles = Command.define(
  'LoadThreadFiles',
  { href: S.String, scopeKey: S.String },
  SucceededLoadThreadFiles,
  FailedLoadThreadFiles,
)(({ href, scopeKey }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.threadFiles.load',
      request: href,
      schema: ThreadFilesResponse,
    })

    return SucceededLoadThreadFiles({ response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadThreadFiles({
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)

export const LoadThreadFileDetail = Command.define(
  'LoadThreadFileDetail',
  { fileId: S.String, href: S.String },
  SucceededLoadThreadFileDetail,
  FailedLoadThreadFileDetail,
)(({ fileId, href }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.threadFile.detail.load',
      request: href,
      schema: ThreadFileDetailResponse,
    })

    return SucceededLoadThreadFileDetail({ fileId, response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadThreadFileDetail({
          error: errorMessageFromUnknown(error),
          fileId,
        }),
      ),
    ),
  ),
)

const saveBlobAsDownload = (
  blob: Blob,
  filename: string,
): Effect.Effect<void, ChatApiError> =>
  Effect.try({
    try: () => {
      if (typeof document === 'undefined') {
        return
      }

      const href = globalThis.URL.createObjectURL(blob)

      try {
        const anchor = document.createElement('a')
        anchor.download = filename
        anchor.href = href
        anchor.rel = 'noopener'
        anchor.style.display = 'none'
        document.body.append(anchor)
        anchor.click()
        anchor.remove()
      } finally {
        globalThis.URL.revokeObjectURL(href)
      }
    },
    catch: errorFromUnknown,
  })

export const DownloadThreadFile = Command.define(
  'DownloadThreadFile',
  { downloadUrl: S.String, fileId: S.String, filename: S.String },
  SucceededDownloadThreadFile,
  FailedDownloadThreadFile,
)(({ downloadUrl, fileId, filename }) =>
  Effect.gen(function* () {
    const blob = yield* requestBlob({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: '*/*' },
      },
      name: 'loggedIn.threadFile.download',
      request: downloadUrl,
    })

    yield* saveBlobAsDownload(blob, filename)

    return SucceededDownloadThreadFile({ fileId })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedDownloadThreadFile({
          error: errorMessageFromUnknown(error),
          fileId,
        }),
      ),
    ),
  ),
)

export const UpdateThreadFileDownload = Command.define(
  'UpdateThreadFileDownload',
  { downloadEnabled: S.Boolean, fileId: S.String },
  SucceededUpdateThreadFileDownload,
  FailedUpdateThreadFileDownload,
)(({ downloadEnabled, fileId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ downloadEnabled }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'PATCH',
      },
      name: 'loggedIn.threadFile.download.update',
      request: `/api/thread-files/${encodeURIComponent(fileId)}`,
      schema: ThreadFileDetailResponse,
    })

    return SucceededUpdateThreadFileDownload({ fileId, response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedUpdateThreadFileDownload({
          error: errorMessageFromUnknown(error),
          fileId,
        }),
      ),
    ),
  ),
)

export const UploadThreadFile = Command.define(
  'UploadThreadFile',
  {
    file: FileSchema,
    inputId: S.String,
    scopeKey: S.String,
    teamId: S.NullOr(S.String),
    threadId: S.String,
  },
  SucceededUploadThreadFile,
  FailedUploadThreadFile,
)(({ file, inputId, scopeKey, teamId, threadId }) =>
  Effect.gen(function* () {
    const form = new FormData()
    form.set('file', file)
    form.set('threadId', threadId)

    if (teamId !== null) {
      form.set('teamId', teamId)
    }

    const uploadAbort = yield* Effect.sync(() => {
      const controller = new AbortController()
      const timeout = globalThis.setTimeout(() => {
        controller.abort()
      }, 30000)

      return { signal: controller.signal, timeout } as const
    })

    const response = yield* requestJson({
      catch: error =>
        error instanceof DOMException && error.name === 'AbortError'
          ? new Error('File upload timed out after 30 seconds.')
          : error,
      init: {
        body: form,
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
        method: 'POST',
        signal: uploadAbort.signal,
      },
      name: 'loggedIn.threadFile.upload',
      request: '/api/thread-files',
      schema: ThreadFileUploadResponse,
    }).pipe(
      Effect.ensuring(
        Effect.sync(() => globalThis.clearTimeout(uploadAbort.timeout)),
      ),
    )

    if (typeof document !== 'undefined') {
      const input = document.getElementById(inputId)

      if (input instanceof HTMLInputElement) {
        input.value = ''
      }
    }

    return SucceededUploadThreadFile({ response, scopeKey })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedUploadThreadFile({
          error: errorMessageFromUnknown(error),
          scopeKey,
        }),
      ),
    ),
  ),
)
