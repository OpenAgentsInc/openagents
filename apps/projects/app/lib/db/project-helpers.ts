import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { env } from 'cloudflare:workers';
import type { Database } from './types';

// Initialize the DB connection
export function getDb() {
  return new Kysely<Database>({
    dialect: new D1Dialect({
      database: env.DB as any,
    }),
  });
}

// Project operations
export async function getProjects() {
  const db = getDb();

  // Get all projects with related information
  const projects = await db
    .selectFrom('project')
    .leftJoin('user', 'user.id', 'project.leadId')
    .leftJoin('project_status', 'project_status.id', 'project.statusId')
    .select([
      'project.id',
      'project.name',
      'project.description',
      'project.icon',
      'project.color',
      'project.slugId',
      'project.priority',
      'project.health',
      'project.progress as percentComplete',
      'project.startDate',
      'project.targetDate',
      'project_status.name as statusName',
      'project_status.color as statusColor',
      'project_status.type as statusType',
      'user.id as leadId',
      'user.name as leadName',
      'user.image as leadImage',
      'project.creatorId',
      'project.createdAt',
      'project.updatedAt'
    ])
    .where('project.archivedAt', 'is', null)
    .orderBy('project.createdAt', 'desc')
    .execute();

  // Transform to match front-end expectations
  return projects.map(project => ({
    id: project.id,
    name: project.name,
    description: project.description,
    icon: project.icon,
    color: project.color,
    slugId: project.slugId,
    percentComplete: parseFloat(project.percentComplete as string) * 100,
    startDate: project.startDate,
    targetDate: project.targetDate,
    priority: {
      id: project.priority ? project.priority.toString() : '0',
      name: getPriorityName(project.priority),
      color: getPriorityColor(project.priority),
    },
    status: {
      id: project.statusType || 'backlog',
      name: project.statusName || 'Backlog',
      color: project.statusColor || '#808080',
    },
    health: {
      id: project.health || 'on-track',
      name: getHealthName(project.health),
      color: getHealthColor(project.health),
      description: getHealthDescription(project.health),
    },
    lead: project.leadId ? {
      id: project.leadId,
      name: project.leadName || '',
      image: project.leadImage || null,
    } : null,
    createdAt: project.createdAt,
    updatedAt: project.updatedAt,
  }));
}

export async function getProjectById(id: string) {
  const db = getDb();

  // Get the project with related information
  const project = await db
    .selectFrom('project')
    .leftJoin('user as lead', 'lead.id', 'project.leadId')
    .leftJoin('user as creator', 'creator.id', 'project.creatorId')
    .leftJoin('project_status', 'project_status.id', 'project.statusId')
    .select([
      'project.id',
      'project.name',
      'project.description',
      'project.icon',
      'project.color',
      'project.slugId',
      'project.priority',
      'project.health',
      'project.progress as percentComplete',
      'project.startDate',
      'project.targetDate',
      'project.content',
      'project_status.id as statusId',
      'project_status.name as statusName',
      'project_status.color as statusColor',
      'project_status.type as statusType',
      'lead.id as leadId',
      'lead.name as leadName',
      'lead.email as leadEmail',
      'lead.image as leadImage',
      'creator.id as creatorId',
      'creator.name as creatorName',
      'creator.email as creatorEmail',
      'creator.image as creatorImage',
      'project.createdAt',
      'project.updatedAt'
    ])
    .where('project.id', '=', id)
    .executeTakeFirst();

  if (!project) return null;

  // Get project members
  const members = await db
    .selectFrom('project_member')
    .innerJoin('user', 'user.id', 'project_member.userId')
    .select([
      'user.id',
      'user.name',
      'user.email',
      'user.image'
    ])
    .where('project_member.projectId', '=', id)
    .execute();

  // Get project teams
  const teams = await db
    .selectFrom('team_project')
    .innerJoin('team', 'team.id', 'team_project.teamId')
    .select([
      'team.id',
      'team.name',
      'team.icon',
      'team.color',
      'team.key'
    ])
    .where('team_project.projectId', '=', id)
    .execute();

  // Get project milestones if they exist
  const milestones = await db
    .selectFrom('project_milestone')
    .select([
      'id',
      'name',
      'description',
      'targetDate',
      'sortOrder',
      'createdAt',
      'updatedAt'
    ])
    .where('projectId', '=', id)
    .orderBy('sortOrder')
    .execute();

  // Get issues associated with this project
  const issues = await db
    .selectFrom('issue')
    .leftJoin('user as assignee', 'assignee.id', 'issue.assigneeId')
    .leftJoin('workflow_state', 'workflow_state.id', 'issue.stateId')
    .select([
      'issue.id',
      'issue.title',
      'issue.identifier',
      'issue.priority',
      'issue.estimate',
      'workflow_state.name as stateName',
      'workflow_state.type as stateType',
      'workflow_state.color as stateColor',
      'assignee.id as assigneeId',
      'assignee.name as assigneeName',
      'assignee.image as assigneeImage',
      'issue.dueDate',
      'issue.createdAt',
      'issue.updatedAt'
    ])
    .where('issue.projectId', '=', id)
    .where('issue.archivedAt', 'is', null)
    .orderBy('issue.createdAt', 'desc')
    .execute();

  // Format project for frontend
  return {
    id: project.id,
    name: project.name,
    description: project.description,
    icon: project.icon,
    color: project.color,
    slugId: project.slugId,
    percentComplete: parseFloat(project.percentComplete as string) * 100,
    startDate: project.startDate,
    targetDate: project.targetDate,
    content: project.content,
    priority: {
      id: project.priority ? project.priority.toString() : '0',
      name: getPriorityName(project.priority),
      color: getPriorityColor(project.priority),
    },
    status: {
      id: project.statusId || '',
      name: project.statusName || 'Backlog',
      color: project.statusColor || '#808080',
      type: project.statusType || 'backlog',
    },
    health: {
      id: project.health || 'on-track',
      name: getHealthName(project.health),
      color: getHealthColor(project.health),
      description: getHealthDescription(project.health),
    },
    lead: project.leadId ? {
      id: project.leadId,
      name: project.leadName || '',
      email: project.leadEmail || '',
      image: project.leadImage || null,
    } : null,
    creator: project.creatorId ? {
      id: project.creatorId,
      name: project.creatorName || '',
      email: project.creatorEmail || '',
      image: project.creatorImage || null,
    } : null,
    members,
    teams,
    milestones,
    issues: issues.map(issue => ({
      id: issue.id,
      title: issue.title,
      identifier: issue.identifier,
      priority: {
        id: issue.priority ? issue.priority.toString() : '0',
        name: getPriorityName(issue.priority),
        color: getPriorityColor(issue.priority),
      },
      estimate: issue.estimate,
      state: {
        name: issue.stateName || '',
        type: issue.stateType || '',
        color: issue.stateColor || '',
      },
      assignee: issue.assigneeId ? {
        id: issue.assigneeId,
        name: issue.assigneeName || '',
        image: issue.assigneeImage || null,
      } : null,
      dueDate: issue.dueDate,
      createdAt: issue.createdAt,
      updatedAt: issue.updatedAt,
    })),
    createdAt: project.createdAt,
    updatedAt: project.updatedAt,
  };
}

export async function getProjectsByTeamId(teamId: string) {
  const db = getDb();

  // Get all projects associated with the team
  const projects = await db
    .selectFrom('team_project')
    .innerJoin('project', 'project.id', 'team_project.projectId')
    .leftJoin('user', 'user.id', 'project.leadId')
    .leftJoin('project_status', 'project_status.id', 'project.statusId')
    .select([
      'project.id',
      'project.name',
      'project.description',
      'project.icon',
      'project.color',
      'project.slugId',
      'project.priority',
      'project.health',
      'project.progress as percentComplete',
      'project.startDate',
      'project.targetDate',
      'project_status.name as statusName',
      'project_status.color as statusColor',
      'project_status.type as statusType',
      'user.id as leadId',
      'user.name as leadName',
      'user.image as leadImage',
      'project.createdAt',
      'project.updatedAt'
    ])
    .where('team_project.teamId', '=', teamId)
    .where('project.archivedAt', 'is', null)
    .orderBy('project.createdAt', 'desc')
    .execute();

  // Transform to match front-end expectations
  return projects.map(project => ({
    id: project.id,
    name: project.name,
    description: project.description,
    icon: project.icon,
    color: project.color,
    slugId: project.slugId,
    percentComplete: parseFloat(project.percentComplete as string) * 100,
    startDate: project.startDate,
    targetDate: project.targetDate,
    priority: {
      id: project.priority ? project.priority.toString() : '0',
      name: getPriorityName(project.priority),
      color: getPriorityColor(project.priority),
    },
    status: {
      id: project.statusType || 'backlog',
      name: project.statusName || 'Backlog',
      color: project.statusColor || '#808080',
    },
    health: {
      id: project.health || 'on-track',
      name: getHealthName(project.health),
      color: getHealthColor(project.health),
      description: getHealthDescription(project.health),
    },
    lead: project.leadId ? {
      id: project.leadId,
      name: project.leadName || '',
      image: project.leadImage || null,
    } : null,
    createdAt: project.createdAt,
    updatedAt: project.updatedAt,
  }));
}

export async function createProject(projectData: any) {
  const db = getDb();
  const id = crypto.randomUUID();
  const now = new Date().toISOString();
  const slugId = projectData.name.toLowerCase().replace(/[^a-z0-9]+/g, '-');

  await db
    .insertInto('project')
    .values({
      id,
      name: projectData.name,
      description: projectData.description || '',
      icon: projectData.icon || '📋',
      color: projectData.color || '#6366F1',
      slugId: `${slugId}-${id.substring(0, 8)}`,
      sortOrder: 0,
      priority: projectData.priority || 0,
      prioritySortOrder: 0,
      health: projectData.health || 'onTrack',
      progress: projectData.percentComplete ? projectData.percentComplete / 100 : 0,
      scope: 0,
      startDate: projectData.startDate || null,
      targetDate: projectData.targetDate || null,
      content: projectData.content || '',
      creatorId: projectData.creatorId,
      leadId: projectData.leadId || null,
      statusId: projectData.statusId,
      createdAt: now,
      updatedAt: now,
    })
    .execute();

  // Add team associations if provided
  if (projectData.teamIds && projectData.teamIds.length > 0) {
    for (const teamId of projectData.teamIds) {
      await db
        .insertInto('team_project')
        .values({
          id: crypto.randomUUID(),
          teamId,
          projectId: id,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
    }
  }

  // Add member associations if provided
  if (projectData.memberIds && projectData.memberIds.length > 0) {
    for (const userId of projectData.memberIds) {
      await db
        .insertInto('project_member')
        .values({
          id: crypto.randomUUID(),
          projectId: id,
          userId,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
    }
  }

  return id;
}

// Helper functions for formatting data
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

function getHealthName(health: any): string {
  switch (health) {
    case 'onTrack': return 'On Track';
    case 'atRisk': return 'At Risk';
    case 'offTrack': return 'Off Track';
    default: return 'On Track';
  }
}

function getHealthColor(health: any): string {
  switch (health) {
    case 'onTrack': return '#10B981'; // Green
    case 'atRisk': return '#F59E0B'; // Amber
    case 'offTrack': return '#EF4444'; // Red
    default: return '#10B981'; // Green
  }
}

function getHealthDescription(health: any): string {
  switch (health) {
    case 'onTrack': return 'The project is proceeding as planned.';
    case 'atRisk': return 'The project has some issues that need attention.';
    case 'offTrack': return 'The project is experiencing significant problems.';
    default: return 'The project is proceeding as planned.';
  }
}
