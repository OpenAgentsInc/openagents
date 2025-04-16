import { sql } from 'kysely';
import { 
  text, 
  integer, 
  sqliteTable,
  primaryKey
} from 'drizzle-orm/sqlite-core';

// User table (defined in better-auth.sql, but included here for reference)
export const user = sqliteTable('user', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  email: text('email').notNull().unique(),
  emailVerified: integer('emailVerified').notNull(),
  image: text('image'),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
});

// Project table
export const project = sqliteTable('project', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  status: text('status').notNull(),
  icon: text('icon').notNull(),
  percentComplete: integer('percentComplete').notNull().default(0),
  startDate: text('startDate').notNull(),
  priority: text('priority').notNull(),
  health: text('health').notNull(),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
  ownerId: text('ownerId')
    .notNull()
    .references(() => user.id),
});

// Team table
export const team = sqliteTable('team', {
  id: text('id').primaryKey(),
  name: text('name').notNull(),
  icon: text('icon').notNull(),
  color: text('color').notNull(),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
  ownerId: text('ownerId')
    .notNull()
    .references(() => user.id),
});

// Many-to-many relationship tables

// Users belonging to projects with permissions
export const projectMember = sqliteTable('project_member', {
  id: text('id').primaryKey(),
  projectId: text('projectId')
    .notNull()
    .references(() => project.id, { onDelete: 'cascade' }),
  userId: text('userId')
    .notNull()
    .references(() => user.id, { onDelete: 'cascade' }),
  role: text('role').notNull().default('member'),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
}, (table) => {
  return {
    unq: primaryKey({ columns: [table.projectId, table.userId] }),
  };
});

// Users belonging to teams with permissions
export const teamMember = sqliteTable('team_member', {
  id: text('id').primaryKey(),
  teamId: text('teamId')
    .notNull()
    .references(() => team.id, { onDelete: 'cascade' }),
  userId: text('userId')
    .notNull()
    .references(() => user.id, { onDelete: 'cascade' }),
  role: text('role').notNull().default('member'),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
}, (table) => {
  return {
    unq: primaryKey({ columns: [table.teamId, table.userId] }),
  };
});

// Projects belonging to teams
export const teamProject = sqliteTable('team_project', {
  id: text('id').primaryKey(),
  teamId: text('teamId')
    .notNull()
    .references(() => team.id, { onDelete: 'cascade' }),
  projectId: text('projectId')
    .notNull()
    .references(() => project.id, { onDelete: 'cascade' }),
  createdAt: text('createdAt').notNull().$defaultFn(() => new Date().toISOString()),
  updatedAt: text('updatedAt').notNull().$defaultFn(() => new Date().toISOString()),
}, (table) => {
  return {
    unq: primaryKey({ columns: [table.teamId, table.projectId] }),
  };
});