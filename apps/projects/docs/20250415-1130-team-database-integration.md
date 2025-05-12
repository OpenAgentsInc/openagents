# Team Database Integration

## Overview
This document outlines the implementation of database-driven teams functionality in the OpenAgents website application. The implementation replaces mock data with real database operations using Cloudflare D1 and Kysely, following React Router v7 patterns.

## Changes

### Database Operations
- Updated `team-helpers.server.ts` to include the following operations:
  - `getTeams`: Retrieves all teams from the database with calculated member and project counts
  - `getTeamById`: Retrieves a specific team with its members and projects
  - `getTeamsForUser`: Retrieves teams a specified user is a member of
  - `createTeam`: Creates a new team and adds the creator as an owner
  - `addMemberToTeam`: Adds a user to a team with specified ownership status

### React Router Integration
- Updated the teams route (`teams.tsx`) to follow React Router v7 patterns:
  - Removed Remix's `json()` wrapper in favor of returning plain objects directly
  - Added proper authentication checks and redirects
  - Implemented form-based team creation through a route action
  - Used direct form submission instead of JSON stringify/parse approach

### UI Components
- Created a Team creation modal pattern similar to the Project creation:
  - Added `CreateTeam` component with form validation
  - Implemented reusable `IconPicker` and `ColorPicker` components
  - Connected to authentication through `useSession` from auth-client
  - Used React Router's `useSubmit` for form submission to route action

### Store Management
- Added a `create-team-store.ts` using Zustand for state management:
  - Controls modal visibility state
  - Provides open/close functionality for the modal

### Header Integration
- Updated Teams header components to use named exports
- Added "Add team" button with modal trigger
- Connected header with loader data for team counts

## Database Schema
The implementation uses the following tables from `projects.sql`:
- `team`: Stores team data including name, key, description, settings
- `team_membership`: Defines the many-to-many relationship between teams and users
- `team_project`: Defines the many-to-many relationship between teams and projects

## Authentication Integration
- Server-side: Using `auth.api.getSession(request)` to get the current user
- Client-side: Using `useSession()` hook to access session data for UI permissions
- Automatically adding the authenticated user as an owner when creating a team

## Integration with Projects and Issues
- Updated the projects route to use `getTeamsForUser` instead of the generic `getTeams`
- Updated the issues route to also use `getTeamsForUser` to make teams available in the issue creation modal
- This ensures that when creating a project or issue, only teams that the user is a member of are shown in the selectors
- Teams and projects are properly associated through the `team_project` join table
- Issues are properly associated with teams through the direct `teamId` field

## Future Improvements
- Implement team detail page with member management
- Add ability to join/leave teams
- Implement team settings page for advanced configuration
- Add proper role-based permissions for team operations