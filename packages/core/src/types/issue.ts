/**
 * Shared Issue types to be used across OpenAgents applications and services
 */

/**
 * Base interface for a User across all subsystems
 */
export interface BaseUser {
  id: string;
  name: string;
  email?: string;
  image?: string | null;
}

/**
 * Base interface for an Issue Status/State across all subsystems
 */
export interface IssueStatus {
  id: string;
  name: string;
  color: string;
  type?: string;
}

/**
 * Base interface for an Issue Priority across all subsystems
 */
export interface IssuePriority {
  id: string;
  name: string;
  color: string;
}

/**
 * Base interface for an Issue Label across all subsystems
 */
export interface IssueLabel {
  id: string;
  name: string;
  color: string;
}

/**
 * Base interface for a Project across all subsystems
 */
export interface BaseProject {
  id: string;
  name: string;
  color?: string;
  icon?: string;
}

/**
 * Base interface for a Team across all subsystems
 */
export interface BaseTeam {
  id: string;
  name: string;
  key: string;
}

/**
 * Common issue interface for use across all subsystems
 * This can be extended as needed by different implementations
 */
export interface BaseIssue {
  // Core identifiers
  id: string;
  number?: number;
  identifier?: string; // Usually in format TEAM-123

  // Basic information
  title: string;
  description: string;
  status: string | IssueStatus; // Status can be an ID string or full object
  priority?: string | IssuePriority; // Priority can be an ID string or full object

  // Relationships
  assignee?: BaseUser | string | null; // User object, ID string, or null
  labels?: IssueLabel[] | string[]; // Array of label objects or label IDs
  projectId?: string;
  project?: BaseProject;
  teamId?: string;
  team?: BaseTeam;
  parentId?: string; // For parent-child issue relationships
  creatorId?: string;
  creator?: BaseUser;

  // Additional context
  source?: 'github' | 'linear' | 'other'; // Where the issue originated
  url?: string; // Link to issue in external system

  // Dates
  created: Date | string;
  updated?: Date | string;
  dueDate?: Date | string;
  completedAt?: Date | string;
}

/**
 * Comment on an issue
 */
export interface IssueComment {
  id: string;
  content: string;
  author: string | BaseUser;
  created: Date | string;
}

/**
 * Implementation step interface for solver agent
 */
export interface ImplementationStep {
  id: string;
  description: string;
  type: 'analysis' | 'research' | 'implementation' | 'testing' | 'documentation';
  status: 'pending' | 'in_progress' | 'completed' | 'skipped';
  notes?: string;
  created: Date | string;
  started?: Date | string;
  completed?: Date | string;
  dependsOn?: string[]; // IDs of steps this one depends on
  filePaths?: string[]; // Files involved in this step
}