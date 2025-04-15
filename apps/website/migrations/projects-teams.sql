-- Project related tables
create table "project" (
    "id" text not null primary key,
    "name" text not null,
    "status" text not null,
    "icon" text not null,
    "percentComplete" integer not null default 0,
    "startDate" date not null,
    "priority" text not null, 
    "health" text not null,
    "createdAt" date not null default current_timestamp,
    "updatedAt" date not null default current_timestamp,
    "ownerId" text not null references "user" ("id")
);

create table "team" (
    "id" text not null primary key,
    "name" text not null,
    "icon" text not null,
    "color" text not null,
    "createdAt" date not null default current_timestamp,
    "updatedAt" date not null default current_timestamp,
    "ownerId" text not null references "user" ("id")
);

-- Many-to-many relationships

-- Users belonging to projects with permissions
create table "project_member" (
    "id" text not null primary key,
    "projectId" text not null references "project" ("id") on delete cascade,
    "userId" text not null references "user" ("id") on delete cascade,
    "role" text not null default 'member', -- could be 'owner', 'admin', 'member', etc.
    "createdAt" date not null default current_timestamp,
    "updatedAt" date not null default current_timestamp,
    unique("projectId", "userId")
);

-- Users belonging to teams with permissions
create table "team_member" (
    "id" text not null primary key,
    "teamId" text not null references "team" ("id") on delete cascade,
    "userId" text not null references "user" ("id") on delete cascade,
    "role" text not null default 'member', -- could be 'owner', 'admin', 'member', etc.
    "createdAt" date not null default current_timestamp,
    "updatedAt" date not null default current_timestamp,
    unique("teamId", "userId")
);

-- Projects belonging to teams
create table "team_project" (
    "id" text not null primary key,
    "teamId" text not null references "team" ("id") on delete cascade,
    "projectId" text not null references "project" ("id") on delete cascade,
    "createdAt" date not null default current_timestamp,
    "updatedAt" date not null default current_timestamp,
    unique("teamId", "projectId")
);

-- Create indices for faster lookups
create index "idx_project_owner" on "project" ("ownerId");
create index "idx_team_owner" on "team" ("ownerId");
create index "idx_project_member_project" on "project_member" ("projectId");
create index "idx_project_member_user" on "project_member" ("userId");
create index "idx_team_member_team" on "team_member" ("teamId");
create index "idx_team_member_user" on "team_member" ("userId");
create index "idx_team_project_team" on "team_project" ("teamId");
create index "idx_team_project_project" on "team_project" ("projectId");