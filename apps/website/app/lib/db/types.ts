import { Project, Team, ProjectMember, TeamMember, TeamProject, User } from '../types/db-schema';

// Define the database interface for Kysely
export interface Database {
  user: User;
  project: Project;
  team: Team;
  project_member: ProjectMember;
  team_member: TeamMember;
  team_project: TeamProject;
  // Include other tables from better-auth.sql
  session: {
    id: string;
    expiresAt: Date;
    token: string;
    createdAt: Date;
    updatedAt: Date;
    ipAddress: string | null;
    userAgent: string | null;
    userId: string;
  };
  account: {
    id: string;
    accountId: string;
    providerId: string;
    userId: string;
    accessToken: string | null;
    refreshToken: string | null;
    idToken: string | null;
    accessTokenExpiresAt: Date | null;
    refreshTokenExpiresAt: Date | null;
    scope: string | null;
    password: string | null;
    createdAt: Date;
    updatedAt: Date;
  };
  verification: {
    id: string;
    identifier: string;
    value: string;
    expiresAt: Date;
    createdAt: Date | null;
    updatedAt: Date | null;
  };
}