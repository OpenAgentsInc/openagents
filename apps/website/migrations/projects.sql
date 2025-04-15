-- projects.sql

-- Represents a state in a team's issue workflow.
create table "workflow_state" (
    "id" text not null primary key,
    "name" text not null,                                -- The state's name.
    "description" text,                                  -- Description of the state.
    "color" text not null,                               -- The state's UI color as a HEX string.
    "type" text not null,                                -- The type of the state (e.g., 'triage', 'backlog', 'unstarted', 'started', 'completed', 'canceled').
    "position" float not null,                           -- The position of the state in the team flow.
    "teamId" text not null references "team" ("id") on delete cascade, -- The team to which this state belongs to.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
);

-- Represents the status of a project (e.g., Backlog, Planned, Started, Paused, Completed, Canceled).
create table "project_status" (
    "id" text not null primary key,
    "name" text not null,                                -- The name of the status.
    "description" text,                                  -- Description of the status.
    "color" text not null,                               -- The UI color of the status as a HEX string.
    "type" text not null,                                -- The type of the project status (e.g., 'backlog', 'planned', 'started', 'paused', 'completed', 'canceled').
    "position" float not null,                           -- The position of the status in the workspace's project flow.
    "indefinite" boolean not null default false,         -- Whether or not a project can be in this status indefinitely.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
);

-- Represents a team, an organizational unit containing issues and users.
create table "team" (
    "id" text not null primary key,
    "name" text not null,                                -- The team's name.
    "key" text not null unique,                          -- The team's unique key, used in URLs and issue identifiers.
    "description" text,                                  -- The team's description.
    "icon" text,                                         -- The icon of the team.
    "color" text,                                        -- The team's color.
    "private" boolean not null default false,            -- Whether the team is private or not.
    "timezone" text not null default 'America/Los_Angeles', -- The timezone of the team.
    "inviteHash" text not null,                          -- Unique hash for the team to be used in invite URLs.
    "cyclesEnabled" boolean not null default false,      -- Whether the team uses cycles.
    "cycleDuration" integer,                             -- The duration of a cycle in weeks.
    "cycleCooldownTime" integer,                         -- The cooldown time after each cycle in weeks.
    "cycleStartDay" integer,                             -- The day of the week that a new cycle starts (0 = Sunday, 1 = Monday...).
    "cycleLockToActive" boolean,                         -- Auto assign issues to current cycle if in active status.
    "cycleIssueAutoAssignStarted" boolean,               -- Auto assign started issues to current cycle.
    "cycleIssueAutoAssignCompleted" boolean,             -- Auto assign completed issues to current cycle.
    "upcomingCycleCount" float not null default 0,       -- How many upcoming cycles to create.
    "autoArchivePeriod" integer not null default 0,      -- Period after which automatically closed and completed issues are automatically archived in months. 0 means disabled.
    "autoClosePeriod" integer,                           -- Period after which issues are automatically closed in months. Null means disabled.
    "autoCloseStateId" text references "workflow_state" ("id") on delete set null, -- The canceled workflow state which auto closed issues will be set to.
    "autoCloseChildIssues" boolean,                      -- Whether child issues should automatically close when their parent issue is closed.
    "autoCloseParentIssues" boolean,                     -- Whether parent issues should automatically close when all child issues are closed.
    "issueEstimationType" text not null default 'notUsed', -- The issue estimation type to use (e.g., 'notUsed', 'exponential', 'fibonacci', 'linear', 'tShirt').
    "issueEstimationAllowZero" boolean not null default false, -- Whether to allow zeros in issues estimates.
    "issueEstimationExtended" boolean not null default false, -- Whether to add additional points to the estimate scale.
    "defaultIssueEstimate" integer not null default 0,   -- What to use as a default estimate for unestimated issues.
    "defaultIssueStateId" text references "workflow_state" ("id") on delete set null, -- The default workflow state into which issues are set when they are opened by team members.
    "triageEnabled" boolean not null default false,      -- Whether triage mode is enabled for the team or not.
    "triageIssueStateId" text references "workflow_state" ("id") on delete set null, -- The workflow state into which issues are set when they are opened by non-team members or integrations if triage is enabled.
    "requirePriorityToLeaveTriage" boolean not null default false, -- Whether an issue needs to have a priority set before leaving triage.
    "groupIssueHistory" boolean not null default true,   -- Whether to group recent issue history entries.
    "setIssueSortOrderOnStateChange" text not null default 'bottom', -- Where to move issues when changing state ('bottom', 'top', 'keep').
    "markedAsDuplicateWorkflowStateId" text references "workflow_state" ("id") on delete set null, -- The workflow state into which issues are moved when they are marked as a duplicate of another issue.
    "parentId" text references "team" ("id") on delete set null, -- The team's parent team.
    "inheritIssueEstimation" boolean not null default true, -- Whether the team should inherit its estimation settings from its parent. Only applies to sub-teams.
    "inheritWorkflowStatuses" boolean not null default true, -- Whether the team should inherit its workflow statuses from its parent. Only applies to sub-teams.
    "scimGroupName" text,                                -- The SCIM group name for the team.
    "scimManaged" boolean not null default false,        -- Whether the team is managed by SCIM integration.
    "joinByDefault" boolean,                             -- Whether new users should join this team by default.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
    -- Cannot add defaultTemplateForMembersId/NonMembersId without Template table
);

-- Represents a project, a collection of issues with a specific goal and timeframe.
create table "project" (
    "id" text not null primary key,
    "name" text not null,                                -- The project's name.
    "description" text not null,                         -- The project's description.
    "icon" text,                                         -- The icon of the project.
    "color" text not null,                               -- The project's color.
    "slugId" text not null unique,                       -- The project's unique URL slug.
    "sortOrder" float not null,                          -- The sort order for the project within the organization.
    "priority" integer not null default 0,               -- The priority of the project (0=NP, 1=U, 2=H, 3=M, 4=L).
    "prioritySortOrder" float not null,                  -- The sort order for the project within the organization, when ordered by priority.
    "health" text,                                       -- The health of the project ('onTrack', 'atRisk', 'offTrack').
    "healthUpdatedAt" timestamp with time zone,          -- The time at which the project health was updated.
    "progress" float not null default 0,                 -- The overall progress of the project (0-1).
    "scope" float not null default 0,                    -- The overall scope (total estimate points) of the project.
    "startDate" date,                                    -- The estimated start date of the project.
    "startDateResolution" text,                          -- The resolution of the project's start date ('year', 'quarter', 'month', 'halfYear').
    "targetDate" date,                                   -- The estimated completion date of the project.
    "targetDateResolution" text,                         -- The resolution of the project's estimated completion date.
    "trashed" boolean default false,                     -- A flag that indicates whether the project is in the trash bin.
    "content" text,                                      -- The project's content in markdown format.
    "updateReminderFrequency" float,                     -- The frequency at which to prompt for updates.
    "updateReminderFrequencyInWeeks" float,              -- The n-weekly frequency at which to prompt for updates.
    "updateRemindersDay" text,                           -- The day at which to prompt for updates ('Monday', 'Tuesday'...).
    "updateRemindersHour" float,                         -- The hour at which to prompt for updates.
    "projectUpdateRemindersPausedUntilAt" timestamp with time zone, -- The time until which project update reminders are paused.
    "creatorId" text references "user" ("id") on delete set null,  -- The user who created the project.
    "leadId" text references "user" ("id") on delete set null,     -- The project lead.
    "statusId" text not null references "project_status" ("id"), -- The status that the project is associated with.
    "convertedFromIssueId" text references "issue" ("id") on delete set null, -- The issue this project was created from.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "canceledAt" timestamp with time zone,               -- The time at which the project was moved into canceled state.
    "completedAt" timestamp with time zone,              -- The time at which the project was moved into completed state.
    "startedAt" timestamp with time zone,                -- The time at which the project was moved into started state.
    "autoArchivedAt" timestamp with time zone,           -- The time at which the project was automatically archived by the auto pruning process.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
    -- Cannot add lastAppliedTemplateId without Template table
);

-- Represents a cycle, a time-boxed period for completing issues within a team.
create table "cycle" (
    "id" text not null primary key,
    "number" integer not null,                           -- The number of the cycle.
    "name" text,                                         -- The custom name of the cycle.
    "description" text,                                  -- The cycle's description.
    "startsAt" timestamp with time zone not null,        -- The start time of the cycle.
    "endsAt" timestamp with time zone not null,          -- The end time of the cycle.
    "teamId" text not null references "team" ("id") on delete cascade, -- The team that the cycle is associated with.
    "completedAt" timestamp with time zone,              -- The completion time of the cycle. If null, the cycle hasn't been completed.
    "autoArchivedAt" timestamp with time zone,           -- The time at which the cycle was automatically archived by the auto pruning process.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
);

-- Represents a project milestone, a significant point or goal within a project.
create table "project_milestone" (
    "id" text not null primary key,
    "name" text not null,                                -- The name of the project milestone.
    "description" text,                                  -- The project milestone's description in markdown format.
    "targetDate" date,                                   -- The planned completion date of the milestone.
    "sortOrder" float not null,                          -- The order of the milestone in relation to other milestones within a project.
    "projectId" text not null references "project" ("id") on delete cascade, -- The project of the milestone.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
);

-- Represents an issue, the core unit of work.
create table "issue" (
    "id" text not null primary key,
    "title" text not null,                               -- The issue's title.
    "description" text,                                  -- The issue's description in markdown format.
    "priority" integer not null default 0,               -- The priority of the issue (0=NP, 1=U, 2=H, 3=M, 4=L).
    "prioritySortOrder" float not null,                  -- The order of the item in relation to other items in the organization, when ordered by priority.
    "identifier" text not null,                          -- Issue's human readable identifier (e.g., ENG-123).
    "number" integer not null,                           -- The issue's unique number within the team.
    "branchName" text not null,                          -- Suggested branch name for the issue.
    "url" text not null,                                 -- Issue URL.
    "estimate" integer,                                  -- The estimate of the complexity of the issue.
    "sortOrder" float not null,                          -- The order of the item in relation to other items in the organization.
    "subIssueSortOrder" float,                           -- The order of the item in the sub-issue list. Only set if the issue has a parent.
    "integrationSourceType" text,                        -- Integration type that created this issue, if applicable (e.g., 'github', 'slack').
    "customerTicketCount" integer not null default 0,    -- Count of attachments created by customer support ticketing systems.
    "slaStartedAt" timestamp with time zone,             -- The time at which the issue's SLA began.
    "slaBreachesAt" timestamp with time zone,            -- The time at which the issue's SLA will breach.
    "slaHighRiskAt" timestamp with time zone,            -- The time at which the issue's SLA will enter high risk state.
    "slaMediumRiskAt" timestamp with time zone,          -- The time at which the issue's SLA will enter medium risk state.
    "slaType" text,                                      -- The type of SLA set on the issue ('all' or 'onlyBusinessDays').
    "trashed" boolean default false,                     -- A flag that indicates whether the issue is in the trash bin.
    "creatorId" text references "user" ("id") on delete set null, -- The user who created the issue.
    "assigneeId" text references "user" ("id") on delete set null, -- The user to whom the issue is assigned to.
    "snoozedById" text references "user" ("id") on delete set null, -- The user who snoozed the issue.
    "parentId" text references "issue" ("id") on delete cascade, -- The parent of the issue.
    "teamId" text not null references "team" ("id") on delete cascade, -- The team that the issue is associated with.
    "projectId" text references "project" ("id") on delete set null, -- The project that the issue is associated with.
    "cycleId" text references "cycle" ("id") on delete set null, -- The cycle that the issue is associated with.
    "stateId" text not null references "workflow_state" ("id"), -- The workflow state that the issue is associated with.
    "projectMilestoneId" text references "project_milestone" ("id") on delete set null, -- The projectMilestone that the issue is associated with.
    "sourceCommentId" text references "comment" ("id") on delete set null, -- The comment that this issue was created from.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "canceledAt" timestamp with time zone,               -- The time at which the issue was moved into canceled state.
    "completedAt" timestamp with time zone,              -- The time at which the issue was moved into completed state.
    "startedAt" timestamp with time zone,                -- The time at which the issue was moved into started state.
    "snoozedUntilAt" timestamp with time zone,           -- The time until an issue will be snoozed in Triage view.
    "dueDate" date,                                      -- The date at which the issue is due.
    "autoArchivedAt" timestamp with time zone,           -- The time at which the issue was automatically archived by the auto pruning process.
    "autoClosedAt" timestamp with time zone,             -- The time at which the issue was automatically closed by the auto pruning process.
    "addedToCycleAt" timestamp with time zone,           -- The time at which the issue was added to a cycle.
    "addedToProjectAt" timestamp with time zone,         -- The time at which the issue was added to a project.
    "addedToTeamAt" timestamp with time zone,            -- The time at which the issue was added to a team.
    "startedTriageAt" timestamp with time zone,          -- The time at which the issue entered triage.
    "triagedAt" timestamp with time zone                 -- The time at which the issue left triage.
    -- Cannot add recurringIssueTemplateId/lastAppliedTemplateId without Template table
);
-- Ensure combined identifier is unique if required by business logic, e.g., unique(teamId, number)
-- Consider unique constraint on identifier if it's globally unique

-- Represents a comment associated with an issue, project update, etc.
create table "comment" (
    "id" text not null primary key,
    "body" text not null,                                -- The comment content in markdown format.
    "url" text not null,                                 -- Comment's URL.
    "quotedText" text,                                   -- The text that this comment references. Only defined for inline comments.
    "issueId" text references "issue" ("id") on delete cascade, -- The issue that the comment is associated with.
    "projectUpdateId" text references "project_update" ("id") on delete cascade, -- The project update that the comment is associated with.
    "userId" text references "user" ("id") on delete set null, -- The user who wrote the comment.
    "parentId" text references "comment" ("id") on delete cascade, -- The parent comment under which the current comment is nested.
    "resolvingUserId" text references "user" ("id") on delete set null, -- The user that resolved the thread.
    "resolvingCommentId" text references "comment" ("id") on delete set null, -- The comment that resolved the thread.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "editedAt" timestamp with time zone,                 -- The time user edited the comment.
    "resolvedAt" timestamp with time zone                -- The time the resolvingUser resolved the thread.
    -- Needs constraint check (issueId is not null or projectUpdateId is not null etc.) if applicable
);

-- Represents a project update post.
create table "project_update" (
    "id" text not null primary key,
    "body" text not null,                                -- The update content in markdown format.
    "health" text not null,                              -- The health of the project at the time of the update ('onTrack', 'atRisk', 'offTrack').
    "isDiffHidden" boolean not null default false,       -- Whether project update diff should be hidden.
    "isStale" boolean not null default false,            -- Whether the project update is stale.
    "slugId" text not null unique,                       -- The update's unique URL slug.
    "url" text not null,                                 -- The URL to the project update.
    "projectId" text not null references "project" ("id") on delete cascade, -- The project that the update is associated with.
    "userId" text not null references "user" ("id") on delete cascade, -- The user who wrote the update.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    "editedAt" timestamp with time zone                  -- The time the update was edited.
);

-- Represents labels that can be associated with issues.
create table "issue_label" (
    "id" text not null primary key,
    "name" text not null,                                -- The label's name.
    "description" text,                                  -- The label's description.
    "color" text not null,                               -- The label's color as a HEX string.
    "isGroup" boolean not null default false,            -- Whether this label is considered to be a group.
    "teamId" text references "team" ("id") on delete cascade, -- The team that the label is associated with. If null, the label is global/workspace level.
    "parentId" text references "issue_label" ("id") on delete cascade, -- The parent label.
    "creatorId" text references "user" ("id") on delete set null, -- The user who created the label.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
);

-- Represents attachments linked to issues (e.g., files, URLs, external resources).
create table "attachment" (
    "id" text not null primary key,
    "url" text not null,                                 -- Location of the attachment which is also used as an identifier.
    "title" text not null,                               -- Content for the title line in the Linear attachment widget.
    "subtitle" text,                                     -- Content for the subtitle line in the Linear attachment widget.
    "metadata" jsonb not null default '{}'::jsonb,       -- Custom metadata related to the attachment. Use jsonb if available, otherwise text.
    "sourceType" text,                                   -- An accessor helper to source.type, defines the source type of the attachment.
    "groupBySource" boolean not null default false,      -- Indicates if attachments for the same source application should be grouped in the Linear UI.
    "issueId" text not null references "issue" ("id") on delete cascade, -- The issue this attachment belongs to.
    "creatorId" text references "user" ("id") on delete set null, -- The creator of the attachment.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp  -- The last time the entity was meaningfully updated.
    -- Consider unique constraint on (issueId, url) if needed
);

-- Represents an emoji reaction on an issue, comment, or project update.
create table "reaction" (
    "id" text not null primary key,
    "emoji" text not null,                               -- Name of the reaction's emoji.
    "userId" text references "user" ("id") on delete cascade, -- The user that created the reaction.
    "commentId" text references "comment" ("id") on delete cascade, -- The comment that the reaction is associated with.
    "issueId" text references "issue" ("id") on delete cascade, -- The issue that the reaction is associated with.
    "projectUpdateId" text references "project_update" ("id") on delete cascade, -- The project update that the reaction is associated with.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    -- Check constraint to ensure at least one of commentId, issueId, projectUpdateId is not null
    constraint "chk_reaction_target" check (
        ("commentId" is not null)::integer +
        ("issueId" is not null)::integer +
        ("projectUpdateId" is not null)::integer >= 1
    )
);

-- Represents a relationship between two issues (e.g., blocks, duplicate, related).
create table "issue_relation" (
    "id" text not null primary key,
    "type" text not null,                                -- The relationship type ('blocks', 'duplicate', 'related', 'similar').
    "issueId" text not null references "issue" ("id") on delete cascade, -- The issue whose relationship is being described.
    "relatedIssueId" text not null references "issue" ("id") on delete cascade, -- The related issue.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique ("issueId", "relatedIssueId", "type")          -- Ensure relation uniqueness
);

-- Represents a relationship between two projects.
create table "project_relation" (
    "id" text not null primary key,
    "type" text not null,                                -- The relationship type (e.g., 'blocks', 'related').
    "projectId" text not null references "project" ("id") on delete cascade, -- The project whose relationship is being described.
    "relatedProjectId" text not null references "project" ("id") on delete cascade, -- The related project.
    "projectMilestoneId" text references "project_milestone" ("id") on delete cascade, -- Optional milestone anchor for the source project.
    "relatedProjectMilestoneId" text references "project_milestone" ("id") on delete cascade, -- Optional milestone anchor for the related project.
    "anchorType" text not null,                          -- The type of anchor on the project end of the relation.
    "relatedAnchorType" text not null,                   -- The type of anchor on the relatedProject end of the relation.
    "userId" text references "user" ("id") on delete set null, -- The last user who created or modified the relation.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique ("projectId", "relatedProjectId", "type", "projectMilestoneId", "relatedProjectMilestoneId") -- Ensure relation uniqueness potentially including milestones
);

-- Many-to-many relationship: Users belonging to teams.
create table "team_membership" (
    "id" text not null primary key,
    "teamId" text not null references "team" ("id") on delete cascade, -- The team associated with the membership.
    "userId" text not null references "user" ("id") on delete cascade, -- The user associated with the membership.
    "owner" boolean not null default false,              -- Whether the user is the owner of the team.
    "sortOrder" float not null default 0,                -- The order of the item in the users team list.
    "archivedAt" timestamp with time zone,               -- The time at which the entity was archived.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique("teamId", "userId")
);

-- Many-to-many relationship: Users belonging to projects.
create table "project_member" (
    "id" text not null primary key,
    "projectId" text not null references "project" ("id") on delete cascade, -- The project associated with the membership.
    "userId" text not null references "user" ("id") on delete cascade, -- The user associated with the membership.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique("projectId", "userId")
);

-- Many-to-many relationship: Projects belonging to teams.
create table "team_project" (
    "id" text not null primary key,
    "teamId" text not null references "team" ("id") on delete cascade, -- The team associated with the project.
    "projectId" text not null references "project" ("id") on delete cascade, -- The project associated with the team.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique("teamId", "projectId")
);

-- Many-to-many relationship: Labels applied to issues.
create table "issue_label" (
    "id" text not null primary key,
    "issueId" text not null references "issue" ("id") on delete cascade, -- The issue the label is applied to.
    "labelId" text not null references "issue_label" ("id") on delete cascade, -- The label applied to the issue.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique("issueId", "labelId")
);

-- Many-to-many relationship: Users subscribed to issues.
create table "issue_subscriber" (
    "id" text not null primary key,
    "issueId" text not null references "issue" ("id") on delete cascade, -- The issue the user is subscribed to.
    "subscriberId" text not null references "user" ("id") on delete cascade, -- The user subscribed to the issue.
    "createdAt" timestamp with time zone not null default current_timestamp, -- The time at which the entity was created.
    "updatedAt" timestamp with time zone not null default current_timestamp, -- The last time the entity was meaningfully updated.
    unique("issueId", "subscriberId")
);

-- Indices for faster lookups
create index "idx_workflow_state_team" on "workflow_state" ("teamId");
create index "idx_project_status_type" on "project_status" ("type");

create index "idx_team_parent" on "team" ("parentId");
create index "idx_team_key" on "team" ("key");

create index "idx_project_creator" on "project" ("creatorId");
create index "idx_project_lead" on "project" ("leadId");
create index "idx_project_status" on "project" ("statusId");
create index "idx_project_slugId" on "project" ("slugId");
create index "idx_project_convertedFromIssue" on "project" ("convertedFromIssueId");

create index "idx_cycle_team" on "cycle" ("teamId");
create index "idx_cycle_endsAt" on "cycle" ("endsAt");

create index "idx_project_milestone_project" on "project_milestone" ("projectId");

create index "idx_issue_creator" on "issue" ("creatorId");
create index "idx_issue_assignee" on "issue" ("assigneeId");
create index "idx_issue_parent" on "issue" ("parentId");
create index "idx_issue_team" on "issue" ("teamId");
create index "idx_issue_project" on "issue" ("projectId");
create index "idx_issue_cycle" on "issue" ("cycleId");
create index "idx_issue_state" on "issue" ("stateId");
create index "idx_issue_milestone" on "issue" ("projectMilestoneId");
create index "idx_issue_identifier_team" on "issue" ("identifier", "teamId"); -- For team-specific identifiers
create index "idx_issue_sourceComment" on "issue" ("sourceCommentId");
create index "idx_issue_snoozedBy" on "issue" ("snoozedById");
create index "idx_issue_createdAt" on "issue" ("createdAt"); -- Common for sorting
create index "idx_issue_updatedAt" on "issue" ("updatedAt"); -- Common for sorting

create index "idx_comment_issue" on "comment" ("issueId");
create index "idx_comment_user" on "comment" ("userId");
create index "idx_comment_parent" on "comment" ("parentId");
create index "idx_comment_projectUpdate" on "comment" ("projectUpdateId");
create index "idx_comment_resolvingUser" on "comment" ("resolvingUserId");

create index "idx_project_update_project" on "project_update" ("projectId");
create index "idx_project_update_user" on "project_update" ("userId");

create index "idx_issue_label_team" on "issue_label" ("teamId");
create index "idx_issue_label_parent" on "issue_label" ("parentId");
create index "idx_issue_label_creator" on "issue_label" ("creatorId");

create index "idx_attachment_issue" on "attachment" ("issueId");
create index "idx_attachment_creator" on "attachment" ("creatorId");
create index "idx_attachment_url" on "attachment" ("url");

create index "idx_reaction_user" on "reaction" ("userId");
create index "idx_reaction_comment" on "reaction" ("commentId");
create index "idx_reaction_issue" on "reaction" ("issueId");
create index "idx_reaction_project_update" on "reaction" ("projectUpdateId");

create index "idx_issue_relation_issue" on "issue_relation" ("issueId");
create index "idx_issue_relation_related" on "issue_relation" ("relatedIssueId");

create index "idx_project_relation_project" on "project_relation" ("projectId");
create index "idx_project_relation_related" on "project_relation" ("relatedProjectId");
create index "idx_project_relation_milestone" on "project_relation" ("projectMilestoneId");
create index "idx_project_relation_related_milestone" on "project_relation" ("relatedProjectMilestoneId");
create index "idx_project_relation_user" on "project_relation" ("userId");

create index "idx_team_membership_team" on "team_membership" ("teamId");
create index "idx_team_membership_user" on "team_membership" ("userId");

create index "idx_project_member_project" on "project_member" ("projectId");
create index "idx_project_member_user" on "project_member" ("userId");

create index "idx_team_project_team" on "team_project" ("teamId");
create index "idx_team_project_project" on "team_project" ("projectId");

create index "idx_issue_label_issue" on "issue_label" ("issueId");
create index "idx_issue_label_label" on "issue_label" ("labelId");

create index "idx_issue_subscriber_issue" on "issue_subscriber" ("issueId");
create index "idx_issue_subscriber_subscriber" on "issue_subscriber" ("subscriberId");
