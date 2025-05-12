# Project Management Data Structures Documentation

This document provides a comprehensive overview of all data structures used in the project management system.

## Project Structure

A project has the following fields:

```typescript
interface Project {
  id: string;                    // Unique identifier
  name: string;                  // Project name
  status: Status;               // Current status
  icon: LucideIcon;            // Visual representation
  percentComplete: number;      // Progress (0-100)
  startDate: string;           // Format: YYYY-MM-DD
  lead: User;                  // Project lead
  priority: Priority;          // Project priority
  health: Health;              // Project health status
}
```

### Project Health Status Values

```typescript
type Health = {
  id: 'no-update' | 'off-track' | 'on-track' | 'at-risk';
  name: string;
  color: string;
  description: string;
}
```

Available health statuses:
- `no-update`: No updates in last 30 days (Color: #FF0000)
- `off-track`: Project is delayed (Color: #FF0000)
- `on-track`: Project is on schedule (Color: #00FF00)
- `at-risk`: Project might be delayed (Color: #FF0000)

## Status Values

```typescript
interface Status {
  id: string;
  name: string;
  color: string;
  icon: React.FC;
}
```

Available statuses:
- `in-progress`: In Progress (Color: #facc15)
- `technical-review`: Technical Review (Color: #22c55e)
- `completed`: Completed (Color: #8b5cf6)
- `paused`: Paused (Color: #0ea5e9)
- `to-do`: Todo (Color: #f97316)
- `backlog`: Backlog (Color: #ec4899)

## Priority Levels

```typescript
interface Priority {
  id: string;
  name: string;
  icon: React.FC;
}
```

Available priorities (in order of importance):
1. `urgent`: Urgent
2. `high`: High
3. `medium`: Medium
4. `low`: Low
5. `no-priority`: No priority

## Labels

```typescript
interface LabelInterface {
  id: string;
  name: string;
  color: string;
}
```

Available labels:
- `ui`: UI Enhancement (Color: purple)
- `bug`: Bug (Color: red)
- `feature`: Feature (Color: green)
- `documentation`: Documentation (Color: blue)
- `refactor`: Refactor (Color: yellow)
- `performance`: Performance (Color: orange)
- `design`: Design (Color: pink)
- `security`: Security (Color: gray)
- `accessibility`: Accessibility (Color: indigo)
- `testing`: Testing (Color: teal)
- `internationalization`: Internationalization (Color: cyan)

## User Roles and Status

```typescript
interface User {
  id: string;
  name: string;
  avatarUrl: string;
  email: string;
  status: 'online' | 'offline' | 'away';
  role: 'Member' | 'Admin' | 'Guest';
  joinedDate: string;
  teamIds: string[];
}
```

User status colors:
- `online`: #00cc66
- `offline`: #969696
- `away`: #ffcc00

## Teams

```typescript
interface Team {
  id: string;
  name: string;
  icon: string;
  joined: boolean;
  color: string;
  members: User[];
  projects: Project[];
}
```

Available teams:
- `CORE`: LNDev Core (🛠️)
- `DESIGN`: Design System (🎨)
- `PERF`: Performance Lab (☀️)
- `UX`: UX Team (👨🏼‍🎨)
- `DATA`: Data Science (📊)
- `MOBILE`: Mobile Development (📱)
- `WEB`: Web Development (🌐)
- `UI`: UI Team (👨🏼‍🎨)
- `CLOUD`: Cloud Infrastructure (☁️)
- `SECURITY`: Security Team (🔒)
- `AI`: AI Research (🧠)
- `QA`: Quality Assurance (✅)
- `DEVOPS`: DevOps (⚙️)
- `FRONTEND`: Frontend Experts (🖥️)
- `BACKEND`: Backend Engineers (🗄️)
- `PRODUCT`: Product (📋)
- `ANALYTICS`: Analytics Team (📈)
- `INNO`: Innovation Lab (💡)

## Cycles (Sprints)

```typescript
interface Cycle {
  id: string;
  number: number;
  name: string;
  teamId: string;
  startDate: string;
  endDate: string;
  progress: number;
}
```

## Common Usage Examples

### Setting Project Status
To mark a project as completed:
```typescript
project.status = status.find(s => s.id === 'completed');
```

### Updating Project Health
To mark a project as on track:
```typescript
project.health = health.find(h => h.id === 'on-track');
```

### Setting Priority
To set urgent priority:
```typescript
project.priority = priorities.find(p => p.id === 'urgent');
```

### Adding Labels
To add a bug label:
```typescript
issue.labels.push(labels.find(l => l.id === 'bug'));
```
