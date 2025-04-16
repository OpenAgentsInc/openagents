import { LexoRank } from '@/lib/utils';
import { type LabelInterface } from './labels';
import { type Priority } from './priorities';
import { type Project } from './projects';
import { type Status } from './status';
import { type User } from './users';
import { issues } from './issues-data';

export interface Issue {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  assignee: User | null; // Changed from assignees to assignee to match store interface
  priority: Priority;
  labels: LabelInterface[];
  createdAt: string | null;
  cycleId: string;
  project?: Project;
  subissues?: string[];
  rank: string;
}

// generates issues ranks using LexoRank algorithm.
export const ranks: string[] = [];
const generateIssuesRanks = () => {
  const firstRank = new LexoRank('a3c');
  ranks.push(firstRank.toString());
  for (let i = 1; i < 30; i++) {
    const previousRank = LexoRank.from(ranks[i - 1]);
    const currentRank = previousRank.increment();
    ranks.push(currentRank.toString());
  }
};
generateIssuesRanks();

export function groupIssuesByStatus(issues: Issue[]): Record<string, Issue[]> {
  // Guard against null/undefined issues
  if (!issues || !Array.isArray(issues)) {
    console.error('Invalid issues array in groupIssuesByStatus');
    return {};
  }
  
  // Clean the issues array to ensure all items have valid status objects
  const validIssues = issues.filter(issue => {
    if (!issue) return false;
    
    // For issues without status at all, give them a default
    if (!issue.status) {
      console.warn(`Issue ${issue.id}: "${issue.title}" missing status, using default backlog`);
      issue.status = {
        id: 'default-backlog',
        name: 'Backlog',
        color: '#95A5A6',
        type: 'backlog'
      };
    } 
    // For issues with status object but no id, give a default id
    else if (!issue.status.id) {
      console.warn(`Issue ${issue.id}: "${issue.title}" has status but no id, using default backlog id`);
      issue.status.id = 'default-backlog';
    }
    
    return true;
  });
  
  // Group the valid issues by status
  const result = validIssues.reduce<Record<string, Issue[]>>((acc, issue) => {
    const statusId = issue.status.id;
    
    if (!acc[statusId]) {
      acc[statusId] = [];
    }

    acc[statusId].push(issue);
    return acc;
  }, {});
  
  return result;
}

export function sortIssuesByPriority(issues: Issue[]): Issue[] {
  const priorityOrder: Record<string, number> = {
    'urgent': 0,
    'high': 1,
    'medium': 2,
    'low': 3,
    'no-priority': 4,
  };

  return issues
    .slice()
    .sort(
      (a, b) =>
        priorityOrder[a.priority.id as keyof typeof priorityOrder] -
        priorityOrder[b.priority.id as keyof typeof priorityOrder]
    );
}

export { issues };
