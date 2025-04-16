# Issue Database Integration

## Overview

This document describes the implementation of database-driven issue functionality in the OpenAgents website. We've transitioned from mock data to real database storage using Cloudflare D1 with Kysely.

## Key Components

### Database Access Layer

Created a new file `issue-helpers.server.ts` that provides the following functions:

- `getAllIssues()`: Fetches all issues with joined data
- `getIssuesByTeamId(teamId)`: Fetches issues filtered by team
- `getIssuesByProjectId(projectId)`: Fetches issues filtered by project
- `getIssueById(id)`: Fetches a single issue with all its related data
- `createIssue(data)`: Creates a new issue
- `updateIssue(id, data)`: Updates an existing issue
- `getWorkflowStates(teamId?)`: Gets workflow states (optionally filtered by team)
- `getIssueLabels(teamId?)`: Gets issue labels (optionally filtered by team)

### Routes & Components

- Created a new route file `/app/routes/issues.tsx` that serves issues data
- Updated `/app/components/common/issues/all-issues.tsx` to use data from the loader
- Updated `/app/store/issues-store.ts` to support loading from the database
- Fixed `/app/components/common/issues/issue-line.tsx` and `/app/components/common/issues/issue-grid.tsx` to handle the new data structure
- Added an "Issues" link to the workspace navigation

### React Router Updates

- Used `react-router` instead of `react-router-dom` for imports
- Removed use of `json` function from Remix
- Modified loading functions to directly return objects
- Updated error handling to fit React Router v7 patterns

### Fix for Create New Issue Modal

- Updated the `create-new-issue/index.tsx` component:
  - Changed import for Issue type from mock-data to store
  - Replaced `getAllIssues()` function usage with direct access to store's `issues` array
  - Added a custom `generateRank()` function to replace dependency on mock data

### Data Flow

1. `/app/routes/issues.tsx` loads data from the database using `issue-helpers.server.ts`
2. Data is passed to the frontend via the React Router loader
3. The `useIssuesStore` Zustand store is populated with this data using `setIssues()`
4. UI components get their data from the store

## Schema Overview

The database schema is defined in `projects.sql` and includes:

- `issue`: The main issue table
- `workflow_state`: Status types for issues
- `issue_label`: Labels that can be applied to issues
- `issue_to_label`: Junction table for the many-to-many relationship with labels
- Related tables: `project`, `team`, `user`, etc.

## Data Transformation

To maintain backwards compatibility, the database helpers transform database records into a format that matches the previous mock data structure. Key transformations:

- Status information combines fields from `workflow_state`
- Priority numeric values are mapped to strings like 'urgent', 'high', etc.
- Labels are fetched with separate queries and added to the issue objects
- Related data like `project` and `assignee` is joined and formatted

## Issue Creation Implementation

We've implemented a functional issue creation UI that connects to the database:

1. Updated the `create-new-issue/index.tsx` component to:
   - Use React Router's `useSubmit` to send form data to the route action
   - Structure form data to match database field requirements
   - Add validation for required fields
   - Properly handle session authentication
   - Connect state changes to UI components

2. Created a new TeamSelector component that:
   - Fetches teams from the loader data
   - Allows selecting a team for the new issue
   - Displays team icons and identifiers

3. Updated the HeaderIssues component to include a prominent "New Issue" button

4. Integrated the CreateIssueModalProvider in the Issues route

5. Fixed component naming and exports to follow modern React Router conventions

## Next Steps

1. Add issue editing capabilities
2. Implement issue filtering and sorting
3. Add drag-and-drop functionality to move issues between states
4. Implement proper error handling and loading states
5. Enhance the UI with issue details pages and expanded views