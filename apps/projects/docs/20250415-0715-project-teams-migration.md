# Project and Team Migrations

This document explains how to run the migrations for project and team tables.

## Database Structure

We've created a relational data model with the following tables:

1. `project` - Contains project information
2. `team` - Contains team information
3. `project_member` - Many-to-many relationship between users and projects (with roles)
4. `team_member` - Many-to-many relationship between users and teams (with roles)
5. `team_project` - Many-to-many relationship between teams and projects

This structure supports:
- Users belonging to multiple projects
- Users belonging to multiple teams
- Projects being owned by a particular user
- Teams being owned by a particular user
- Projects belonging to multiple teams
- Teams having multiple projects

## Running the Migrations

To execute the migrations, run the following commands:

```bash
# Execute locally
npx wrangler d1 execute v5-website --local --file=./migrations/projects-teams.sql

# Execute on remote D1 database
npx wrangler d1 execute v5-website --remote --file=./migrations/projects-teams.sql
```

## Seeding the Database

A seed file is provided to populate the database with sample data. To run it:

1. First, edit the `migrations/seed-projects-teams.ts` file to uncomment the database connection code
2. Then run:

```bash
npx tsx migrations/seed-projects-teams.ts
```

## Type Definitions

Type definitions for the database tables are provided in:
- `app/lib/types/db-schema.ts` - Database model interfaces
- `app/lib/db/schema.ts` - Drizzle ORM schema definitions
- `app/lib/db/types.ts` - Kysely database interface

## Helper Functions

Helper functions for working with projects and teams are provided in:
- `app/lib/db/project-team-helpers.ts`

These functions include:
- Retrieving projects and teams
- Creating projects and teams
- Managing relationships between entities
- Querying users with their related entities

## Usage Examples

```typescript
import {
  getProjects,
  getTeamById,
  createProject,
  addUserToTeam
} from '../lib/db/project-team-helpers';

// Get all projects
const projects = await getProjects();

// Get a specific team with its members and projects
const team = await getTeamById('team-id');

// Create a new project
const projectId = await createProject({
  name: 'New Project',
  status: 'planning',
  icon: '🚀',
  percentComplete: 0,
  startDate: new Date(),
  priority: 'high',
  health: 'on-track',
  ownerId: 'user-id'
});

// Add a user to a team with a specific role
await addUserToTeam('team-id', 'user-id', 'admin');
```

## Relations

The database schema supports these key relationships:

1. A user can own multiple projects
2. A user can own multiple teams
3. A user can be a member of multiple projects with different roles
4. A user can be a member of multiple teams with different roles
5. A project can belong to multiple teams
6. A team can have multiple projects

Each relationship has its own junction table with appropriate foreign keys.
