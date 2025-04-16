import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { env } from 'cloudflare:workers';
import { Database } from './types';
import { getProjectsByTeamId } from './project-helpers.server';

// Initialize the DB connection
export function getDb() {
  return new Kysely<Database>({
    dialect: new D1Dialect({
      database: env.DB as any,
    }),
  });
}

export interface TeamInputData {
  name: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  private?: boolean;
  timezone?: string;
  cyclesEnabled?: boolean;
}

// Team operations
export async function getTeams() {
  const db = getDb();
  
  // Get all teams
  const teams = await db
    .selectFrom('team')
    .select([
      'id',
      'name',
      'key',
      'description',
      'icon',
      'color',
      'private',
      'inviteHash',
      'createdAt',
      'updatedAt'
    ])
    .where('archivedAt', 'is', null)
    .orderBy('name')
    .execute();
  
  // Enhanced team data for frontend
  const enhancedTeams = [];
  
  for (const team of teams) {
    // Get team members count
    const memberCount = await db
      .selectFrom('team_membership')
      .select(({ fn }) => [fn.count('id').as('count')])
      .where('teamId', '=', team.id)
      .executeTakeFirst();
    
    // Get team projects count
    const projectCount = await db
      .selectFrom('team_project')
      .select(({ fn }) => [fn.count('id').as('count')])
      .where('teamId', '=', team.id)
      .executeTakeFirst();
    
    // This will be replaced by proper user session check
    const isJoined = false;
    
    enhancedTeams.push({
      id: team.id,
      name: team.name,
      key: team.key,
      description: team.description,
      icon: team.icon || '👥',
      color: team.color || '#6366F1',
      private: Boolean(team.private),
      joined: isJoined,
      memberCount: parseInt(memberCount?.count as string || '0', 10),
      projectCount: parseInt(projectCount?.count as string || '0', 10),
      createdAt: team.createdAt,
      updatedAt: team.updatedAt
    });
  }
  
  return enhancedTeams;
}

export async function getTeamById(id: string) {
  const db = getDb();
  
  // Get the team
  const team = await db
    .selectFrom('team')
    .select([
      'id',
      'name',
      'key',
      'description',
      'icon',
      'color',
      'private',
      'inviteHash',
      'createdAt',
      'updatedAt'
    ])
    .where('id', '=', id)
    .where('archivedAt', 'is', null)
    .executeTakeFirst();
    
  if (!team) return null;
  
  // Get team members
  const members = await db
    .selectFrom('team_membership')
    .innerJoin('user', 'user.id', 'team_membership.userId')
    .select([
      'user.id',
      'user.name',
      'user.email',
      'user.image',
      'team_membership.owner'
    ])
    .where('team_membership.teamId', '=', id)
    .execute();
  
  // Get team projects
  const projects = await getProjectsByTeamId(id);
  
  return {
    id: team.id,
    name: team.name,
    key: team.key,
    description: team.description,
    icon: team.icon || '👥',
    color: team.color || '#6366F1',
    private: Boolean(team.private),
    inviteHash: team.inviteHash,
    members: members.map(member => ({
      id: member.id,
      name: member.name,
      email: member.email,
      image: member.image,
      role: member.owner ? 'owner' : 'member',
    })),
    projects,
    createdAt: team.createdAt,
    updatedAt: team.updatedAt
  };
}

export async function getTeamsForUser(userId: string) {
  const db = getDb();
  
  // Get all teams that the user is a member of
  const teamIds = await db
    .selectFrom('team_membership')
    .select(['teamId'])
    .where('userId', '=', userId)
    .execute();
  
  if (teamIds.length === 0) return [];
  
  // Get enhanced team details with counts
  const enhancedTeams = [];
  
  for (const { teamId } of teamIds) {
    const team = await db
      .selectFrom('team')
      .select([
        'id',
        'name',
        'key',
        'description',
        'icon',
        'color',
        'private',
        'createdAt',
        'updatedAt'
      ])
      .where('id', '=', teamId)
      .where('archivedAt', 'is', null)
      .executeTakeFirst();
      
    if (team) {
      // Get team members count
      const memberCount = await db
        .selectFrom('team_membership')
        .select(({ fn }) => [fn.count('id').as('count')])
        .where('teamId', '=', teamId)
        .executeTakeFirst();
      
      // Get team projects count
      const projectCount = await db
        .selectFrom('team_project')
        .select(({ fn }) => [fn.count('id').as('count')])
        .where('teamId', '=', teamId)
        .executeTakeFirst();
        
      enhancedTeams.push({
        id: team.id,
        name: team.name,
        key: team.key,
        description: team.description,
        icon: team.icon || '👥',
        color: team.color || '#6366F1',
        private: Boolean(team.private),
        joined: true, // User is a member of this team
        memberCount: parseInt(memberCount?.count as string || '0', 10),
        projectCount: parseInt(projectCount?.count as string || '0', 10),
        createdAt: team.createdAt,
        updatedAt: team.updatedAt
      });
    }
  }
  
  return enhancedTeams;
}

export async function createTeam(teamData: TeamInputData, creatorId: string) {
  const db = getDb();
  const id = crypto.randomUUID();
  const now = new Date().toISOString();
  
  // Generate a key based on the team name
  let key = teamData.name
    .toUpperCase()
    .replace(/[^A-Z0-9]/g, '')
    .substring(0, 5);
    
  // Check if the key already exists
  const existingTeam = await db
    .selectFrom('team')
    .select(['id'])
    .where('key', '=', key)
    .executeTakeFirst();
    
  if (existingTeam) {
    // Append a random string to make it unique
    key = `${key}-${id.substring(0, 4)}`;
  }
  
  // Create team
  await db
    .insertInto('team')
    .values({
      id,
      name: teamData.name,
      key,
      description: teamData.description || '',
      icon: teamData.icon || '👥',
      color: teamData.color || '#6366F1',
      private: teamData.private ? 1 : 0,
      timezone: teamData.timezone || 'America/Los_Angeles',
      inviteHash: crypto.randomUUID(),
      cyclesEnabled: teamData.cyclesEnabled ? 1 : 0,
      cycleDuration: null,
      cycleCooldownTime: null,
      cycleStartDay: null,
      upcomingCycleCount: 0,
      autoArchivePeriod: 0,
      issueEstimationType: 'notUsed',
      issueEstimationAllowZero: 0,
      issueEstimationExtended: 0,
      defaultIssueEstimate: 0,
      triageEnabled: 0,
      requirePriorityToLeaveTriage: 0,
      groupIssueHistory: 1,
      setIssueSortOrderOnStateChange: 'bottom',
      inheritIssueEstimation: 1,
      inheritWorkflowStatuses: 1,
      scimManaged: 0,
      createdAt: now,
      updatedAt: now,
    })
    .execute();
  
  // Add the creator as an owner if provided
  if (creatorId) {
    await db
      .insertInto('team_membership')
      .values({
        id: crypto.randomUUID(),
        teamId: id,
        userId: creatorId,
        owner: 1,
        sortOrder: 0,
        createdAt: now,
        updatedAt: now,
      })
      .execute();
  }
  
  return id;
}

export async function addMemberToTeam(teamId: string, userId: string, isOwner: boolean = false) {
  const db = getDb();
  const id = crypto.randomUUID();
  const now = new Date().toISOString();
  
  await db
    .insertInto('team_membership')
    .values({
      id,
      teamId,
      userId,
      owner: isOwner ? 1 : 0,
      sortOrder: 0,
      createdAt: now,
      updatedAt: now,
    })
    .execute();
    
  return id;
}