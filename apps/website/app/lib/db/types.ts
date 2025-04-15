// Database interface types for Kysely
export interface Database {
  // Authentication tables
  user: {
    id: string;
    name: string;
    email: string;
    emailVerified: number;
    image: string | null;
    createdAt: string;
    updatedAt: string;
  };
  session: {
    id: string;
    expiresAt: string;
    token: string;
    createdAt: string;
    updatedAt: string;
    ipAddress: string | null;
    userAgent: string | null;
    userId: string;
  };
  account: {
    id: string;
    accountId: string;
    providerId: string;
    userId: string;
    accessToken: string | null;
    refreshToken: string | null;
    idToken: string | null;
    accessTokenExpiresAt: string | null;
    refreshTokenExpiresAt: string | null;
    scope: string | null;
    password: string | null;
    createdAt: string;
    updatedAt: string;
  };
  verification: {
    id: string;
    identifier: string;
    value: string;
    expiresAt: string;
    createdAt: string | null;
    updatedAt: string | null;
  };

  // Project management tables
  project_status: {
    id: string;
    name: string;
    description: string | null;
    color: string;
    type: string;
    position: number;
    indefinite: number;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  team: {
    id: string;
    name: string;
    key: string;
    description: string | null;
    icon: string | null;
    color: string | null;
    private: number;
    timezone: string;
    inviteHash: string;
    cyclesEnabled: number;
    cycleDuration: number | null;
    cycleCooldownTime: number | null;
    cycleStartDay: number | null;
    cycleLockToActive: number | null;
    cycleIssueAutoAssignStarted: number | null;
    cycleIssueAutoAssignCompleted: number | null;
    upcomingCycleCount: number;
    autoArchivePeriod: number;
    autoClosePeriod: number | null;
    autoCloseStateId: string | null;
    autoCloseChildIssues: number | null;
    autoCloseParentIssues: number | null;
    issueEstimationType: string;
    issueEstimationAllowZero: number;
    issueEstimationExtended: number;
    defaultIssueEstimate: number;
    defaultIssueStateId: string | null;
    triageEnabled: number;
    triageIssueStateId: string | null;
    requirePriorityToLeaveTriage: number;
    groupIssueHistory: number;
    setIssueSortOrderOnStateChange: string;
    markedAsDuplicateWorkflowStateId: string | null;
    parentId: string | null;
    inheritIssueEstimation: number;
    inheritWorkflowStatuses: number;
    scimGroupName: string | null;
    scimManaged: number;
    joinByDefault: number | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  workflow_state: {
    id: string;
    name: string;
    description: string | null;
    color: string;
    type: string;
    position: number;
    teamId: string;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  issue_label: {
    id: string;
    name: string;
    description: string | null;
    color: string;
    isGroup: number;
    teamId: string | null;
    parentId: string | null;
    creatorId: string | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  project: {
    id: string;
    name: string;
    description: string;
    icon: string | null;
    color: string;
    slugId: string;
    sortOrder: number;
    priority: number;
    prioritySortOrder: number;
    health: string | null;
    healthUpdatedAt: string | null;
    progress: number;
    scope: number;
    startDate: string | null;
    startDateResolution: string | null;
    targetDate: string | null;
    targetDateResolution: string | null;
    trashed: number | null;
    content: string | null;
    updateReminderFrequency: number | null;
    updateReminderFrequencyInWeeks: number | null;
    updateRemindersDay: string | null;
    updateRemindersHour: number | null;
    projectUpdateRemindersPausedUntilAt: string | null;
    creatorId: string | null;
    leadId: string | null;
    statusId: string;
    convertedFromIssueId: string | null;
    archivedAt: string | null;
    canceledAt: string | null;
    completedAt: string | null;
    startedAt: string | null;
    autoArchivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  cycle: {
    id: string;
    number: number;
    name: string | null;
    description: string | null;
    startsAt: string;
    endsAt: string;
    teamId: string;
    completedAt: string | null;
    autoArchivedAt: string | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  project_milestone: {
    id: string;
    name: string;
    description: string | null;
    targetDate: string | null;
    sortOrder: number;
    projectId: string;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  project_update: {
    id: string;
    body: string;
    health: string;
    isDiffHidden: number;
    isStale: number;
    slugId: string;
    url: string;
    projectId: string;
    userId: string;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
    editedAt: string | null;
  };
  
  issue: {
    id: string;
    title: string;
    description: string | null;
    priority: number;
    prioritySortOrder: number;
    identifier: string;
    number: number;
    branchName: string;
    url: string;
    estimate: number | null;
    sortOrder: number;
    subIssueSortOrder: number | null;
    integrationSourceType: string | null;
    customerTicketCount: number;
    slaStartedAt: string | null;
    slaBreachesAt: string | null;
    slaHighRiskAt: string | null;
    slaMediumRiskAt: string | null;
    slaType: string | null;
    trashed: number | null;
    creatorId: string | null;
    assigneeId: string | null;
    snoozedById: string | null;
    parentId: string | null;
    teamId: string;
    projectId: string | null;
    cycleId: string | null;
    stateId: string;
    projectMilestoneId: string | null;
    sourceCommentId: string | null;
    createdAt: string;
    updatedAt: string;
    archivedAt: string | null;
    canceledAt: string | null;
    completedAt: string | null;
    startedAt: string | null;
    snoozedUntilAt: string | null;
    dueDate: string | null;
    autoArchivedAt: string | null;
    autoClosedAt: string | null;
    addedToCycleAt: string | null;
    addedToProjectAt: string | null;
    addedToTeamAt: string | null;
    startedTriageAt: string | null;
    triagedAt: string | null;
  };
  
  comment: {
    id: string;
    body: string;
    url: string;
    quotedText: string | null;
    issueId: string | null;
    projectUpdateId: string | null;
    userId: string | null;
    parentId: string | null;
    resolvingUserId: string | null;
    resolvingCommentId: string | null;
    createdAt: string;
    updatedAt: string;
    archivedAt: string | null;
    editedAt: string | null;
    resolvedAt: string | null;
  };
  
  attachment: {
    id: string;
    url: string;
    title: string;
    subtitle: string | null;
    metadata: string;
    sourceType: string | null;
    groupBySource: number;
    issueId: string;
    creatorId: string | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  reaction: {
    id: string;
    emoji: string;
    userId: string | null;
    commentId: string | null;
    issueId: string | null;
    projectUpdateId: string | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  issue_relation: {
    id: string;
    type: string;
    issueId: string;
    relatedIssueId: string;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  project_relation: {
    id: string;
    type: string;
    projectId: string;
    relatedProjectId: string;
    projectMilestoneId: string | null;
    relatedProjectMilestoneId: string | null;
    anchorType: string;
    relatedAnchorType: string;
    userId: string | null;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  // Many-to-many relationship tables
  team_membership: {
    id: string;
    teamId: string;
    userId: string;
    owner: number;
    sortOrder: number;
    archivedAt: string | null;
    createdAt: string;
    updatedAt: string;
  };
  
  project_member: {
    id: string;
    projectId: string;
    userId: string;
    createdAt: string;
    updatedAt: string;
  };
  
  team_project: {
    id: string;
    teamId: string;
    projectId: string;
    createdAt: string;
    updatedAt: string;
  };
  
  issue_to_label: {
    id: string;
    issueId: string;
    labelId: string;
    createdAt: string;
    updatedAt: string;
  };
  
  issue_subscriber: {
    id: string;
    issueId: string;
    subscriberId: string;
    createdAt: string;
    updatedAt: string;
  };
}