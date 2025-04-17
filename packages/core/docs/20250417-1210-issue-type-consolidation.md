# Issue Type Consolidation

**Date:** 2025-04-17  
**Author:** OpenAgents Team

## Overview

This document outlines the consolidation of Issue-related types across the OpenAgents platform. Previously, issue types were defined independently in different parts of the codebase (packages/agents/src/agents/solver/types.ts, apps/website/app/store/issues-store.ts, etc.). This led to potential inconsistencies and duplication.

## Implementation

### 1. Created Unified Types

We've added a central location for Issue-related types in `packages/core/src/types/issue.ts`. This file defines base interfaces that can be extended by different parts of the system:

- `BaseUser` - Common user interface
- `IssueStatus` - Issue status/state properties
- `IssuePriority` - Priority interface
- `IssueLabel` - Label properties
- `BaseProject` - Project properties
- `BaseTeam` - Team properties
- `BaseIssue` - Core issue interface with common properties
- `IssueComment` - Comment interface
- `ImplementationStep` - For Solver agent implementation steps

### 2. Making Types Flexible

The types are designed to be flexible and accommodate different usage patterns:

```typescript
export interface BaseIssue {
  // ... 
  status: string | IssueStatus; // Status can be an ID string or full object
  priority?: string | IssuePriority; // Priority can be an ID string or full object
  assignee?: BaseUser | string | null; // User object, ID string, or null
  labels?: IssueLabel[] | string[]; // Array of label objects or label IDs
  // ...
  created: Date | string; // Dates can be Date objects or ISO strings
}
```

### 3. Updated Implementations

The following files were updated to use the common types:

1. **Solver Agent Types**:
   - Updated `packages/agents/src/agents/solver/types.ts` to import and use `BaseIssue`, `ImplementationStep`, and `IssueComment`
   - Added a `SolverIssue` type that extends `BaseIssue` for potential solver-specific fields

2. **Website Issue Store**:
   - Updated `apps/website/app/store/issues-store.ts` to import and use `BaseUser` and `IssueStatus`
   - Redefined `User` as a type alias for `BaseUser`
   - Extended `Status` from `BaseIssueStatus`

3. **Core Package Exports**:
   - Updated `packages/core/src/index.ts` to export the issue types, making them available via `@openagents/core`

## Benefits

1. **Single Source of Truth**: Common issue properties are defined in one place
2. **Type Consistency**: All parts of the system use consistent types
3. **Flexibility**: Types accommodate different data representations (e.g., ID strings vs. objects)
4. **Reduced Duplication**: Eliminates duplicate definitions across the codebase
5. **Extension Points**: Base types can be extended for specific needs

## Usage Examples

### Importing Types

```typescript
// Import directly from the core package
import { BaseIssue, IssueStatus, BaseUser, ImplementationStep } from '@openagents/core';
```

### Extending Base Types

```typescript
// Extend the base issue type for specific requirements
export interface CustomIssue extends BaseIssue {
  customField: string;
  customStatus: 'pending' | 'approved' | 'rejected';
}
```

### Using with API Responses

```typescript
// Mapping API response to our common type
function mapApiResponseToIssue(response: any): BaseIssue {
  return {
    id: response.id,
    title: response.title,
    description: response.description || '',
    status: {
      id: response.statusId,
      name: response.statusName,
      color: response.statusColor
    },
    // ...other fields
    created: new Date(response.createdAt)
  };
}
```

## Future Improvements

1. **Complete Migration**: More components and services should be updated to use these common types
2. **Improved Validation**: Add runtime validation for issue objects
3. **Schema Integration**: Align these TypeScript interfaces with database schemas
4. **Documentation**: Add JSDoc comments to all type definitions
5. **Testing**: Add type tests to ensure type compatibility