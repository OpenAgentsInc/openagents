-- projects.sql
-- Enable foreign key constraints
PRAGMA foreign_keys = OFF; -- Temporarily disable for table creation/ordering flexibility

-- Tables without dependencies within this file (or only external 'user')

-- Represents the status of a project (e.g., Backlog, Planned, Started, Paused, Completed, Canceled).
create table project_status (
    id TEXT not null primary key,
    name TEXT not null,                                -- The name of the status.
    description TEXT,                                  -- Description of the status.
    color TEXT not null,                               -- The UI color of the status as a HEX string.
    type TEXT not null,                                -- The type of the project status (e.g., 'backlog', 'planned', 'started', 'paused', 'completed', 'canceled').
    position REAL not null,                            -- The position of the status in the workspace's project flow.
    indefinite INTEGER not null default 0,             -- Whether or not a project can be in this status indefinitely (0=false, 1=true).
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents a team, an organizational unit containing issues and users.
-- Note: FKs to workflow_state are nullable, allowing team creation before states if necessary,
-- but workflow_state itself needs teamId, so team is created first.
create table team (
    id TEXT not null primary key,
    name TEXT not null,                                -- The team's name.
    key TEXT not null unique,                          -- The team's unique key, used in URLs and issue identifiers.
    description TEXT,                                  -- The team's description.
    icon TEXT,                                         -- The icon of the team.
    color TEXT,                                        -- The team's color.
    private INTEGER not null default 0,                -- Whether the team is private or not (0=false, 1=true).
    timezone TEXT not null default 'America/Los_Angeles', -- The timezone of the team.
    inviteHash TEXT not null,                          -- Unique hash for the team to be used in invite URLs.
    cyclesEnabled INTEGER not null default 0,          -- Whether the team uses cycles (0=false, 1=true).
    cycleDuration INTEGER,                             -- The duration of a cycle in weeks.
    cycleCooldownTime INTEGER,                         -- The cooldown time after each cycle in weeks.
    cycleStartDay INTEGER,                             -- The day of the week that a new cycle starts (0 = Sunday, 1 = Monday...).
    cycleLockToActive INTEGER,                         -- Auto assign issues to current cycle if in active status (0=false, 1=true).
    cycleIssueAutoAssignStarted INTEGER,               -- Auto assign started issues to current cycle (0=false, 1=true).
    cycleIssueAutoAssignCompleted INTEGER,             -- Auto assign completed issues to current cycle (0=false, 1=true).
    upcomingCycleCount REAL not null default 0,        -- How many upcoming cycles to create.
    autoArchivePeriod INTEGER not null default 0,      -- Period after which automatically closed and completed issues are automatically archived in months. 0 means disabled.
    autoClosePeriod INTEGER,                           -- Period after which issues are automatically closed in months. Null means disabled.
    autoCloseStateId TEXT,                             -- FK added below if deferred FKs aren't used, otherwise references workflow_state(id) ON DELETE SET NULL
    autoCloseChildIssues INTEGER,                      -- Whether child issues should automatically close when their parent issue is closed (0=false, 1=true).
    autoCloseParentIssues INTEGER,                     -- Whether parent issues should automatically close when all child issues are closed (0=false, 1=true).
    issueEstimationType TEXT not null default 'notUsed', -- The issue estimation type to use (e.g., 'notUsed', 'exponential', 'fibonacci', 'linear', 'tShirt').
    issueEstimationAllowZero INTEGER not null default 0, -- Whether to allow zeros in issues estimates (0=false, 1=true).
    issueEstimationExtended INTEGER not null default 0, -- Whether to add additional points to the estimate scale (0=false, 1=true).
    defaultIssueEstimate INTEGER not null default 0,   -- What to use as a default estimate for unestimated issues.
    defaultIssueStateId TEXT,                          -- FK added below if deferred FKs aren't used, otherwise references workflow_state(id) ON DELETE SET NULL
    triageEnabled INTEGER not null default 0,          -- Whether triage mode is enabled for the team or not (0=false, 1=true).
    triageIssueStateId TEXT,                           -- FK added below if deferred FKs aren't used, otherwise references workflow_state(id) ON DELETE SET NULL
    requirePriorityToLeaveTriage INTEGER not null default 0, -- Whether an issue needs to have a priority set before leaving triage (0=false, 1=true).
    groupIssueHistory INTEGER not null default 1,      -- Whether to group recent issue history entries (0=false, 1=true).
    setIssueSortOrderOnStateChange TEXT not null default 'bottom', -- Where to move issues when changing state ('bottom', 'top', 'keep').
    markedAsDuplicateWorkflowStateId TEXT,             -- FK added below if deferred FKs aren't used, otherwise references workflow_state(id) ON DELETE SET NULL
    parentId TEXT references team (id) on delete set null, -- The team's parent team.
    inheritIssueEstimation INTEGER not null default 1, -- Whether the team should inherit its estimation settings from its parent (0=false, 1=true). Only applies to sub-teams.
    inheritWorkflowStatuses INTEGER not null default 1,-- Whether the team should inherit its workflow statuses from its parent (0=false, 1=true). Only applies to sub-teams.
    scimGroupName TEXT,                                -- The SCIM group name for the team.
    scimManaged INTEGER not null default 0,            -- Whether the team is managed by SCIM integration (0=false, 1=true).
    joinByDefault INTEGER,                             -- Whether new users should join this team by default (0=false, 1=true).
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    -- Declaring FKs here for SQLite compatibility (even if table doesn't exist yet, enabled later)
    foreign key(autoCloseStateId) references workflow_state(id) on delete set null,
    foreign key(defaultIssueStateId) references workflow_state(id) on delete set null,
    foreign key(triageIssueStateId) references workflow_state(id) on delete set null,
    foreign key(markedAsDuplicateWorkflowStateId) references workflow_state(id) on delete set null
);

-- Represents a state in a team's issue workflow.
create table workflow_state (
    id TEXT not null primary key,
    name TEXT not null,                                -- The state's name.
    description TEXT,                                  -- Description of the state.
    color TEXT not null,                               -- The state's UI color as a HEX string.
    type TEXT not null,                                -- The type of the state (e.g., 'triage', 'backlog', 'unstarted', 'started', 'completed', 'canceled').
    position REAL not null,                            -- The position of the state in the team flow.
    teamId TEXT not null references team (id) on delete cascade, -- The team to which this state belongs to.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents labels that can be associated with issues.
create table issue_label (
    id TEXT not null primary key,
    name TEXT not null,                                -- The label's name.
    description TEXT,                                  -- The label's description.
    color TEXT not null,                               -- The label's color as a HEX string.
    isGroup INTEGER not null default 0,                -- Whether this label is considered to be a group (0=false, 1=true).
    teamId TEXT references team (id) on delete cascade, -- The team that the label is associated with. If null, the label is global/workspace level.
    parentId TEXT references issue_label (id) on delete cascade, -- The parent label.
    creatorId TEXT references user (id) on delete set null, -- The user who created the label.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents a project, a collection of issues with a specific goal and timeframe.
-- Note: convertedFromIssueId references 'issue', defined later. Allowed by SQLite if FKs enabled after creation.
create table project (
    id TEXT not null primary key,
    name TEXT not null,                                -- The project's name.
    description TEXT not null,                         -- The project's description.
    icon TEXT,                                         -- The icon of the project.
    color TEXT not null,                               -- The project's color.
    slugId TEXT not null unique,                       -- The project's unique URL slug.
    sortOrder REAL not null,                           -- The sort order for the project within the organization.
    priority INTEGER not null default 0,               -- The priority of the project (0=NP, 1=U, 2=H, 3=M, 4=L).
    prioritySortOrder REAL not null,                   -- The sort order for the project within the organization, when ordered by priority.
    health TEXT,                                       -- The health of the project ('onTrack', 'atRisk', 'offTrack').
    healthUpdatedAt TEXT,                              -- The time at which the project health was updated (ISO8601 format).
    progress REAL not null default 0,                  -- The overall progress of the project (0-1).
    scope REAL not null default 0,                     -- The overall scope (total estimate points) of the project.
    startDate TEXT,                                    -- The estimated start date of the project (YYYY-MM-DD).
    startDateResolution TEXT,                          -- The resolution of the project's start date ('year', 'quarter', 'month', 'halfYear').
    targetDate TEXT,                                   -- The estimated completion date of the project (YYYY-MM-DD).
    targetDateResolution TEXT,                         -- The resolution of the project's estimated completion date.
    trashed INTEGER default 0,                         -- A flag that indicates whether the project is in the trash bin (0=false, 1=true).
    content TEXT,                                      -- The project's content in markdown format.
    updateReminderFrequency REAL,                      -- The frequency at which to prompt for updates.
    updateReminderFrequencyInWeeks REAL,               -- The n-weekly frequency at which to prompt for updates.
    updateRemindersDay TEXT,                           -- The day at which to prompt for updates ('Monday', 'Tuesday'...).
    updateRemindersHour REAL,                          -- The hour at which to prompt for updates.
    projectUpdateRemindersPausedUntilAt TEXT,          -- The time until which project update reminders are paused (ISO8601 format).
    creatorId TEXT references user (id) on delete set null,  -- The user who created the project.
    leadId TEXT references user (id) on delete set null,     -- The project lead.
    statusId TEXT not null references project_status (id), -- The status that the project is associated with.
    convertedFromIssueId TEXT references issue (id) on delete set null, -- The issue this project was created from.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    canceledAt TEXT,                                   -- The time at which the project was moved into canceled state (ISO8601 format).
    completedAt TEXT,                                  -- The time at which the project was moved into completed state (ISO8601 format).
    startedAt TEXT,                                    -- The time at which the project was moved into started state (ISO8601 format).
    autoArchivedAt TEXT,                               -- The time at which the project was automatically archived by the auto pruning process (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents a cycle, a time-boxed period for completing issues within a team.
create table cycle (
    id TEXT not null primary key,
    number INTEGER not null,                           -- The number of the cycle.
    name TEXT,                                         -- The custom name of the cycle.
    description TEXT,                                  -- The cycle's description.
    startsAt TEXT not null,                            -- The start time of the cycle (ISO8601 format).
    endsAt TEXT not null,                              -- The end time of the cycle (ISO8601 format).
    teamId TEXT not null references team (id) on delete cascade, -- The team that the cycle is associated with.
    completedAt TEXT,                                  -- The completion time of the cycle. If null, the cycle hasn't been completed (ISO8601 format).
    autoArchivedAt TEXT,                               -- The time at which the cycle was automatically archived by the auto pruning process (ISO8601 format).
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents a project milestone, a significant point or goal within a project.
create table project_milestone (
    id TEXT not null primary key,
    name TEXT not null,                                -- The name of the project milestone.
    description TEXT,                                  -- The project milestone's description in markdown format.
    targetDate TEXT,                                   -- The planned completion date of the milestone (YYYY-MM-DD).
    sortOrder REAL not null,                           -- The order of the milestone in relation to other milestones within a project.
    projectId TEXT not null references project (id) on delete cascade, -- The project of the milestone.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents a project update post.
create table project_update (
    id TEXT not null primary key,
    body TEXT not null,                                -- The update content in markdown format.
    health TEXT not null,                              -- The health of the project at the time of the update ('onTrack', 'atRisk', 'offTrack').
    isDiffHidden INTEGER not null default 0,           -- Whether project update diff should be hidden (0=false, 1=true).
    isStale INTEGER not null default 0,                -- Whether the project update is stale (0=false, 1=true).
    slugId TEXT not null unique,                       -- The update's unique URL slug.
    url TEXT not null,                                 -- The URL to the project update.
    projectId TEXT not null references project (id) on delete cascade, -- The project that the update is associated with.
    userId TEXT not null references user (id) on delete cascade, -- The user who wrote the update.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    editedAt TEXT                                      -- The time the update was edited (ISO8601 format).
);

-- Represents an issue, the core unit of work.
-- Needs comment table first for sourceCommentId FK.
create table issue (
    id TEXT not null primary key,
    title TEXT not null,                               -- The issue's title.
    description TEXT,                                  -- The issue's description in markdown format.
    priority INTEGER not null default 0,               -- The priority of the issue (0=NP, 1=U, 2=H, 3=M, 4=L).
    prioritySortOrder REAL not null,                   -- The order of the item in relation to other items in the organization, when ordered by priority.
    identifier TEXT not null,                          -- Issue's human readable identifier (e.g., ENG-123).
    number INTEGER not null,                           -- The issue's unique number within the team.
    branchName TEXT not null,                          -- Suggested branch name for the issue.
    url TEXT not null,                                 -- Issue URL.
    estimate INTEGER,                                  -- The estimate of the complexity of the issue.
    sortOrder REAL not null,                           -- The order of the item in relation to other items in the organization.
    subIssueSortOrder REAL,                            -- The order of the item in the sub-issue list. Only set if the issue has a parent.
    integrationSourceType TEXT,                        -- Integration type that created this issue, if applicable (e.g., 'github', 'slack').
    customerTicketCount INTEGER not null default 0,    -- Count of attachments created by customer support ticketing systems.
    slaStartedAt TEXT,                                 -- The time at which the issue's SLA began (ISO8601 format).
    slaBreachesAt TEXT,                                -- The time at which the issue's SLA will breach (ISO8601 format).
    slaHighRiskAt TEXT,                                -- The time at which the issue's SLA will enter high risk state (ISO8601 format).
    slaMediumRiskAt TEXT,                              -- The time at which the issue's SLA will enter medium risk state (ISO8601 format).
    slaType TEXT,                                      -- The type of SLA set on the issue ('all' or 'onlyBusinessDays').
    trashed INTEGER default 0,                         -- A flag that indicates whether the issue is in the trash bin (0=false, 1=true).
    creatorId TEXT references user (id) on delete set null, -- The user who created the issue.
    assigneeId TEXT references user (id) on delete set null, -- The user to whom the issue is assigned to.
    snoozedById TEXT references user (id) on delete set null, -- The user who snoozed the issue.
    parentId TEXT references issue (id) on delete cascade, -- The parent of the issue.
    teamId TEXT not null references team (id) on delete cascade, -- The team that the issue is associated with.
    projectId TEXT references project (id) on delete set null, -- The project that the issue is associated with.
    cycleId TEXT references cycle (id) on delete set null, -- The cycle that the issue is associated with.
    stateId TEXT not null references workflow_state (id), -- The workflow state that the issue is associated with.
    projectMilestoneId TEXT references project_milestone (id) on delete set null, -- The projectMilestone that the issue is associated with.
    sourceCommentId TEXT references comment (id) on delete set null, -- The comment that this issue was created from.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    canceledAt TEXT,                                   -- The time at which the issue was moved into canceled state (ISO8601 format).
    completedAt TEXT,                                  -- The time at which the issue was moved into completed state (ISO8601 format).
    startedAt TEXT,                                    -- The time at which the issue was moved into started state (ISO8601 format).
    snoozedUntilAt TEXT,                               -- The time until an issue will be snoozed in Triage view (ISO8601 format).
    dueDate TEXT,                                      -- The date at which the issue is due (YYYY-MM-DD).
    autoArchivedAt TEXT,                               -- The time at which the issue was automatically archived by the auto pruning process (ISO8601 format).
    autoClosedAt TEXT,                                 -- The time at which the issue was automatically closed by the auto pruning process (ISO8601 format).
    addedToCycleAt TEXT,                               -- The time at which the issue was added to a cycle (ISO8601 format).
    addedToProjectAt TEXT,                             -- The time at which the issue was added to a project (ISO8601 format).
    addedToTeamAt TEXT,                                -- The time at which the issue was added to a team (ISO8601 format).
    startedTriageAt TEXT,                              -- The time at which the issue entered triage (ISO8601 format).
    triagedAt TEXT                                     -- The time at which the issue left triage (ISO8601 format).
);

-- Represents a comment associated with an issue, project update, etc.
create table comment (
    id TEXT not null primary key,
    body TEXT not null,                                -- The comment content in markdown format.
    url TEXT not null,                                 -- Comment's URL.
    quotedText TEXT,                                   -- The text that this comment references. Only defined for inline comments.
    issueId TEXT references issue (id) on delete cascade, -- The issue that the comment is associated with.
    projectUpdateId TEXT references project_update (id) on delete cascade, -- The project update that the comment is associated with.
    userId TEXT references user (id) on delete set null, -- The user who wrote the comment.
    parentId TEXT references comment (id) on delete cascade, -- The parent comment under which the current comment is nested.
    resolvingUserId TEXT references user (id) on delete set null, -- The user that resolved the thread.
    resolvingCommentId TEXT references comment (id) on delete set null, -- The comment that resolved the thread.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    editedAt TEXT,                                     -- The time user edited the comment (ISO8601 format).
    resolvedAt TEXT,                                   -- The time the resolvingUser resolved the thread (ISO8601 format).
    -- Check constraint to ensure it's linked to something (adjust as needed if more targets added)
    constraint chk_comment_target check (issueId is not null or projectUpdateId is not null)
);

-- Represents attachments linked to issues (e.g., files, URLs, external resources).
create table attachment (
    id TEXT not null primary key,
    url TEXT not null,                                 -- Location of the attachment which is also used as an identifier.
    title TEXT not null,                               -- Content for the title line in the Linear attachment widget.
    subtitle TEXT,                                     -- Content for the subtitle line in the Linear attachment widget.
    metadata TEXT not null default '{}',               -- Custom metadata related to the attachment (as JSON string).
    sourceType TEXT,                                   -- An accessor helper to source.type, defines the source type of the attachment.
    groupBySource INTEGER not null default 0,          -- Indicates if attachments for the same source application should be grouped (0=false, 1=true).
    issueId TEXT not null references issue (id) on delete cascade, -- The issue this attachment belongs to.
    creatorId TEXT references user (id) on delete set null, -- The creator of the attachment.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))  -- The last time the entity was meaningfully updated (ISO8601 format).
);

-- Represents an emoji reaction on an issue, comment, or project update.
create table reaction (
    id TEXT not null primary key,
    emoji TEXT not null,                               -- Name of the reaction's emoji.
    userId TEXT references user (id) on delete cascade, -- The user that created the reaction.
    commentId TEXT references comment (id) on delete cascade, -- The comment that the reaction is associated with.
    issueId TEXT references issue (id) on delete cascade, -- The issue that the reaction is associated with.
    projectUpdateId TEXT references project_update (id) on delete cascade, -- The project update that the reaction is associated with.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    -- Check constraint to ensure at least one target is not null
    constraint chk_reaction_target check (
        commentId is not null or
        issueId is not null or
        projectUpdateId is not null
    )
);

-- Represents a relationship between two issues (e.g., blocks, duplicate, related).
create table issue_relation (
    id TEXT not null primary key,
    type TEXT not null,                                -- The relationship type ('blocks', 'duplicate', 'related', 'similar').
    issueId TEXT not null references issue (id) on delete cascade, -- The issue whose relationship is being described.
    relatedIssueId TEXT not null references issue (id) on delete cascade, -- The related issue.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique (issueId, relatedIssueId, type)
);

-- Represents a relationship between two projects.
create table project_relation (
    id TEXT not null primary key,
    type TEXT not null,                                -- The relationship type (e.g., 'blocks', 'related').
    projectId TEXT not null references project (id) on delete cascade, -- The project whose relationship is being described.
    relatedProjectId TEXT not null references project (id) on delete cascade, -- The related project.
    projectMilestoneId TEXT references project_milestone (id) on delete cascade, -- Optional milestone anchor for the source project.
    relatedProjectMilestoneId TEXT references project_milestone (id) on delete cascade, -- Optional milestone anchor for the related project.
    anchorType TEXT not null,                          -- The type of anchor on the project end of the relation.
    relatedAnchorType TEXT not null,                   -- The type of anchor on the relatedProject end of the relation.
    userId TEXT references user (id) on delete set null, -- The last user who created or modified the relation.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique (projectId, relatedProjectId, type, projectMilestoneId, relatedProjectMilestoneId)
);

-- Join Tables (Many-to-Many Relationships)

-- Users belonging to teams.
create table team_membership (
    id TEXT not null primary key,
    teamId TEXT not null references team (id) on delete cascade, -- The team associated with the membership.
    userId TEXT not null references user (id) on delete cascade, -- The user associated with the membership.
    owner INTEGER not null default 0,                  -- Whether the user is the owner of the team (0=false, 1=true).
    sortOrder REAL not null default 0,                 -- The order of the item in the users team list.
    archivedAt TEXT,                                   -- The time at which the entity was archived (ISO8601 format).
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique(teamId, userId)
);

-- Users belonging to projects.
create table project_member (
    id TEXT not null primary key,
    projectId TEXT not null references project (id) on delete cascade, -- The project associated with the membership.
    userId TEXT not null references user (id) on delete cascade, -- The user associated with the membership.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique(projectId, userId)
);

-- Projects belonging to teams.
create table team_project (
    id TEXT not null primary key,
    teamId TEXT not null references team (id) on delete cascade, -- The team associated with the project.
    projectId TEXT not null references project (id) on delete cascade, -- The project associated with the team.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique(teamId, projectId)
);

-- Labels applied to issues.
create table issue_to_label (
    id TEXT not null primary key,
    issueId TEXT not null references issue (id) on delete cascade, -- The issue the label is applied to.
    labelId TEXT not null references issue_label (id) on delete cascade, -- The label applied to the issue.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique(issueId, labelId)
);

-- Users subscribed to issues.
create table issue_subscriber (
    id TEXT not null primary key,
    issueId TEXT not null references issue (id) on delete cascade, -- The issue the user is subscribed to.
    subscriberId TEXT not null references user (id) on delete cascade, -- The user subscribed to the issue.
    createdAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The time at which the entity was created (ISO8601 format).
    updatedAt TEXT not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- The last time the entity was meaningfully updated (ISO8601 format).
    unique(issueId, subscriberId)
);

-- Indices for faster lookups
create index idx_workflow_state_team on workflow_state (teamId);
create index idx_project_status_type on project_status (type);

create index idx_team_parent on team (parentId);
create index idx_team_key on team (key);

create index idx_project_creator on project (creatorId);
create index idx_project_lead on project (leadId);
create index idx_project_status on project (statusId);
create index idx_project_slugId on project (slugId);
create index idx_project_convertedFromIssue on project (convertedFromIssueId);

create index idx_cycle_team on cycle (teamId);
create index idx_cycle_endsAt on cycle (endsAt);

create index idx_project_milestone_project on project_milestone (projectId);

create index idx_issue_creator on issue (creatorId);
create index idx_issue_assignee on issue (assigneeId);
create index idx_issue_parent on issue (parentId);
create index idx_issue_team on issue (teamId);
create index idx_issue_project on issue (projectId);
create index idx_issue_cycle on issue (cycleId);
create index idx_issue_state on issue (stateId);
create index idx_issue_milestone on issue (projectMilestoneId);
create index idx_issue_identifier_team on issue (identifier, teamId);
create index idx_issue_sourceComment on issue (sourceCommentId);
create index idx_issue_snoozedBy on issue (snoozedById);
create index idx_issue_createdAt on issue (createdAt);
create index idx_issue_updatedAt on issue (updatedAt);
create index idx_issue_dueDate on issue (dueDate);

create index idx_comment_issue on comment (issueId);
create index idx_comment_user on comment (userId);
create index idx_comment_parent on comment (parentId);
create index idx_comment_projectUpdate on comment (projectUpdateId);
create index idx_comment_resolvingUser on comment (resolvingUserId);

create index idx_project_update_project on project_update (projectId);
create index idx_project_update_user on project_update (userId);

create index idx_issue_label_team on issue_label (teamId);
create index idx_issue_label_parent on issue_label (parentId);
create index idx_issue_label_creator on issue_label (creatorId);

create index idx_attachment_issue on attachment (issueId);
create index idx_attachment_creator on attachment (creatorId);
create index idx_attachment_url on attachment (url);

create index idx_reaction_user on reaction (userId);
create index idx_reaction_comment on reaction (commentId);
create index idx_reaction_issue on reaction (issueId);
create index idx_reaction_project_update on reaction (projectUpdateId);

create index idx_issue_relation_issue on issue_relation (issueId);
create index idx_issue_relation_related on issue_relation (relatedIssueId);

create index idx_project_relation_project on project_relation (projectId);
create index idx_project_relation_related on project_relation (relatedProjectId);
create index idx_project_relation_milestone on project_relation (projectMilestoneId);
create index idx_project_relation_related_milestone on project_relation (relatedProjectMilestoneId);
create index idx_project_relation_user on project_relation (userId);

create index idx_team_membership_team on team_membership (teamId);
create index idx_team_membership_user on team_membership (userId);

create index idx_project_member_project on project_member (projectId);
create index idx_project_member_user on project_member (userId);

create index idx_team_project_team on team_project (teamId);
create index idx_team_project_project on team_project (projectId);

create index idx_issue_to_label_issue on issue_to_label (issueId);
create index idx_issue_to_label_label on issue_to_label (labelId);

create index idx_issue_subscriber_issue on issue_subscriber (issueId);
create index idx_issue_subscriber_subscriber on issue_subscriber (subscriberId);

-- Re-enable foreign keys after all tables are defined
PRAGMA foreign_keys = ON;
