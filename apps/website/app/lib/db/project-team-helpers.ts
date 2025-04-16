import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { env } from 'cloudflare:workers';
import { 
  Project, 
  Team, 
  ProjectMember, 
  TeamMember, 
  TeamProject, 
  User, 
  ProjectWithRelations, 
  TeamWithRelations,
  UserWithRelations
} from '../types/db-schema';

// Define the database interface for Kysely
interface Database {
  user: User;
  project: Project;
  team: Team;
  project_member: ProjectMember;
  team_member: TeamMember;
  team_project: TeamProject;
}

// Initialize the DB connection
export function getDb() {
  return new Kysely<Database>({
    dialect: new D1Dialect({
      database: env.DB,
    }),
  });
}

// Project operations
export async function getProjects() {
  const db = getDb();
  return db.selectFrom('project').selectAll().execute();
}

export async function getProjectById(id: string): Promise<ProjectWithRelations | undefined> {
  const db = getDb();
  
  // Get the project
  const project = await db
    .selectFrom('project')
    .where('id', '=', id)
    .selectAll()
    .executeTakeFirst();
    
  if (!project) return undefined;
  
  // Get the owner
  const owner = await db
    .selectFrom('user')
    .where('id', '=', project.ownerId)
    .selectAll()
    .executeTakeFirst();
    
  if (!owner) return undefined;
  
  // Get the members with their roles
  const projectMembers = await db
    .selectFrom('project_member')
    .where('projectId', '=', id)
    .innerJoin('user', 'user.id', 'project_member.userId')
    .select([
      'user.id',
      'user.name',
      'user.email',
      'user.image',
      'project_member.role'
    ])
    .execute();
    
  // Get the teams
  const teamProjects = await db
    .selectFrom('team_project')
    .where('projectId', '=', id)
    .innerJoin('team', 'team.id', 'team_project.teamId')
    .selectAll('team')
    .execute();
    
  return {
    ...project,
    owner,
    members: projectMembers.map(member => ({
      id: member.id,
      name: member.name,
      email: member.email,
      image: member.image,
      role: member.role,
      emailVerified: 0, // Default values for compatibility
      createdAt: new Date(),
      updatedAt: new Date()
    })),
    teams: teamProjects
  };
}

export async function createProject(project: Omit<Project, 'id' | 'createdAt' | 'updatedAt'>) {
  const db = getDb();
  const id = crypto.randomUUID();
  
  await db
    .insertInto('project')
    .values({
      ...project,
      id,
      createdAt: new Date(),
      updatedAt: new Date()
    })
    .execute();
    
  return id;
}

// Team operations
export async function getTeams() {
  const db = getDb();
  return db.selectFrom('team').selectAll().execute();
}

export async function getTeamById(id: string): Promise<TeamWithRelations | undefined> {
  const db = getDb();
  
  // Get the team
  const team = await db
    .selectFrom('team')
    .where('id', '=', id)
    .selectAll()
    .executeTakeFirst();
    
  if (!team) return undefined;
  
  // Get the owner
  const owner = await db
    .selectFrom('user')
    .where('id', '=', team.ownerId)
    .selectAll()
    .executeTakeFirst();
    
  if (!owner) return undefined;
  
  // Get the members with their roles
  const teamMembers = await db
    .selectFrom('team_member')
    .where('teamId', '=', id)
    .innerJoin('user', 'user.id', 'team_member.userId')
    .select([
      'user.id',
      'user.name',
      'user.email',
      'user.image',
      'team_member.role'
    ])
    .execute();
    
  // Get the projects
  const teamProjects = await db
    .selectFrom('team_project')
    .where('teamId', '=', id)
    .innerJoin('project', 'project.id', 'team_project.projectId')
    .selectAll('project')
    .execute();
    
  return {
    ...team,
    owner,
    members: teamMembers.map(member => ({
      id: member.id,
      name: member.name,
      email: member.email,
      image: member.image,
      role: member.role,
      emailVerified: 0, // Default values for compatibility
      createdAt: new Date(),
      updatedAt: new Date()
    })),
    projects: teamProjects
  };
}

export async function createTeam(team: Omit<Team, 'id' | 'createdAt' | 'updatedAt'>) {
  const db = getDb();
  const id = crypto.randomUUID();
  
  await db
    .insertInto('team')
    .values({
      ...team,
      id,
      createdAt: new Date(),
      updatedAt: new Date()
    })
    .execute();
    
  return id;
}

// Relationship operations
export async function addUserToProject(projectId: string, userId: string, role: string = 'member') {
  const db = getDb();
  const id = crypto.randomUUID();
  
  await db
    .insertInto('project_member')
    .values({
      id,
      projectId,
      userId,
      role,
      createdAt: new Date(),
      updatedAt: new Date()
    })
    .execute();
    
  return id;
}

export async function addUserToTeam(teamId: string, userId: string, role: string = 'member') {
  const db = getDb();
  const id = crypto.randomUUID();
  
  await db
    .insertInto('team_member')
    .values({
      id,
      teamId,
      userId,
      role,
      createdAt: new Date(),
      updatedAt: new Date()
    })
    .execute();
    
  return id;
}

export async function addProjectToTeam(teamId: string, projectId: string) {
  const db = getDb();
  const id = crypto.randomUUID();
  
  await db
    .insertInto('team_project')
    .values({
      id,
      teamId,
      projectId,
      createdAt: new Date(),
      updatedAt: new Date()
    })
    .execute();
    
  return id;
}

// User-focused queries
export async function getUserWithRelations(userId: string): Promise<UserWithRelations | undefined> {
  const db = getDb();
  
  // Get the user
  const user = await db
    .selectFrom('user')
    .where('id', '=', userId)
    .selectAll()
    .executeTakeFirst();
    
  if (!user) return undefined;
  
  // Get owned projects
  const ownedProjects = await db
    .selectFrom('project')
    .where('ownerId', '=', userId)
    .selectAll()
    .execute();
    
  // Get owned teams
  const ownedTeams = await db
    .selectFrom('team')
    .where('ownerId', '=', userId)
    .selectAll()
    .execute();
    
  // Get projects the user is a member of
  const memberProjects = await db
    .selectFrom('project_member')
    .where('userId', '=', userId)
    .innerJoin('project', 'project.id', 'project_member.projectId')
    .select(eb => [
      ...eb.selectAll('project'),
      'project_member.role'
    ])
    .execute();
    
  // Get teams the user is a member of
  const memberTeams = await db
    .selectFrom('team_member')
    .where('userId', '=', userId)
    .innerJoin('team', 'team.id', 'team_member.teamId')
    .select(eb => [
      ...eb.selectAll('team'),
      'team_member.role'
    ])
    .execute();
    
  return {
    ...user,
    ownedProjects,
    ownedTeams,
    memberProjects,
    memberTeams
  };
}