import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { replaceThreadFileInScopes } from '../chatState'
import { Message } from '../message'
import {
  FailedThreadFileUpload,
  Model,
  SucceededThreadFileUpload,
  UploadingThreadFile,
  threadFileDetailFromDto,
  threadFileRecordFromDto,
} from '../model'
import {
  noUpdate,
  type UpdateReturn,
} from '../transition'
import {
  DownloadThreadFile,
  LoadThreadFileDetail,
  LoadThreadFiles,
  UpdateThreadFileDownload,
  UploadThreadFile,
} from './commands'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const removeFileError = (
  errors: Readonly<Record<string, string>>,
  fileId: string,
): Readonly<Record<string, string>> => {
  const { [fileId]: _removed, ...nextErrors } = errors

  return nextErrors
}

export const updateThreadFiles = (
  model: Model,
  message: Message,
): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadThreadFiles: ({ href, scopeKey }) => [
        model,
        [LoadThreadFiles({ href, scopeKey })],
        Option.none(),
      ],
      SucceededLoadThreadFiles: ({ response, scopeKey }) => [
        evo(model, {
          threadFilesByScope: filesByScope => ({
            ...filesByScope,
            [scopeKey]: response.files.map(threadFileRecordFromDto),
          }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadThreadFiles: ({ error }) => [
        evo(model, {
          threadFileUpload: () => FailedThreadFileUpload({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadThreadFileDetail: ({ fileId, href }) => [
        model,
        [LoadThreadFileDetail({ fileId, href })],
        Option.none(),
      ],
      SucceededLoadThreadFileDetail: ({ fileId, response }) => {
        const detail = threadFileDetailFromDto(response.detail)

        return [
          evo(model, {
            threadFileDetailErrorsById: errors =>
              removeFileError(errors, fileId),
            threadFileDetailsById: details => ({
              ...details,
              [fileId]: detail,
            }),
            threadFilesByScope: filesByScope =>
              replaceThreadFileInScopes(filesByScope, detail.file),
          }),
          [],
          Option.none(),
        ]
      },
      FailedLoadThreadFileDetail: ({ error, fileId }) => [
        evo(model, {
          threadFileDetailErrorsById: errors => ({
            ...errors,
            [fileId]: error,
          }),
        }),
        [],
        Option.none(),
      ],
      ClickedThreadFileDownload: ({ downloadUrl, fileId, filename }) => [
        evo(model, {
          threadFileDownloadErrorsById: errors =>
            removeFileError(errors, fileId),
        }),
        [DownloadThreadFile({ downloadUrl, fileId, filename })],
        Option.none(),
      ],
      SucceededDownloadThreadFile: ({ fileId }) => [
        evo(model, {
          threadFileDownloadErrorsById: errors =>
            removeFileError(errors, fileId),
        }),
        [],
        Option.none(),
      ],
      FailedDownloadThreadFile: ({ error, fileId }) => [
        evo(model, {
          threadFileDownloadErrorsById: errors => ({
            ...errors,
            [fileId]: error,
          }),
        }),
        [],
        Option.none(),
      ],
      ClickedThreadFileDownloadToggle: ({ downloadEnabled, fileId }) => [
        model,
        [UpdateThreadFileDownload({ downloadEnabled, fileId })],
        Option.none(),
      ],
      SucceededUpdateThreadFileDownload: ({ fileId, response }) => {
        const detail = threadFileDetailFromDto(response.detail)

        return [
          evo(model, {
            threadFileDetailErrorsById: errors =>
              removeFileError(errors, fileId),
            threadFileDetailsById: details => ({
              ...details,
              [fileId]: detail,
            }),
            threadFilesByScope: filesByScope =>
              replaceThreadFileInScopes(filesByScope, detail.file),
          }),
          [],
          Option.none(),
        ]
      },
      FailedUpdateThreadFileDownload: ({ error, fileId }) => [
        evo(model, {
          threadFileDetailErrorsById: errors => ({
            ...errors,
            [fileId]: error,
          }),
        }),
        [],
        Option.none(),
      ],
      SubmittedThreadFileUpload: ({
        file,
        inputId,
        scopeKey,
        teamId,
        threadId,
      }) => [
        evo(model, {
          threadFileUpload: () => UploadingThreadFile({ scopeKey }),
        }),
        [UploadThreadFile({ file, inputId, scopeKey, teamId, threadId })],
        Option.none(),
      ],
      SucceededUploadThreadFile: ({ response, scopeKey }) => [
        evo(model, {
          threadFileUpload: () =>
            SucceededThreadFileUpload({
              message: `${response.file.filename} uploaded.`,
            }),
          threadFilesByScope: filesByScope => {
            const uploadedFile = threadFileRecordFromDto(response.file)
            const existing = filesByScope[scopeKey] ?? []

            return {
              ...filesByScope,
              [scopeKey]: [
                uploadedFile,
                ...existing.filter(file => file.id !== uploadedFile.id),
              ],
            }
          },
        }),
        [],
        Option.none(),
      ],
      FailedUploadThreadFile: ({ error }) => [
        evo(model, {
          threadFileUpload: () => FailedThreadFileUpload({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
