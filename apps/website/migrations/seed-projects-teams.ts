import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { Database } from '../app/lib/db/types';

// This is a seed script that can be run to populate the database
// Example usage: npx tsx seed-projects-teams.ts

// Mock data from the mock-data directory would be imported and used here
// For now, we'll just define some sample data

const sampleUsers = [
  { id: 'user1', name: 'Alice Smith', email: 'alice@example.com' },
  { id: 'user2', name: 'Bob Johnson', email: 'bob@example.com' },
  { id: 'user3', name: 'Carol Williams', email: 'carol@example.com' },
];

const sampleProjects = [
  { 
    id: 'proj1', 
    name: 'Website Redesign', 
    status: 'in_progress', 
    icon: '🎨', 
    percentComplete: 60, 
    startDate: '2025-04-01', 
    priority: 'high', 
    health: 'on_track', 
    ownerId: 'user1' 
  },
  { 
    id: 'proj2', 
    name: 'Mobile App Development', 
    status: 'planning', 
    icon: '📱', 
    percentComplete: 20, 
    startDate: '2025-03-15', 
    priority: 'medium', 
    health: 'at_risk', 
    ownerId: 'user2' 
  },
  { 
    id: 'proj3', 
    name: 'Data Migration', 
    status: 'completed', 
    icon: '📊', 
    percentComplete: 100, 
    startDate: '2025-02-10', 
    priority: 'high', 
    health: 'on_track', 
    ownerId: 'user3' 
  },
];

const sampleTeams = [
  { 
    id: 'team1', 
    name: 'Design Team', 
    icon: '🎨', 
    color: '#FF5733', 
    ownerId: 'user1' 
  },
  { 
    id: 'team2', 
    name: 'Development Team', 
    icon: '👨‍💻', 
    color: '#33A1FF', 
    ownerId: 'user2' 
  },
  { 
    id: 'team3', 
    name: 'Data Science Team', 
    icon: '📊', 
    color: '#33FF57', 
    ownerId: 'user3' 
  },
];

// Relationships
const projectMembers = [
  { id: 'pm1', projectId: 'proj1', userId: 'user1', role: 'owner' },
  { id: 'pm2', projectId: 'proj1', userId: 'user2', role: 'member' },
  { id: 'pm3', projectId: 'proj2', userId: 'user2', role: 'owner' },
  { id: 'pm4', projectId: 'proj2', userId: 'user3', role: 'admin' },
  { id: 'pm5', projectId: 'proj3', userId: 'user3', role: 'owner' },
  { id: 'pm6', projectId: 'proj3', userId: 'user1', role: 'member' },
];

const teamMembers = [
  { id: 'tm1', teamId: 'team1', userId: 'user1', role: 'owner' },
  { id: 'tm2', teamId: 'team1', userId: 'user2', role: 'member' },
  { id: 'tm3', teamId: 'team2', userId: 'user2', role: 'owner' },
  { id: 'tm4', teamId: 'team2', userId: 'user3', role: 'admin' },
  { id: 'tm5', teamId: 'team3', userId: 'user3', role: 'owner' },
  { id: 'tm6', teamId: 'team3', userId: 'user1', role: 'member' },
];

const teamProjects = [
  { id: 'tp1', teamId: 'team1', projectId: 'proj1' },
  { id: 'tp2', teamId: 'team2', projectId: 'proj1' },
  { id: 'tp3', teamId: 'team2', projectId: 'proj2' },
  { id: 'tp4', teamId: 'team3', projectId: 'proj3' },
];

async function seed(db: Kysely<Database>) {
  const now = new Date().toISOString();

  // Insert users if they don't exist
  for (const user of sampleUsers) {
    try {
      await db.insertInto('user')
        .values({
          ...user,
          emailVerified: 1,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`User ${user.email} already exists`);
    }
  }

  // Insert projects
  for (const project of sampleProjects) {
    try {
      await db.insertInto('project')
        .values({
          ...project,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`Project ${project.name} already exists`);
    }
  }

  // Insert teams
  for (const team of sampleTeams) {
    try {
      await db.insertInto('team')
        .values({
          ...team,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`Team ${team.name} already exists`);
    }
  }

  // Insert project members
  for (const member of projectMembers) {
    try {
      await db.insertInto('project_member')
        .values({
          ...member,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`Project member ${member.id} already exists`);
    }
  }

  // Insert team members
  for (const member of teamMembers) {
    try {
      await db.insertInto('team_member')
        .values({
          ...member,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`Team member ${member.id} already exists`);
    }
  }

  // Insert team projects
  for (const tp of teamProjects) {
    try {
      await db.insertInto('team_project')
        .values({
          ...tp,
          createdAt: now,
          updatedAt: now,
        })
        .executeTakeFirstOrThrow();
    } catch (error) {
      console.log(`Team project ${tp.id} already exists`);
    }
  }

  console.log('Seed completed successfully');
}

// This function would be called with the actual D1 database
// when running the script with the Wrangler CLI
async function main() {
  // For local development, you would get the D1 instance
  // const db = new Kysely<Database>({
  //   dialect: new D1Dialect({ database: process.env.D1_DATABASE }),
  // });
  
  // await seed(db);
  console.log('To run this seed, add the D1 database connection code and run with Wrangler');
}

main().catch(console.error);