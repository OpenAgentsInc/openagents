import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { env } from 'cloudflare:workers';
import { Database } from './types';

// Initialize the DB connection
export function getDb() {
  return new Kysely<Database>({
    dialect: new D1Dialect({
      database: env.DB as any,
    }),
  });
}

// Get all issues
export async function getAllIssues() {
  const db = getDb();
  
  const issues = await db
    .selectFrom('issue')
    .leftJoin('workflow_state', 'workflow_state.id', 'issue.stateId')
    .leftJoin('user as assignee', 'assignee.id', 'issue.assigneeId')
    .leftJoin('user as creator', 'creator.id', 'issue.creatorId')
    .leftJoin('project', 'project.id', 'issue.projectId')
    .leftJoin('team', 'team.id', 'issue.teamId')
    .select([
      'issue.id',
      'issue.title',
      'issue.description',
      'issue.priority',
      'issue.prioritySortOrder',
      'issue.identifier',
      'issue.number',
      'issue.estimate',
      'issue.sortOrder',
      'issue.createdAt',
      'issue.dueDate',
      'workflow_state.id as stateId',
      'workflow_state.name as stateName',
      'workflow_state.color as stateColor',
      'workflow_state.type as stateType',
      'assignee.id as assigneeId',
      'assignee.name as assigneeName',
      'assignee.image as assigneeImage',
      'creator.id as creatorId',
      'creator.name as creatorName',
      'project.id as projectId',
      'project.name as projectName',
      'project.icon as projectIcon',
      'project.color as projectColor',
      'team.id as teamId',
      'team.name as teamName',
      'team.key as teamKey',
      'issue.cycleId'
    ])
    .where('issue.archivedAt', 'is', null)
    .orderBy('issue.sortOrder', 'asc')
    .execute();

  // Get labels for all fetched issues
  const issueIds = issues.map(issue => issue.id);
  
  // If no issues found, return empty array
  if (issueIds.length === 0) {
    return [];
  }

  // Fetch labels for all issues in a single query
  const issueLabels = await db
    .selectFrom('issue_to_label')
    .innerJoin('issue_label', 'issue_label.id', 'issue_to_label.labelId')
    .select([
      'issue_to_label.issueId',
      'issue_label.id as labelId',
      'issue_label.name as labelName',
      'issue_label.color as labelColor'
    ])
    .where('issue_to_label.issueId', 'in', issueIds)
    .execute();

  // Group labels by issueId
  const labelsByIssueId = issueLabels.reduce((acc, label) => {
    if (!acc[label.issueId]) {
      acc[label.issueId] = [];
    }
    acc[label.issueId].push({
      id: label.labelId,
      name: label.labelName,
      color: label.labelColor
    });
    return acc;
  }, {} as Record<string, {id: string, name: string, color: string}[]>);

  // Transform to match front-end expectations
  return issues.map(issue => ({
    id: issue.id,
    identifier: issue.identifier,
    title: issue.title,
    description: issue.description || '',
    status: {
      id: issue.stateId || '',
      name: issue.stateName || '',
      color: issue.stateColor || '',
      type: issue.stateType || '',
    },
    assignees: issue.assigneeId ? {
      id: issue.assigneeId,
      name: issue.assigneeName || '',
      image: issue.assigneeImage || null,
    } : null,
    priority: {
      id: getPriorityKey(issue.priority),
      name: getPriorityName(issue.priority),
      color: getPriorityColor(issue.priority),
    },
    labels: labelsByIssueId[issue.id] || [],
    createdAt: issue.createdAt,
    cycleId: issue.cycleId || '',
    rank: issue.sortOrder.toString(), // Use sortOrder as rank
    project: issue.projectId ? {
      id: issue.projectId,
      name: issue.projectName || '',
      icon: issue.projectIcon || '',
      color: issue.projectColor || '',
    } : undefined,
  }));
}

// Get issues by team ID
export async function getIssuesByTeamId(teamId: string) {
  const db = getDb();
  
  const issues = await db
    .selectFrom('issue')
    .leftJoin('workflow_state', 'workflow_state.id', 'issue.stateId')
    .leftJoin('user as assignee', 'assignee.id', 'issue.assigneeId')
    .leftJoin('user as creator', 'creator.id', 'issue.creatorId')
    .leftJoin('project', 'project.id', 'issue.projectId')
    .select([
      'issue.id',
      'issue.title',
      'issue.description',
      'issue.priority',
      'issue.prioritySortOrder',
      'issue.identifier',
      'issue.number',
      'issue.estimate',
      'issue.sortOrder',
      'issue.createdAt',
      'issue.dueDate',
      'workflow_state.id as stateId',
      'workflow_state.name as stateName',
      'workflow_state.color as stateColor',
      'workflow_state.type as stateType',
      'assignee.id as assigneeId',
      'assignee.name as assigneeName',
      'assignee.image as assigneeImage',
      'creator.id as creatorId',
      'creator.name as creatorName',
      'project.id as projectId',
      'project.name as projectName',
      'project.icon as projectIcon',
      'project.color as projectColor',
      'issue.cycleId'
    ])
    .where('issue.teamId', '=', teamId)
    .where('issue.archivedAt', 'is', null)
    .orderBy('issue.sortOrder', 'asc')
    .execute();

  // Get labels for all fetched issues
  const issueIds = issues.map(issue => issue.id);
  
  // If no issues found, return empty array
  if (issueIds.length === 0) {
    return [];
  }

  // Fetch labels for all issues in a single query
  const issueLabels = await db
    .selectFrom('issue_to_label')
    .innerJoin('issue_label', 'issue_label.id', 'issue_to_label.labelId')
    .select([
      'issue_to_label.issueId',
      'issue_label.id as labelId',
      'issue_label.name as labelName',
      'issue_label.color as labelColor'
    ])
    .where('issue_to_label.issueId', 'in', issueIds)
    .execute();

  // Group labels by issueId
  const labelsByIssueId = issueLabels.reduce((acc, label) => {
    if (!acc[label.issueId]) {
      acc[label.issueId] = [];
    }
    acc[label.issueId].push({
      id: label.labelId,
      name: label.labelName,
      color: label.labelColor
    });
    return acc;
  }, {} as Record<string, {id: string, name: string, color: string}[]>);

  // Transform to match front-end expectations
  return issues.map(issue => ({
    id: issue.id,
    identifier: issue.identifier,
    title: issue.title,
    description: issue.description || '',
    status: {
      id: issue.stateId || '',
      name: issue.stateName || '',
      color: issue.stateColor || '',
      type: issue.stateType || '',
    },
    assignees: issue.assigneeId ? {
      id: issue.assigneeId,
      name: issue.assigneeName || '',
      image: issue.assigneeImage || null,
    } : null,
    priority: {
      id: getPriorityKey(issue.priority),
      name: getPriorityName(issue.priority),
      color: getPriorityColor(issue.priority),
    },
    labels: labelsByIssueId[issue.id] || [],
    createdAt: issue.createdAt,
    cycleId: issue.cycleId || '',
    rank: issue.sortOrder.toString(), // Use sortOrder as rank
    project: issue.projectId ? {
      id: issue.projectId,
      name: issue.projectName || '',
      icon: issue.projectIcon || '',
      color: issue.projectColor || '',
    } : undefined,
  }));
}

// Get issues by project ID
export async function getIssuesByProjectId(projectId: string) {
  const db = getDb();
  
  const issues = await db
    .selectFrom('issue')
    .leftJoin('workflow_state', 'workflow_state.id', 'issue.stateId')
    .leftJoin('user as assignee', 'assignee.id', 'issue.assigneeId')
    .leftJoin('user as creator', 'creator.id', 'issue.creatorId')
    .select([
      'issue.id',
      'issue.title',
      'issue.description',
      'issue.priority',
      'issue.prioritySortOrder',
      'issue.identifier',
      'issue.number',
      'issue.estimate',
      'issue.sortOrder',
      'issue.createdAt',
      'issue.dueDate',
      'workflow_state.id as stateId',
      'workflow_state.name as stateName',
      'workflow_state.color as stateColor',
      'workflow_state.type as stateType',
      'assignee.id as assigneeId',
      'assignee.name as assigneeName',
      'assignee.image as assigneeImage',
      'creator.id as creatorId',
      'creator.name as creatorName',
      'issue.cycleId',
      'issue.teamId'
    ])
    .where('issue.projectId', '=', projectId)
    .where('issue.archivedAt', 'is', null)
    .orderBy('issue.sortOrder', 'asc')
    .execute();

  // Get labels for all fetched issues
  const issueIds = issues.map(issue => issue.id);
  
  // If no issues found, return empty array
  if (issueIds.length === 0) {
    return [];
  }

  // Fetch labels for all issues in a single query
  const issueLabels = await db
    .selectFrom('issue_to_label')
    .innerJoin('issue_label', 'issue_label.id', 'issue_to_label.labelId')
    .select([
      'issue_to_label.issueId',
      'issue_label.id as labelId',
      'issue_label.name as labelName',
      'issue_label.color as labelColor'
    ])
    .where('issue_to_label.issueId', 'in', issueIds)
    .execute();

  // Group labels by issueId
  const labelsByIssueId = issueLabels.reduce((acc, label) => {
    if (!acc[label.issueId]) {
      acc[label.issueId] = [];
    }
    acc[label.issueId].push({
      id: label.labelId,
      name: label.labelName,
      color: label.labelColor
    });
    return acc;
  }, {} as Record<string, {id: string, name: string, color: string}[]>);

  // Transform to match front-end expectations
  return issues.map(issue => ({
    id: issue.id,
    identifier: issue.identifier,
    title: issue.title,
    description: issue.description || '',
    status: {
      id: issue.stateId || '',
      name: issue.stateName || '',
      color: issue.stateColor || '',
      type: issue.stateType || '',
    },
    assignees: issue.assigneeId ? {
      id: issue.assigneeId,
      name: issue.assigneeName || '',
      image: issue.assigneeImage || null,
    } : null,
    priority: {
      id: getPriorityKey(issue.priority),
      name: getPriorityName(issue.priority),
      color: getPriorityColor(issue.priority),
    },
    labels: labelsByIssueId[issue.id] || [],
    createdAt: issue.createdAt,
    cycleId: issue.cycleId || '',
    rank: issue.sortOrder.toString(), // Use sortOrder as rank
    project: {
      id: projectId,
      name: '', // We don't need to fetch project details again since we're already filtering by project
      icon: '',
      color: '',
    },
  }));
}

// Get issue by ID
export async function getIssueById(id: string) {
  const db = getDb();
  
  const issue = await db
    .selectFrom('issue')
    .leftJoin('workflow_state', 'workflow_state.id', 'issue.stateId')
    .leftJoin('user as assignee', 'assignee.id', 'issue.assigneeId')
    .leftJoin('user as creator', 'creator.id', 'issue.creatorId')
    .leftJoin('project', 'project.id', 'issue.projectId')
    .leftJoin('team', 'team.id', 'issue.teamId')
    .select([
      'issue.id',
      'issue.title',
      'issue.description',
      'issue.priority',
      'issue.prioritySortOrder',
      'issue.identifier',
      'issue.number',
      'issue.estimate',
      'issue.sortOrder',
      'issue.createdAt',
      'issue.updatedAt',
      'issue.dueDate',
      'issue.parentId',
      'workflow_state.id as stateId',
      'workflow_state.name as stateName',
      'workflow_state.color as stateColor',
      'workflow_state.type as stateType',
      'assignee.id as assigneeId',
      'assignee.name as assigneeName',
      'assignee.email as assigneeEmail',
      'assignee.image as assigneeImage',
      'creator.id as creatorId',
      'creator.name as creatorName',
      'creator.email as creatorEmail',
      'creator.image as creatorImage',
      'project.id as projectId',
      'project.name as projectName',
      'project.icon as projectIcon',
      'project.color as projectColor',
      'team.id as teamId',
      'team.name as teamName',
      'team.key as teamKey',
      'issue.cycleId'
    ])
    .where('issue.id', '=', id)
    .executeTakeFirst();
    
  if (!issue) return null;

  // Get labels for the issue
  const labels = await db
    .selectFrom('issue_to_label')
    .innerJoin('issue_label', 'issue_label.id', 'issue_to_label.labelId')
    .select([
      'issue_label.id',
      'issue_label.name',
      'issue_label.color'
    ])
    .where('issue_to_label.issueId', '=', id)
    .execute();
    
  // Get sub-issues
  const subIssues = await db
    .selectFrom('issue')
    .select(['id'])
    .where('parentId', '=', id)
    .where('archivedAt', 'is', null)
    .execute();
    
  // Format issue for frontend
  return {
    id: issue.id,
    identifier: issue.identifier,
    title: issue.title,
    description: issue.description || '',
    status: {
      id: issue.stateId || '',
      name: issue.stateName || '',
      color: issue.stateColor || '',
      type: issue.stateType || '',
    },
    assignees: issue.assigneeId ? {
      id: issue.assigneeId,
      name: issue.assigneeName || '',
      email: issue.assigneeEmail || '',
      image: issue.assigneeImage || null,
    } : null,
    priority: {
      id: getPriorityKey(issue.priority),
      name: getPriorityName(issue.priority),
      color: getPriorityColor(issue.priority),
    },
    labels: labels,
    createdAt: issue.createdAt,
    updatedAt: issue.updatedAt,
    cycleId: issue.cycleId || '',
    rank: issue.sortOrder.toString(), // Use sortOrder as rank
    project: issue.projectId ? {
      id: issue.projectId,
      name: issue.projectName || '',
      icon: issue.projectIcon || '',
      color: issue.projectColor || '',
    } : undefined,
    team: {
      id: issue.teamId,
      name: issue.teamName || '',
      key: issue.teamKey || '',
    },
    subissues: subIssues.map(sub => sub.id),
    parentId: issue.parentId,
    dueDate: issue.dueDate,
    creator: issue.creatorId ? {
      id: issue.creatorId,
      name: issue.creatorName || '',
      email: issue.creatorEmail || '',
      image: issue.creatorImage || null,
    } : null,
  };
}

// Create a new issue
export async function createIssue(issueData: {
  title: string;
  description?: string;
  teamId: string;
  stateId: string;
  priority?: number;
  assigneeId?: string;
  projectId?: string;
  cycleId?: string;
  parentId?: string;
  creatorId?: string;
  labelIds?: string[];
  estimate?: number;
  dueDate?: string;
}) {
  const db = getDb();
  const id = crypto.randomUUID();
  const now = new Date().toISOString();
  
  // Get the team to generate identifier
  const team = await db
    .selectFrom('team')
    .select(['key'])
    .where('id', '=', issueData.teamId)
    .executeTakeFirst();
    
  if (!team) {
    throw new Error('Team not found');
  }
  
  // Get the next issue number for the team
  const highestNumber = await db
    .selectFrom('issue')
    .select(db.fn.max('number').as('maxNumber'))
    .where('teamId', '=', issueData.teamId)
    .executeTakeFirst();
    
  const nextNumber = (highestNumber?.maxNumber || 0) + 1;
  const identifier = `${team.key}-${nextNumber}`;
  
  // Generate a sortOrder value (use a high value to place at the bottom)
  const highestSortOrder = await db
    .selectFrom('issue')
    .select(db.fn.max('sortOrder').as('maxSortOrder'))
    .executeTakeFirst();
    
  const sortOrder = (highestSortOrder?.maxSortOrder || 0) + 1000; // Leave gaps between issues
  
  // Create the branch name from title
  const branchName = issueData.title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  
  // Insert the issue
  await db
    .insertInto('issue')
    .values({
      id,
      title: issueData.title,
      description: issueData.description || null,
      teamId: issueData.teamId,
      stateId: issueData.stateId,
      priority: issueData.priority || 0,
      number: nextNumber,
      identifier,
      branchName: `${identifier.toLowerCase()}/${branchName}`,
      url: `https://linear.app/issue/${identifier}`, // Placeholder URL
      sortOrder,
      prioritySortOrder: sortOrder,
      assigneeId: issueData.assigneeId || null,
      projectId: issueData.projectId || null,
      cycleId: issueData.cycleId || null,
      parentId: issueData.parentId || null,
      creatorId: issueData.creatorId || null,
      estimate: issueData.estimate || null,
      dueDate: issueData.dueDate || null,
      customerTicketCount: 0,
      createdAt: now,
      updatedAt: now,
      ...(issueData.projectId && { addedToProjectAt: now }),
      addedToTeamAt: now,
    })
    .execute();
    
  // Add labels if provided
  if (issueData.labelIds && issueData.labelIds.length > 0) {
    for (const labelId of issueData.labelIds) {
      await db
        .insertInto('issue_to_label')
        .values({
          id: crypto.randomUUID(),
          issueId: id,
          labelId,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
    }
  }
  
  return id;
}

// Update an existing issue
export async function updateIssue(id: string, issueData: {
  title?: string;
  description?: string;
  stateId?: string;
  priority?: number;
  assigneeId?: string | null;
  projectId?: string | null;
  cycleId?: string | null;
  parentId?: string | null;
  estimate?: number | null;
  dueDate?: string | null;
  labelIds?: string[];
}) {
  const db = getDb();
  const now = new Date().toISOString();
  
  // Prepare update object
  const updateValues: any = {
    updatedAt: now,
  };
  
  // Add only provided fields
  if (issueData.title !== undefined) updateValues.title = issueData.title;
  if (issueData.description !== undefined) updateValues.description = issueData.description;
  if (issueData.stateId !== undefined) updateValues.stateId = issueData.stateId;
  if (issueData.priority !== undefined) updateValues.priority = issueData.priority;
  if (issueData.assigneeId !== undefined) updateValues.assigneeId = issueData.assigneeId;
  if (issueData.projectId !== undefined) {
    updateValues.projectId = issueData.projectId;
    if (issueData.projectId) updateValues.addedToProjectAt = now;
  }
  if (issueData.cycleId !== undefined) {
    updateValues.cycleId = issueData.cycleId;
    if (issueData.cycleId) updateValues.addedToCycleAt = now;
  }
  if (issueData.parentId !== undefined) updateValues.parentId = issueData.parentId;
  if (issueData.estimate !== undefined) updateValues.estimate = issueData.estimate;
  if (issueData.dueDate !== undefined) updateValues.dueDate = issueData.dueDate;
  
  // Update the issue
  await db
    .updateTable('issue')
    .set(updateValues)
    .where('id', '=', id)
    .execute();
    
  // Update labels if provided
  if (issueData.labelIds !== undefined) {
    // First delete existing labels
    await db
      .deleteFrom('issue_to_label')
      .where('issueId', '=', id)
      .execute();
      
    // Then add new labels
    if (issueData.labelIds.length > 0) {
      for (const labelId of issueData.labelIds) {
        await db
          .insertInto('issue_to_label')
          .values({
            id: crypto.randomUUID(),
            issueId: id,
            labelId,
            createdAt: now,
            updatedAt: now,
          })
          .execute();
      }
    }
  }
  
  return id;
}

// Get workflow states for teams
export async function getWorkflowStates(teamId?: string) {
  const db = getDb();
  
  let query = db
    .selectFrom('workflow_state')
    .select(['id', 'name', 'color', 'type', 'position', 'teamId'])
    .where('archivedAt', 'is', null);
    
  if (teamId) {
    query = query.where('teamId', '=', teamId);
  }
  
  return query
    .orderBy(['teamId', 'position'])
    .execute();
}

// Get issue labels (optionally filtered by team)
export async function getIssueLabels(teamId?: string) {
  const db = getDb();
  
  let query = db
    .selectFrom('issue_label')
    .select(['id', 'name', 'color', 'teamId'])
    .where('archivedAt', 'is', null);
    
  if (teamId) {
    query = query.where(eb => 
      eb.or([
        eb('teamId', 'is', null), // Global labels
        eb('teamId', '=', teamId) // Team-specific labels
      ])
    );
  }
  
  return query
    .orderBy('name')
    .execute();
}

// Helper functions for formatting data
function getPriorityKey(priority: any): string {
  switch (priority) {
    case 0: return 'no-priority';
    case 1: return 'urgent';
    case 2: return 'high';
    case 3: return 'medium';
    case 4: return 'low';
    default: return 'no-priority';
  }
}

function getPriorityName(priority: any): string {
  switch (priority) {
    case 0: return 'No priority';
    case 1: return 'Urgent';
    case 2: return 'High';
    case 3: return 'Medium';
    case 4: return 'Low';
    default: return 'No priority';
  }
}

function getPriorityColor(priority: any): string {
  switch (priority) {
    case 0: return '#6B7280'; // Gray
    case 1: return '#EF4444'; // Red
    case 2: return '#F59E0B'; // Amber
    case 3: return '#3B82F6'; // Blue
    case 4: return '#10B981'; // Green
    default: return '#6B7280'; // Gray
  }
}