import { Effect, Match as M, Option, Schema as S } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedAdminAdjutantEnrichmentAction,
  FailedAdminSiteDeploymentAction,
  FailedGenerateAdminSite,
  FailedLoadAdminAdjutantAssignments,
  FailedLoadAdminAdjutantReview,
  FailedLoadAdminOverview,
  Message,
  SucceededAdminAdjutantEnrichmentAction,
  SucceededAdminSiteDeploymentAction,
  SucceededGenerateAdminSite,
  SucceededLoadAdminAdjutantAssignments,
  SucceededLoadAdminAdjutantReview,
  SucceededLoadAdminOverview,
} from '../message'
import {
  AdminAdjutantEnrichmentActionFailed,
  AdminAdjutantEnrichmentActionPending,
  AdminAdjutantEnrichmentActionResponse,
  AdminAdjutantEnrichmentActionSucceeded,
  AdminAdjutantAssignmentReviewResponse,
  AdminAdjutantAssignmentsFailed,
  AdminAdjutantAssignmentsLoaded,
  AdminAdjutantAssignmentsLoading,
  AdminAdjutantAssignmentsResponse,
  AdminAdjutantReviewFailed,
  AdminAdjutantReviewLoaded,
  AdminAdjutantReviewLoading,
  AdminOverviewFailed,
  AdminOverviewLoaded,
  AdminOverviewLoading,
  AdminOverviewResponse,
  AdminSiteDeploymentActionFailed,
  AdminSiteDeploymentActionPending,
  AdminSiteDeploymentActionSucceeded,
  AdminSiteGenerationResponse,
  AdminSiteDeploymentActionResponse,
  Model,
} from '../model'
import { type UpdateReturn } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

export const LoadAdminOverview = Command.define(
  'LoadAdminOverview',
  SucceededLoadAdminOverview,
  FailedLoadAdminOverview,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.admin.overview.load',
      request: '/api/admin/overview',
      schema: AdminOverviewResponse,
    })

    return SucceededLoadAdminOverview({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAdminOverview({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const GenerateAdminSite = Command.define(
  'GenerateAdminSite',
  { siteId: S.String },
  SucceededGenerateAdminSite,
  FailedGenerateAdminSite,
)(({ siteId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: '{}',
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.admin.site.generate',
      request: `/api/operator/sites/${encodeURIComponent(siteId)}/generate`,
      schema: AdminSiteGenerationResponse,
    })

    return SucceededGenerateAdminSite({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedGenerateAdminSite({
          error: errorMessageFromUnknown(error),
          siteId,
        }),
      ),
    ),
  ),
)

export const LoadAdminAdjutantAssignments = Command.define(
  'LoadAdminAdjutantAssignments',
  SucceededLoadAdminAdjutantAssignments,
  FailedLoadAdminAdjutantAssignments,
)(
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.admin.adjutant.assignments.load',
      request: '/api/operator/adjutant/assignments',
      schema: AdminAdjutantAssignmentsResponse,
    })

    return SucceededLoadAdminAdjutantAssignments({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAdminAdjutantAssignments({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const LoadAdminAdjutantReview = Command.define(
  'LoadAdminAdjutantReview',
  { assignmentId: S.String },
  SucceededLoadAdminAdjutantReview,
  FailedLoadAdminAdjutantReview,
)(({ assignmentId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.admin.adjutant.review.load',
      request: `/api/operator/adjutant/assignments/${encodeURIComponent(assignmentId)}`,
      schema: AdminAdjutantAssignmentReviewResponse,
    })

    return SucceededLoadAdminAdjutantReview({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadAdminAdjutantReview({
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const RunAdminAdjutantEnrichment = Command.define(
  'RunAdminAdjutantEnrichment',
  { assignmentId: S.String, refresh: S.optionalKey(S.Boolean) },
  SucceededAdminAdjutantEnrichmentAction,
  FailedAdminAdjutantEnrichmentAction,
)(({ assignmentId, refresh }) =>
  Effect.gen(function* () {
    const action = refresh === true ? 'refresh' : 'run'
    const response = yield* requestJson({
      init: {
        body: '{}',
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: `loggedIn.admin.adjutant.enrichment.${action}`,
      request: `/api/operator/adjutant/assignments/${encodeURIComponent(assignmentId)}/enrichment/${action}`,
      schema: AdminAdjutantEnrichmentActionResponse,
    })

    return SucceededAdminAdjutantEnrichmentAction({
      action,
      assignmentId,
      message: `Enrichment ${response.enrichment.status}.`,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAdminAdjutantEnrichmentAction({
          action: refresh === true ? 'refresh' : 'run',
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const ReviewAdminAdjutantSourceCard = Command.define(
  'ReviewAdminAdjutantSourceCard',
  {
    assignmentId: S.String,
    reviewStatus: S.String,
    sourceId: S.String,
  },
  SucceededAdminAdjutantEnrichmentAction,
  FailedAdminAdjutantEnrichmentAction,
)(({ assignmentId, reviewStatus, sourceId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          publicSafe: reviewStatus === 'public_safe',
          reviewStatus,
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.admin.adjutant.enrichment.sourceCard.review',
      request: `/api/operator/adjutant/assignments/${encodeURIComponent(assignmentId)}/enrichment/source-cards/${encodeURIComponent(sourceId)}/review`,
      schema: AdminAdjutantEnrichmentActionResponse,
    })

    return SucceededAdminAdjutantEnrichmentAction({
      action: 'source-card-review',
      assignmentId,
      message: `Source ${reviewStatus}.`,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAdminAdjutantEnrichmentAction({
          action: 'source-card-review',
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const ReviewAdminAdjutantResearchBrief = Command.define(
  'ReviewAdminAdjutantResearchBrief',
  {
    assignmentId: S.String,
    briefId: S.String,
    status: S.String,
  },
  SucceededAdminAdjutantEnrichmentAction,
  FailedAdminAdjutantEnrichmentAction,
)(({ assignmentId, briefId, status }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ status }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.admin.adjutant.enrichment.brief.review',
      request: `/api/operator/adjutant/assignments/${encodeURIComponent(assignmentId)}/enrichment/briefs/${encodeURIComponent(briefId)}/review`,
      schema: AdminAdjutantEnrichmentActionResponse,
    })

    return SucceededAdminAdjutantEnrichmentAction({
      action: 'research-brief-review',
      assignmentId,
      message: `Brief ${status}.`,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAdminAdjutantEnrichmentAction({
          action: 'research-brief-review',
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

const launchChecklist = {
  audienceReviewed: true,
  buildReviewed: true,
  secretsReviewed: true,
  sourceReviewed: true,
  urlReviewed: true,
}

export const DeployAdminSiteVersion = Command.define(
  'DeployAdminSiteVersion',
  {
    assignmentId: S.String,
    publicLaunchChecklist: S.Boolean,
    siteId: S.String,
    versionId: S.String,
  },
  SucceededAdminSiteDeploymentAction,
  FailedAdminSiteDeploymentAction,
)(({ assignmentId, publicLaunchChecklist, siteId, versionId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({
          ...(publicLaunchChecklist ? { launchChecklist } : {}),
        }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.admin.site.version.deploy',
      request: `/api/operator/sites/${encodeURIComponent(siteId)}/versions/${encodeURIComponent(versionId)}/deploy`,
      schema: AdminSiteDeploymentActionResponse,
    })

    return SucceededAdminSiteDeploymentAction({
      action: 'deploy',
      assignmentId,
      message: `Deployment ${response.deployment.id} activated.`,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAdminSiteDeploymentAction({
          action: 'deploy',
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const RunAdminSiteDeploymentAction = Command.define(
  'RunAdminSiteDeploymentAction',
  {
    action: S.Literals(['disable', 'rollback']),
    assignmentId: S.String,
    deploymentId: S.String,
    siteId: S.String,
  },
  SucceededAdminSiteDeploymentAction,
  FailedAdminSiteDeploymentAction,
)(({ action, assignmentId, deploymentId, siteId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ confirm: true }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: `loggedIn.admin.site.deployment.${action}`,
      request: `/api/operator/sites/${encodeURIComponent(siteId)}/deployments/${encodeURIComponent(deploymentId)}/${action}`,
      schema: AdminSiteDeploymentActionResponse,
    })

    return SucceededAdminSiteDeploymentAction({
      action,
      assignmentId,
      message: `Deployment ${response.deployment.id} ${action === 'disable' ? 'disabled' : 'rolled back'}.`,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedAdminSiteDeploymentAction({
          action,
          assignmentId,
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const updateAdmin = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadAdminOverview: () => [
        evo(model, {
          adminAdjutantAssignments: () => AdminAdjutantAssignmentsLoading(),
          adminOverview: () => AdminOverviewLoading(),
        }),
        [LoadAdminOverview(), LoadAdminAdjutantAssignments()],
        Option.none(),
      ],
      SucceededLoadAdminOverview: ({ response }) => [
        evo(model, {
          adminOverview: () =>
            AdminOverviewLoaded({
              softwareOrders: response.softwareOrders,
              users: response.users,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAdminOverview: ({ error }) => [
        evo(model, { adminOverview: () => AdminOverviewFailed({ error }) }),
        [],
        Option.none(),
      ],
      RequestedGenerateAdminSite: ({ siteId }) => [
        model,
        [GenerateAdminSite({ siteId })],
        Option.none(),
      ],
      SucceededGenerateAdminSite: () => [
        model,
        [LoadAdminOverview()],
        Option.none(),
      ],
      FailedGenerateAdminSite: () => [model, [], Option.none()],
      RequestedLoadAdminAdjutantAssignments: () => [
        evo(model, {
          adminAdjutantAssignments: () => AdminAdjutantAssignmentsLoading(),
        }),
        [LoadAdminAdjutantAssignments()],
        Option.none(),
      ],
      SucceededLoadAdminAdjutantAssignments: ({ response }) => [
        evo(model, {
          adminAdjutantAssignments: () =>
            AdminAdjutantAssignmentsLoaded({
              assignments: response.assignments,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAdminAdjutantAssignments: ({ error }) => [
        evo(model, {
          adminAdjutantAssignments: () =>
            AdminAdjutantAssignmentsFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedLoadAdminAdjutantReview: ({ assignmentId }) => [
        evo(model, {
          adminAdjutantReview: () =>
            AdminAdjutantReviewLoading({ assignmentId }),
        }),
        [LoadAdminAdjutantReview({ assignmentId })],
        Option.none(),
      ],
      SucceededLoadAdminAdjutantReview: ({ response }) => [
        evo(model, {
          adminAdjutantReview: () =>
            AdminAdjutantReviewLoaded({
              assignment: response.assignment,
              review: response.review,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadAdminAdjutantReview: ({ assignmentId, error }) => [
        evo(model, {
          adminAdjutantReview: () =>
            AdminAdjutantReviewFailed({ assignmentId, error }),
        }),
        [],
        Option.none(),
      ],
      RequestedRunAdminAdjutantEnrichment: ({ assignmentId, refresh }) => [
        evo(model, {
          adminAdjutantEnrichmentAction: () =>
            AdminAdjutantEnrichmentActionPending({
              action: refresh === true ? 'refresh' : 'run',
              assignmentId,
            }),
        }),
        [
          RunAdminAdjutantEnrichment({
            assignmentId,
            ...(refresh === undefined ? {} : { refresh }),
          }),
        ],
        Option.none(),
      ],
      RequestedReviewAdminAdjutantSourceCard: ({
        assignmentId,
        reviewStatus,
        sourceId,
      }) => [
        evo(model, {
          adminAdjutantEnrichmentAction: () =>
            AdminAdjutantEnrichmentActionPending({
              action: 'source-card-review',
              assignmentId,
            }),
        }),
        [
          ReviewAdminAdjutantSourceCard({
            assignmentId,
            reviewStatus,
            sourceId,
          }),
        ],
        Option.none(),
      ],
      RequestedReviewAdminAdjutantResearchBrief: ({
        assignmentId,
        briefId,
        status,
      }) => [
        evo(model, {
          adminAdjutantEnrichmentAction: () =>
            AdminAdjutantEnrichmentActionPending({
              action: 'research-brief-review',
              assignmentId,
            }),
        }),
        [
          ReviewAdminAdjutantResearchBrief({
            assignmentId,
            briefId,
            status,
          }),
        ],
        Option.none(),
      ],
      SucceededAdminAdjutantEnrichmentAction: ({ assignmentId, message }) => [
        evo(model, {
          adminAdjutantEnrichmentAction: () =>
            AdminAdjutantEnrichmentActionSucceeded({ message }),
        }),
        [
          LoadAdminOverview(),
          LoadAdminAdjutantAssignments(),
          LoadAdminAdjutantReview({ assignmentId }),
        ],
        Option.none(),
      ],
      FailedAdminAdjutantEnrichmentAction: ({ error }) => [
        evo(model, {
          adminAdjutantEnrichmentAction: () =>
            AdminAdjutantEnrichmentActionFailed({ error }),
        }),
        [],
        Option.none(),
      ],
      RequestedDeployAdminSiteVersion: ({
        assignmentId,
        publicLaunchChecklist,
        siteId,
        versionId,
      }) => [
        evo(model, {
          adminSiteDeploymentAction: () =>
            AdminSiteDeploymentActionPending({
              action: 'deploy',
              assignmentId,
            }),
        }),
        [
          DeployAdminSiteVersion({
            assignmentId,
            publicLaunchChecklist,
            siteId,
            versionId,
          }),
        ],
        Option.none(),
      ],
      RequestedAdminSiteDeploymentAction: ({
        action,
        assignmentId,
        deploymentId,
        siteId,
      }) => [
        evo(model, {
          adminSiteDeploymentAction: () =>
            AdminSiteDeploymentActionPending({ action, assignmentId }),
        }),
        [
          RunAdminSiteDeploymentAction({
            action,
            assignmentId,
            deploymentId,
            siteId,
          }),
        ],
        Option.none(),
      ],
      SucceededAdminSiteDeploymentAction: ({ assignmentId, message }) => [
        evo(model, {
          adminSiteDeploymentAction: () =>
            AdminSiteDeploymentActionSucceeded({ message }),
        }),
        [
          LoadAdminOverview(),
          LoadAdminAdjutantAssignments(),
          LoadAdminAdjutantReview({ assignmentId }),
        ],
        Option.none(),
      ],
      FailedAdminSiteDeploymentAction: ({ error }) => [
        evo(model, {
          adminSiteDeploymentAction: () =>
            AdminSiteDeploymentActionFailed({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => [model, [], Option.none()]),
  )
