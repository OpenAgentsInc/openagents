import { D1Dialect } from 'kysely-d1';
import { Kysely } from 'kysely';
import { Database } from '../app/lib/db/types';

// This script seeds the database with initial data based on the Linear API model

async function seed(db: Kysely<Database>) {
  const now = new Date().toISOString();
  
  // Create a test user if it doesn't exist
  const testUserId = crypto.randomUUID();
  try {
    await db
      .insertInto('user')
      .values({
        id: testUserId,
        name: 'Test User',
        email: 'test@example.com',
        emailVerified: 1,
        image: null,
        createdAt: now,
        updatedAt: now,
      })
      .execute();
    console.log('Created test user');
  } catch (error) {
    console.log('Test user already exists, using existing user');
    const existingUser = await db
      .selectFrom('user')
      .selectAll()
      .executeTakeFirst();
    if (existingUser) {
      testUserId = existingUser.id;
    }
  }

  // Project Statuses
  const statusIds = {
    backlog: crypto.randomUUID(),
    planned: crypto.randomUUID(),
    started: crypto.randomUUID(),
    paused: crypto.randomUUID(),
    completed: crypto.randomUUID(),
    canceled: crypto.randomUUID(),
  };

  for (const [type, id] of Object.entries(statusIds)) {
    try {
      await db
        .insertInto('project_status')
        .values({
          id,
          name: type.charAt(0).toUpperCase() + type.slice(1),
          description: `Projects in ${type} state`,
          color: getStatusColor(type),
          type,
          position: getStatusPosition(type),
          indefinite: type === 'backlog' || type === 'planned' ? 1 : 0,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
      console.log(`Created project status: ${type}`);
    } catch (error) {
      console.log(`Project status ${type} already exists or error:`, error);
    }
  }

  // Create teams
  const teamIds = {
    core: crypto.randomUUID(),
    design: crypto.randomUUID(),
    frontend: crypto.randomUUID(),
    backend: crypto.randomUUID(),
  };

  const teamData = [
    {
      id: teamIds.core,
      name: 'Core Team',
      key: 'CORE',
      description: 'Core platform development team',
      icon: '🛠️',
      color: '#FF5733',
    },
    {
      id: teamIds.design,
      name: 'Design Team',
      key: 'DESIGN',
      description: 'UI/UX design team',
      icon: '🎨',
      color: '#33A1FF',
    },
    {
      id: teamIds.frontend,
      name: 'Frontend Team',
      key: 'FE',
      description: 'Frontend development team',
      icon: '💻',
      color: '#33FF57',
    },
    {
      id: teamIds.backend,
      name: 'Backend Team',
      key: 'BE',
      description: 'Backend development team',
      icon: '⚙️',
      color: '#FF33A1',
    },
  ];

  for (const team of teamData) {
    try {
      await db
        .insertInto('team')
        .values({
          id: team.id,
          name: team.name,
          key: team.key,
          description: team.description,
          icon: team.icon,
          color: team.color,
          private: 0,
          timezone: 'America/Los_Angeles',
          inviteHash: crypto.randomUUID(),
          cyclesEnabled: 1,
          cycleDuration: 2,
          cycleCooldownTime: 1,
          cycleStartDay: 1,
          upcomingCycleCount: 1,
          autoArchivePeriod: 3,
          issueEstimationType: 'fibonacci',
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
      console.log(`Created team: ${team.name}`);

      // Add the test user as team owner
      await db
        .insertInto('team_membership')
        .values({
          id: crypto.randomUUID(),
          teamId: team.id,
          userId: testUserId,
          owner: 1,
          sortOrder: 0,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
    } catch (error) {
      console.log(`Team ${team.name} already exists or error:`, error);
    }
  }

  // Create workflow states for teams
  const stateTypes = ['triage', 'backlog', 'unstarted', 'started', 'completed', 'canceled'];
  const stateColors = {
    triage: '#9B59B6',
    backlog: '#95A5A6',
    unstarted: '#3498DB',
    started: '#F1C40F',
    completed: '#2ECC71',
    canceled: '#E74C3C',
  };

  for (const teamId of Object.values(teamIds)) {
    for (let i = 0; i < stateTypes.length; i++) {
      const type = stateTypes[i];
      try {
        await db
          .insertInto('workflow_state')
          .values({
            id: crypto.randomUUID(),
            name: type.charAt(0).toUpperCase() + type.slice(1),
            description: `Issues in ${type} state`,
            color: stateColors[type as keyof typeof stateColors],
            type,
            position: i,
            teamId,
            createdAt: now,
            updatedAt: now,
          })
          .execute();
      } catch (error) {
        console.log(`Workflow state ${type} for team already exists or error:`, error);
      }
    }
    console.log(`Created workflow states for team ID: ${teamId}`);
  }

  // Create projects
  const projects = [
    {
      name: 'Website Redesign',
      description: 'Redesign the company website with new branding',
      icon: 'Palette',
      teamId: teamIds.design,
      statusId: statusIds.started,
      progress: 0.6,
      priority: 2,
    },
    {
      name: 'Mobile App Development',
      description: 'Create a native mobile app for iOS and Android',
      icon: 'Smartphone',
      teamId: teamIds.frontend,
      statusId: statusIds.planned,
      progress: 0.1,
      priority: 1,
    },
    {
      name: 'API Refactoring',
      description: 'Refactor API endpoints for better performance',
      icon: 'Code',
      teamId: teamIds.backend,
      statusId: statusIds.backlog,
      progress: 0,
      priority: 3,
    },
    {
      name: 'Authentication System',
      description: 'Implement OAuth 2.0 and OIDC authentication',
      icon: 'Lock',
      teamId: teamIds.core,
      statusId: statusIds.completed,
      progress: 1.0,
      priority: 0,
    },
  ];

  for (const project of projects) {
    const projectId = crypto.randomUUID();
    try {
      // Create the project
      await db
        .insertInto('project')
        .values({
          id: projectId,
          name: project.name,
          description: project.description,
          icon: project.icon,
          color: getRandomColor(),
          slugId: project.name.toLowerCase().replace(/[^a-z0-9]+/g, '-') + '-' + projectId.substring(0, 8),
          sortOrder: Math.random() * 1000,
          priority: project.priority,
          prioritySortOrder: Math.random() * 1000,
          health: getRandomHealth(),
          progress: project.progress,
          scope: 0,
          startDate: new Date(Date.now() - Math.random() * 30 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
          targetDate: new Date(Date.now() + Math.random() * 60 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
          creatorId: testUserId,
          leadId: testUserId,
          statusId: project.statusId,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
      console.log(`Created project: ${project.name}`);

      // Associate project with team
      await db
        .insertInto('team_project')
        .values({
          id: crypto.randomUUID(),
          teamId: project.teamId,
          projectId,
          createdAt: now,
          updatedAt: now,
        })
        .execute();

      // Add the test user as project member
      await db
        .insertInto('project_member')
        .values({
          id: crypto.randomUUID(),
          projectId,
          userId: testUserId,
          createdAt: now,
          updatedAt: now,
        })
        .execute();
    } catch (error) {
      console.log(`Project ${project.name} already exists or error:`, error);
    }
  }

  console.log('Seed completed successfully');
}

// Helper functions
function getStatusColor(type: string): string {
  switch (type) {
    case 'backlog': return '#95A5A6';
    case 'planned': return '#3498DB';
    case 'started': return '#F1C40F';
    case 'paused': return '#E67E22';
    case 'completed': return '#2ECC71';
    case 'canceled': return '#E74C3C';
    default: return '#95A5A6';
  }
}

function getStatusPosition(type: string): number {
  switch (type) {
    case 'backlog': return 0;
    case 'planned': return 1;
    case 'started': return 2;
    case 'paused': return 3;
    case 'completed': return 4;
    case 'canceled': return 5;
    default: return 0;
  }
}

function getRandomHealth(): string {
  const healths = ['onTrack', 'atRisk', 'offTrack'];
  return healths[Math.floor(Math.random() * healths.length)];
}

function getRandomColor(): string {
  const colors = [
    '#6366F1', // Indigo
    '#3B82F6', // Blue
    '#10B981', // Emerald
    '#F59E0B', // Amber
    '#EF4444', // Red
    '#8B5CF6', // Violet
    '#EC4899', // Pink
    '#06B6D4', // Cyan
  ];
  return colors[Math.floor(Math.random() * colors.length)];
}

// For local execution
async function main() {
  // This function would be called with the actual D1 database when running with Wrangler
  console.log('To run this seed, execute with Wrangler:');
  console.log('npx wrangler d1 execute v5-website --local --file=./migrations/projects.sql');
  console.log('Then modify this script to connect to your database and run:');
  console.log('npx tsx migrations/seed-linear-data.ts');
}

main().catch(console.error);