import { groupIssuesByStatus } from '@/mock-data/issues';
import { type LabelInterface } from '@/mock-data/labels';
import { type Priority } from '@/mock-data/priorities';
import { type Project } from '@/mock-data/projects';
import { type User } from '@/mock-data/users';
import { create } from 'zustand';

// Generic Status interface compatible with both mock and DB data
export interface Status {
  id: string;
  name: string;
  color: string;
  type?: string;
}

// Updated Issue interface to match the database schema
export interface Issue {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  assignees: User | null;
  priority: Priority;
  labels: LabelInterface[];
  createdAt: string | null;
  cycleId: string;
  project?: Project;
  subissues?: string[];
  rank: string;
}

interface IssuesState {
  // Data
  issues: Issue[];
  issuesByStatus: Record<string, Issue[]>;
  isLoaded: boolean;
  workflowStates?: Status[];

  // Actions
  setIssues: (issues: Issue[]) => void;
  setWorkflowStates: (states: Status[]) => void;
  addIssue: (issue: Issue) => void;
  updateIssue: (id: string, updatedIssue: Partial<Issue>) => void;
  deleteIssue: (id: string) => void;

  // Filters
  filterByStatus: (statusId: string) => Issue[];
  filterByPriority: (priorityId: string) => Issue[];
  filterByAssignee: (userId: string | null) => Issue[];
  filterByLabel: (labelId: string) => Issue[];
  filterByProject: (projectId: string) => Issue[];
  searchIssues: (query: string) => Issue[];

  // Status management
  updateIssueStatus: (issueId: string, newStatus: Status) => void;
  getWorkflowStates: () => Status[];

  // Priority management
  updateIssuePriority: (issueId: string, newPriority: Priority) => void;

  // Assignee management
  updateIssueAssignee: (issueId: string, newAssignee: User | null) => void;

  // Labels management
  addIssueLabel: (issueId: string, label: LabelInterface) => void;
  removeIssueLabel: (issueId: string, labelId: string) => void;

  // Project management
  updateIssueProject: (issueId: string, newProject: Project | undefined) => void;

  // Utility functions
  getIssueById: (id: string) => Issue | undefined;
}

export const useIssuesStore = create<IssuesState>((set, get) => ({
  // Initial state with empty data (to be loaded from API)
  issues: [],
  issuesByStatus: {},
  isLoaded: false,
  workflowStates: [
    { id: 'default-triage', name: 'Triage', color: '#6B7280', type: 'triage' },
    { id: 'default-backlog', name: 'Backlog', color: '#95A5A6', type: 'backlog' },
    { id: 'default-todo', name: 'To Do', color: '#3498DB', type: 'todo' },
    { id: 'default-inprogress', name: 'In Progress', color: '#F1C40F', type: 'inprogress' },
    { id: 'default-done', name: 'Done', color: '#2ECC71', type: 'done' }
  ],

  // Set all issues (used when loading from API)
  setIssues: (issues: Issue[]) => {
    console.log('[DEBUG] IssuesStore - Setting issues:', issues.length);
    console.log('[DEBUG] IssuesStore - Issues by status:', Object.keys(groupIssuesByStatus(issues)));
    set({
      issues,
      issuesByStatus: groupIssuesByStatus(issues),
      isLoaded: true,
    });
  },
  
  // Set workflow states (used when loading from API)
  setWorkflowStates: (states: Status[]) => {
    set({
      workflowStates: states,
    });
  },
  
  // Get workflow states
  getWorkflowStates: () => {
    return get().workflowStates || [];
  },

  // Actions
  addIssue: (issue: Issue) => {
    set((state) => {
      const newIssues = [...state.issues, issue];
      return {
        issues: newIssues,
        issuesByStatus: groupIssuesByStatus(newIssues),
      };
    });
  },

  updateIssue: (id: string, updatedIssue: Partial<Issue>) => {
    set((state) => {
      const newIssues = state.issues.map((issue) =>
        issue.id === id ? { ...issue, ...updatedIssue } : issue
      );

      return {
        issues: newIssues,
        issuesByStatus: groupIssuesByStatus(newIssues),
      };
    });
  },

  deleteIssue: (id: string) => {
    set((state) => {
      const newIssues = state.issues.filter((issue) => issue.id !== id);
      return {
        issues: newIssues,
        issuesByStatus: groupIssuesByStatus(newIssues),
      };
    });
  },

  // Filters with error handling
  filterByStatus: (statusId: string) => {
    return get().issues.filter((issue) => {
      // Skip invalid issues
      if (!issue || !issue.status) return false;
      return issue.status.id === statusId;
    });
  },

  filterByPriority: (priorityId: string) => {
    return get().issues.filter((issue) => {
      // Skip invalid issues
      if (!issue || !issue.priority) return false;
      return issue.priority.id === priorityId;
    });
  },

  filterByAssignee: (userId: string | null) => {
    if (userId === null) {
      return get().issues.filter((issue) => {
        if (!issue) return false;
        return issue.assignees === null;
      });
    }
    return get().issues.filter((issue) => {
      if (!issue || !issue.assignees) return false;
      return issue.assignees.id === userId;
    });
  },

  filterByLabel: (labelId: string) => {
    return get().issues.filter((issue) => {
      if (!issue || !issue.labels || !Array.isArray(issue.labels)) return false;
      return issue.labels.some((label) => label && label.id === labelId);
    });
  },

  filterByProject: (projectId: string) => {
    return get().issues.filter((issue) => {
      if (!issue || !issue.project) return false;
      return issue.project.id === projectId;
    });
  },

  searchIssues: (query: string) => {
    const lowerCaseQuery = query.toLowerCase();
    return get().issues.filter(issue => {
      if (!issue) return false;
      
      const titleMatch = issue.title && issue.title.toLowerCase().includes(lowerCaseQuery);
      const identifierMatch = issue.identifier && issue.identifier.toLowerCase().includes(lowerCaseQuery);
      
      return titleMatch || identifierMatch;
    });
  },

  // Status management
  updateIssueStatus: (issueId: string, newStatus: Status) => {
    get().updateIssue(issueId, { status: newStatus });
  },

  // Priority management
  updateIssuePriority: (issueId: string, newPriority: Priority) => {
    get().updateIssue(issueId, { priority: newPriority });
  },

  // Assignee management
  updateIssueAssignee: (issueId: string, newAssignee: User | null) => {
    get().updateIssue(issueId, { assignees: newAssignee });
  },

  // Labels management
  addIssueLabel: (issueId: string, label: LabelInterface) => {
    const issue = get().getIssueById(issueId);
    if (issue) {
      const updatedLabels = [...issue.labels, label];
      get().updateIssue(issueId, { labels: updatedLabels });
    }
  },

  removeIssueLabel: (issueId: string, labelId: string) => {
    const issue = get().getIssueById(issueId);
    if (issue) {
      const updatedLabels = issue.labels.filter((label) => label.id !== labelId);
      get().updateIssue(issueId, { labels: updatedLabels });
    }
  },

  // Project management
  updateIssueProject: (issueId: string, newProject: Project | undefined) => {
    get().updateIssue(issueId, { project: newProject });
  },

  // Utility functions
  getIssueById: (id: string) => {
    return get().issues.find((issue) => issue.id === id);
  },
}));