# Teams in OpenAgents

OpenAgents implements a team-based collaboration system that allows users to work together on projects and share resources. This document outlines how teams are implemented and used throughout the platform.

## Core Concepts

### Team Membership
- Users can belong to multiple teams
- Users can have a "current team" context
- Users can also work in a "personal" context (null team)
- Teams can have many users as members

### Team Resources
Teams serve as organizational units that own various resources:
- Projects
- Threads (through projects)
- Contact Tags (CRM)
- Contacts (optional, through companies)

## Implementation Details

### Models and Relationships

#### Team Model
- Has many users (many-to-many relationship)
- Has many projects
- Has many threads through projects
- Can have many users with it set as their current team

#### User Model
- Can belong to multiple teams
- Has an optional current_team relationship
- Can access projects through their current team
- Can work in personal context (null current_team)

### Team Switching

The platform implements a team switcher component (`TeamSwitcher.tsx`) that allows users to:
- View their personal account context
- View and switch between teams they belong to
- Switch team context via a POST request to `/switch-team`

### Team Context
- Users can work in either team or personal context
- Personal context is represented by a null team_id
- Team context affects resource visibility and access
- Current team context is preserved across sessions

## Frontend Implementation

The team switcher UI provides:
- A dropdown interface for team selection
- Clear visual indication of current team context
- Separation between personal and team contexts
- Smooth state management during team switches

## Testing

The team functionality is thoroughly tested with both unit and feature tests:

### Unit Tests
- Team membership and relationships
- User-team associations
- Project ownership
- Thread ownership through projects
- Current team functionality
- CRM resource ownership

### Key Test Cases
- Team-user many-to-many relationships
- Current team assignment and retrieval
- Project ownership validation
- Thread ownership through project association
- Contact and tag team associations

## Security Considerations

- Resources are scoped to teams
- Users can only access resources of their current team
- Personal resources are protected from team access
- Team switching requires proper authorization

## Future Enhancements

- Team roles and permissions
- Team resource sharing settings
- Team activity logging
- Team member management UI
- Team billing and subscription management