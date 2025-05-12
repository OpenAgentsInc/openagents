# Project Status Selector Fix

## Issue

The Create Project modal was showing "No statuses available" instead of allowing users to select a status. This occurs because the database doesn't have any project statuses defined yet, resulting in an empty status list from the loader.

## Solution

The fix has two parts:

1. **Client-side Fallback**: Added default statuses for the UI if no statuses are available from the server.
2. **Server-side Creation**: Added logic to create statuses on-the-fly when a default status is selected.

## Changes Made

### 1. Status Selector Component (`/app/components/layout/modals/create-project/status-selector.tsx`)

- Added default status definitions that serve as a fallback:
  ```typescript
  const defaultStatuses: ProjectStatus[] = [
    { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
    { id: 'default-planned', name: 'Planned', color: '#3498DB', type: 'planned' },
    { id: 'default-started', name: 'In Progress', color: '#F1C40F', type: 'started' },
    { id: 'default-completed', name: 'Completed', color: '#2ECC71', type: 'completed' },
    { id: 'default-canceled', name: 'Canceled', color: '#E74C3C', type: 'canceled' }
  ];
  ```

- Updated the status selection logic to use default statuses if none are available from the server:
  ```typescript
  const statuses = (loaderData?.options?.statuses?.length > 0) 
    ? loaderData.options.statuses 
    : defaultStatuses;
  ```

- Removed the "No statuses available" message that was shown when no statuses were found.

### 2. Project Route Action (`/app/routes/projects.tsx`)

- Added logic to detect when a default status is being used:
  ```typescript
  if (projectData.statusId && projectData.statusId.startsWith('default-')) {
    // Create the status in the database
  }
  ```

- Implemented status creation in the database when a default status is selected:
  ```typescript
  await db
    .insertInto('project_status')
    .values({
      id: statusId,
      name: statusName,
      description: `Projects in ${statusName.toLowerCase()} state`,
      color: statusColor,
      type: statusType,
      position: getStatusPosition(statusType),
      indefinite: statusType === 'backlog' || statusType === 'planned' ? 1 : 0,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    })
    .execute();
  ```

- Added helper function to determine status position:
  ```typescript
  function getStatusPosition(type: string): number {
    switch (type) {
      case 'backlog': return 0;
      case 'planned': return 1;
      case 'started': return 2;
      case 'paused': return 3;
      case 'completed': return 4;
      case 'canceled': return 5;
      default: return 0;
    }
  }
  ```

### 3. SQL Migration (`/migrations/seed-project-statuses.sql`)

- Created a SQL script to seed default project statuses:
  ```sql
  INSERT OR IGNORE INTO project_status (id, name, description, color, type, position, indefinite)
  VALUES 
    ('status-backlog', 'Backlog', 'Projects in planning stage', '#95A5A6', 'backlog', 0, 1),
    ('status-planned', 'Planned', 'Projects that are planned to start', '#3498DB', 'planned', 1, 1),
    ('status-started', 'In Progress', 'Projects that are currently in progress', '#F1C40F', 'started', 2, 0),
    ('status-paused', 'Paused', 'Projects that are temporarily paused', '#E67E22', 'paused', 3, 0),
    ('status-completed', 'Completed', 'Projects that are successfully completed', '#2ECC71', 'completed', 4, 0),
    ('status-canceled', 'Canceled', 'Projects that are canceled', '#E74C3C', 'canceled', 5, 0);
  ```

## Execution Flow

1. When the Create Project modal opens, it tries to use statuses from the loader data.
2. If no statuses are found, the UI falls back to the default statuses defined in the component.
3. When the user selects a status and creates a project, the action handler checks if a default status is being used.
4. If a default status is being used, it creates that status in the database before creating the project.
5. The project is then created with the new status ID.

## Admin Note

The seed SQL file can be executed to prepopulate the project statuses:

```bash
npx wrangler d1 execute v5-website --local --file=./migrations/seed-project-statuses.sql
```

This will create the default statuses in the database, allowing them to be used directly without the need for on-the-fly creation.