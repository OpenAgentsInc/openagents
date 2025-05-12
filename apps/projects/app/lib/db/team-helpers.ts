import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { env } from 'cloudflare:workers';
import type { Database } from './types';
import { getProjectsByTeamId } from './project-helpers';

// Initialize the DB connection
export function getDb() {
  return new Kysely<Database>({
    dialect: new D1Dialect({
      database: env.DB as any,
    }),
  });
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

    // Check if current user is a member of this team
    // (In a real app, you'd check the current user's session)
    // This is a placeholder for demonstration
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

export async function getTeamsByUserId(userId: string) {
  const db = getDb();

  // Get all teams that the user is a member of
  const teamIds = await db
    .selectFrom('team_membership')
    .select(['teamId'])
    .where('userId', '=', userId)
    .execute();

  if (teamIds.length === 0) return [];

  // Get full team details
  const teams = [];
  for (const { teamId } of teamIds) {
    const team = await getTeamById(teamId);
    if (team) teams.push(team);
  }

  return teams;
}

export async function createTeam(teamData: any) {
  const db = getDb();
  const id = crypto.randomUUID();
  const now = new Date().toISOString();

  // Generate a key based on the team name
  const key = teamData.name
    .toUpperCase()
    .replace(/[^A-Z0-9]/g, '')
    .substring(0, 5);

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
      cycleDuration: teamData.cycleDuration || null,
      cycleCooldownTime: teamData.cycleCooldownTime || null,
      cycleStartDay: teamData.cycleStartDay || null,
      createdAt: now,
      updatedAt: now,
    })
    .execute();

  // Add the creator as an owner
  if (teamData.creatorId) {
    await db
      .insertInto('team_membership')
      .values({
        id: crypto.randomUUID(),
        teamId: id,
        userId: teamData.creatorId,
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
