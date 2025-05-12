# Linear API Integration

This document outlines the implementation of database schemas and models based on the Linear API for the OpenAgents project. The implementation replaces mock data with real database queries and integrates with Cloudflare D1.

## Changes Made

### Database Schema
1. Added Linear-API-inspired schema in `/apps/website/migrations/projects.sql`
   - Comprehensive project, team, workflow state, issue tables
   - Many-to-many relationship tables
   - Indices for performance optimization

### Database Types
1. Updated `/apps/website/app/lib/db/types.ts` with complete TypeScript interface for all tables
   - Authentication tables (user, session, account, verification)
   - Project management tables (project, team, issue, etc.)
   - Relationship tables and indices

### Database Helper Files
1. Created `/apps/website/app/lib/db/project-helpers.ts`
   - Project CRUD operations
   - Query methods with proper relationships
   - Data transformation for frontend consumption

2. Created `/apps/website/app/lib/db/team-helpers.ts`
   - Team CRUD operations
   - Query methods with relationships
   - Member management

### Components
1. Updated `/apps/website/app/components/common/teams/teams.tsx`
   - Connected to real database via loaders
   - Added error handling
   - Empty state handling

2. Updated `/apps/website/app/components/common/teams/team-line.tsx`
   - Updated to use real database schema
   - Added member and project count display

3. Updated `/apps/website/app/components/common/projects/projects.tsx`
   - Connected to database via loaders
   - Added error and empty state handling

4. Updated `/apps/website/app/components/common/projects/project-line.tsx`
   - Adapted to new data schema
   - Added icon loader

5. Created `/apps/website/app/components/ui/icon-loader.tsx`
   - Utility for loading icons from names
   - Supports Lucide icons and emoji

### Routes
1. Updated `/apps/website/app/routes/projects.tsx`
   - Added loader function for data fetching
   - Connected to project-helpers
   - Removed deprecated json() wrapper for modern Remix
   - Added error handling

2. Updated `/apps/website/app/routes/teams.tsx`
   - Added loader function for data fetching
   - Connected to team-helpers
   - Removed deprecated json() wrapper for modern Remix
   - Added error handling

### Seeding
1. Created `/apps/website/migrations/seed-linear-data.ts`
   - Seed script for populating initial data
   - Creates test user, teams, project statuses, projects
   - Sets up relationships between entities

## Data Models

The implementation is based on Linear's data model with these core entities:

### Core Entities
- **User**: Authentication and user identity
- **Team**: Organizational unit for projects and issues
- **Project**: Collection of issues with a common goal and timeframe
- **Issue**: Core unit of work
- **WorkflowState**: States within a team's workflow (triage, backlog, etc.)
- **ProjectStatus**: Status of a project (backlog, planned, started, etc.)

### Relationships
- Users can belong to multiple teams
- Projects can belong to multiple teams
- Users can be members of multiple projects
- Issues belong to teams and can optionally belong to projects

## Running the Implementation

To use this implementation:

1. Run the migrations:
   ```bash
   npx wrangler d1 execute v5-website --local --file=./migrations/projects.sql
   ```

2. Modify and run the seed script:
   ```bash
   npx tsx migrations/seed-linear-data.ts
   ```

3. Start the application:
   ```bash
   npm run dev
   ```

## Future Improvements

1. Add issues view and component
2. Implement proper authentication and permission checks
3. Add create/edit forms for teams and projects
4. Add webhooks to sync with external systems
5. Implement real-time updates

## References

- Linear API Documentation: https://developers.linear.app/docs/
- Cloudflare D1 Documentation: https://developers.cloudflare.com/d1/