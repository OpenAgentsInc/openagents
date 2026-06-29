import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type { Team } from '../../../domain/session'
import { personalFileRouter, teamFileRouter } from '../../../route'
import { formatIsoDateTime } from '../../../time-format'
import * as Ui from '../../../ui'
import {
  ClickedThreadFileDownload,
  ClickedThreadFileDownloadToggle,
  FailedUploadThreadFile,
  type Message,
  SubmittedThreadFileUpload,
} from '../message'
import type {
  Model,
  ThreadFileDetailRecord,
  ThreadFileRecord,
  ThreadFileReferenceRecord,
} from '../model'
import { teamRouteRef, threadFileOwnershipTeamId } from '../model'
import { teamChatThreadId, teamFilesScopeKey } from '../update'

const hComponent = (name: string): Attribute<Message> =>
  html<Message>().DataAttribute('component', name)

const teamForRef = (model: Model, teamRef: string): Team | undefined =>
  model.auth.teams.find(team => teamRouteRef(team) === teamRef)

const formatBytes = (bytes: number): string => {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }

  return `${bytes} B`
}

const fileDownloadEnabled = (file: ThreadFileRecord): boolean =>
  file.downloadEnabled !== false

const fileDetailHref = (
  file: ThreadFileRecord,
  teamRef: string | undefined,
): string =>
  file.scope === 'team' && teamRef !== undefined
    ? teamFileRouter({ teamRef, fileId: file.id })
    : personalFileRouter({ fileId: file.id })

const uploadStatusMessage = (model: Model, scopeKey: string): Html => {
  const h = html<Message>()

  if (
    model.threadFileUpload._tag === 'ThreadFileUploading' &&
    model.threadFileUpload.scopeKey === scopeKey
  ) {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-white/45')],
      ['Uploading...'],
    )
  }

  if (model.threadFileUpload._tag === 'ThreadFileUploadSucceeded') {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-[#00c853]')],
      [model.threadFileUpload.message],
    )
  }

  if (model.threadFileUpload._tag === 'ThreadFileUploadFailed') {
    return h.p(
      [Ui.className<Message>('m-0 text-sm text-[#ff6f00]')],
      [model.threadFileUpload.error],
    )
  }

  return h.div([], [])
}

const uploadPanel = (
  model: Model,
  team: Team,
  scopeKey: string,
  inputId: string,
): Html => {
  const h = html<Message>()

  return Ui.actionPanel<Message>({
    eyebrow: 'Upload',
    title: 'Attach a team file',
    body: 'Files uploaded here are available to this team and can be attached to team threads.',
    action: h.div(
      [Ui.className<Message>('grid min-w-[260px] gap-2')],
      [
        h.input([
          h.Id(inputId),
          h.Name('file'),
          h.Type('file'),
          h.AriaLabel('Upload team file'),
          Ui.className<Message>(
            'block w-full text-sm text-white/60 file:mr-3 file:border file:border-[#333] file:bg-[#080808] file:px-2 file:py-1.5 file:font-[inherit] file:text-[#f1efe8]',
          ),
          h.OnFileChange(files => {
            const file = files[0]

            if (file === undefined) {
              return FailedUploadThreadFile({
                error: 'Choose a file to upload.',
                scopeKey,
              })
            }

            return SubmittedThreadFileUpload({
              file,
              inputId,
              scopeKey,
              teamId: team.id,
              threadId: teamChatThreadId(team.id),
            })
          }),
        ]),
        model.threadFileUpload._tag === 'ThreadFileUploadIdle'
          ? h.p(
              [Ui.className<Message>('m-0 text-sm text-white/35')],
              ['Select a file to upload it.'],
            )
          : h.div([], []),
        uploadStatusMessage(model, scopeKey),
      ],
    ),
  })
}

const fileTable = (
  files: ReadonlyArray<ThreadFileRecord>,
  teamRef: string,
): Html => {
  const h = html<Message>()

  if (files.length === 0) {
    return Ui.emptyState<Message>({
      title: 'No team files yet',
      body: 'Upload a file to make it available to this team.',
    })
  }

  return Ui.tableList<Message>({
    caption: 'Team files',
    columns: [
      { key: 'file', label: 'File' },
      { key: 'size', label: 'Size' },
      { key: 'uploaded', label: 'Uploaded' },
    ],
    rows: files.map(file => ({
      id: file.id,
      cells: {
        file: h.a(
          [
            h.Href(fileDetailHref(file, teamRef)),
            Ui.className<Message>(
              'text-[#f1efe8] underline underline-offset-[3px]',
            ),
          ],
          [file.filename],
        ),
        size: formatBytes(file.sizeBytes),
        uploaded: formatIsoDateTime(file.createdAt),
      },
    })),
  })
}

const fileStatus = (file: ThreadFileRecord): string =>
  fileDownloadEnabled(file) ? 'downloadable' : 'download disabled'

const fileAction = (
  detail: ThreadFileDetailRecord,
  downloadError: string | undefined,
): Html => {
  const h = html<Message>()
  const enabled = fileDownloadEnabled(detail.file)

  return h.div(
    [Ui.className<Message>('grid gap-2')],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center gap-2')],
        [
          enabled
            ? Ui.button<Message>({
                label: 'Download raw file',
                size: 'sm',
                variant: 'primary',
                attrs: [
                  h.Type('button'),
                  h.OnClick(
                    ClickedThreadFileDownload({
                      downloadUrl: detail.file.downloadUrl,
                      fileId: detail.file.id,
                      filename: detail.file.filename,
                    }),
                  ),
                ],
              })
            : h.span(
                [
                  Ui.className<Message>(
                    'border border-[#333] px-3 py-2 text-xs text-white/45',
                  ),
                ],
                ['Raw download disabled'],
              ),
          detail.canManage
            ? Ui.button<Message>({
                label: enabled ? 'Disable download' : 'Enable download',
                size: 'sm',
                variant: 'secondary',
                attrs: [
                  h.Type('button'),
                  h.OnClick(
                    ClickedThreadFileDownloadToggle({
                      downloadEnabled: !enabled,
                      fileId: detail.file.id,
                    }),
                  ),
                ],
              })
            : null,
        ],
      ),
      downloadError === undefined
        ? null
        : h.p(
            [Ui.className<Message>('m-0 text-xs text-[#ff6f00]')],
            [downloadError],
          ),
    ],
  )
}

const referenceTone = (reference: ThreadFileReferenceRecord): Ui.Tone => {
  if (reference.messageKind === 'system') {
    return 'info'
  }

  if (reference.referenceKind.includes('autopilot')) {
    return 'accent'
  }

  return 'neutral'
}

const referenceTitle = (reference: ThreadFileReferenceRecord): string =>
  reference.messageKind === 'system' ? 'Autopilot' : reference.author.name

const referenceList = (
  references: ReadonlyArray<ThreadFileReferenceRecord>,
): Html => {
  const h = html<Message>()

  if (references.length === 0) {
    return Ui.emptyState<Message>({
      title: 'No message references yet',
      body: 'When this file is attached to a team message or used as Autopilot context, those messages will appear here.',
    })
  }

  return h.section(
    [Ui.className<Message>('border border-[#222] bg-[#010102] text-[#f1efe8]')],
    [
      h.div(
        [Ui.className<Message>('border-b border-[#222] px-4 py-3')],
        [
          h.h2([Ui.className<Message>(Ui.titleClass)], ['Message references']),
          h.p(
            [Ui.className<Message>(Ui.metaClass)],
            ['Team messages and AI replies that used this file.'],
          ),
        ],
      ),
      h.ol(
        [h.Role('list'), Ui.className<Message>('m-0 grid list-none p-0')],
        references.map(reference =>
          h.li(
            [
              Ui.className<Message>(
                'grid gap-2 border-b border-[#222] px-4 py-3 last:border-b-0',
              ),
            ],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2',
                  ),
                ],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        Ui.statusDotClass(referenceTone(reference)),
                      ),
                    ],
                    [],
                  ),
                  h.a(
                    [
                      h.Href(reference.href),
                      Ui.className<Message>(
                        'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm text-[#f1efe8] underline underline-offset-[3px] hover:text-[#ffb400]',
                      ),
                    ],
                    [referenceTitle(reference)],
                  ),
                  h.span(
                    [Ui.className<Message>('text-xs text-white/35')],
                    [formatIsoDateTime(reference.createdAt)],
                  ),
                ],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[80ch] text-sm leading-6 text-white/55',
                  ),
                ],
                [reference.excerpt],
              ),
            ],
          ),
        ),
      ),
    ],
  )
}

const detailBody = (
  detail: ThreadFileDetailRecord,
  downloadError: string | undefined,
): Html => {
  const file = detail.file

  return Ui.applicationDetailScreen<Message>({
    eyebrow: file.scope === 'team' ? 'Team file' : 'Personal file',
    title: file.filename,
    body: `${formatBytes(file.sizeBytes)} - ${file.contentType}`,
    action: fileAction(detail, downloadError),
    sections: [
      {
        eyebrow: 'Artifact',
        title: 'File metadata',
        details: [
          { label: 'Status', value: fileStatus(file) },
          { label: 'Scope', value: file.scope },
          { label: 'Thread', value: file.threadId },
          {
            label: 'Uploaded',
            value: formatIsoDateTime(file.createdAt),
          },
        ],
      },
    ],
    aside: Ui.descriptionList<Message>([
      { label: 'File id', value: file.id },
      { label: 'Owner', value: file.ownerUserId },
      { label: 'Messages', value: String(detail.references.length) },
    ]),
    variant: 'sidebar',
  })
}

export const detailView = (
  model: Model,
  input:
    | Readonly<{ fileId: string; variant: 'personal' }>
    | Readonly<{ fileId: string; teamRef: string; variant: 'team' }>,
): Html => {
  const h = html<Message>()
  const team =
    input.variant === 'team' ? teamForRef(model, input.teamRef) : undefined
  const detail = model.threadFileDetailsById[input.fileId]
  const error = model.threadFileDetailErrorsById[input.fileId]

  if (input.variant === 'team' && team === undefined) {
    return h.div(
      [hComponent('thread-file-detail-page')],
      [
        Ui.pageHeader<Message>({
          eyebrow: 'File',
          title: 'Team not found',
          body: 'This team room is not available in the current session.',
        }),
      ],
    )
  }

  if (error !== undefined && detail === undefined) {
    return h.div(
      [hComponent('thread-file-detail-page')],
      [
        Ui.pageHeader<Message>({
          eyebrow: 'File',
          title: 'File unavailable',
          body: error,
        }),
      ],
    )
  }

  if (detail === undefined) {
    return h.div(
      [hComponent('thread-file-detail-page')],
      [
        Ui.pageHeader<Message>({
          eyebrow: 'File',
          title: 'Loading file',
          body: 'Loading artifact metadata and references.',
        }),
      ],
    )
  }

  if (
    input.variant === 'team' &&
    threadFileOwnershipTeamId(detail.file.ownership) !== team?.id
  ) {
    return h.div(
      [hComponent('thread-file-detail-page')],
      [
        Ui.pageHeader<Message>({
          eyebrow: 'File',
          title: 'File unavailable',
          body: 'This file is not part of the selected team room.',
        }),
      ],
    )
  }

  return h.div(
    [
      hComponent('thread-file-detail-page'),
      Ui.className<Message>('grid gap-4'),
    ],
    [
      detailBody(detail, model.threadFileDownloadErrorsById[detail.file.id]),
      h.div(
        [Ui.className<Message>('px-4 pb-6')],
        [referenceList(detail.references)],
      ),
      error === undefined
        ? null
        : h.p(
            [Ui.className<Message>('m-0 px-4 pb-4 text-sm text-[#ff6f00]')],
            [error],
          ),
    ],
  )
}

export const view = (model: Model, teamRef: string): Html => {
  const h = html<Message>()
  const team = teamForRef(model, teamRef)

  if (team === undefined) {
    return h.div(
      [hComponent('team-files-page')],
      [
        Ui.pageHeader<Message>({
          eyebrow: 'Files',
          title: 'Team not found',
          body: 'This team room is not available in the current session.',
        }),
      ],
    )
  }

  const scopeKey = teamFilesScopeKey(team.id)
  const inputId = `team-file-upload-${team.id.replace(/[^a-zA-Z0-9_-]/g, '-')}`
  const files = model.threadFilesByScope[scopeKey] ?? []

  return h.div(
    [hComponent('team-files-page'), Ui.className<Message>('grid gap-4')],
    [
      Ui.pageHeader<Message>({
        eyebrow: 'Files',
        title: `${team.name} files`,
        body: 'Team-scoped uploads for workroom threads.',
      }),
      h.div(
        [Ui.className<Message>('grid gap-4 px-4 pb-6 pt-4')],
        [
          uploadPanel(model, team, scopeKey, inputId),
          fileTable(files, teamRef),
        ],
      ),
    ],
  )
}
