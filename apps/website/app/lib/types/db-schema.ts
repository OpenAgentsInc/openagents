// User type from the existing schema
export interface User {
  id: string;
  name: string;
  email: string;
  emailVerified: number;
  image: string | null;
  createdAt: Date;
  updatedAt: Date;
}

// Project schema type
export interface Project {
  id: string;
  name: string;
  status: string;
  icon: string;
  percentComplete: number;
  startDate: Date;
  priority: string;
  health: string;
  createdAt: Date;
  updatedAt: Date;
  ownerId: string; // Reference to the user who owns this project
}

// Team schema type
export interface Team {
  id: string;
  name: string;
  icon: string;
  color: string;
  createdAt: Date;
  updatedAt: Date;
  ownerId: string; // Reference to the user who owns this team
}

// User-Project relationship with role
export interface ProjectMember {
  id: string;
  projectId: string;
  userId: string;
  role: string; // 'owner', 'admin', 'member', etc.
  createdAt: Date;
  updatedAt: Date;
}

// User-Team relationship with role
export interface TeamMember {
  id: string;
  teamId: string;
  userId: string;
  role: string; // 'owner', 'admin', 'member', etc.
  createdAt: Date;
  updatedAt: Date;
}

// Team-Project relationship
export interface TeamProject {
  id: string;
  teamId: string;
  projectId: string;
  createdAt: Date;
  updatedAt: Date;
}

// Frontend representation types that include relations

export interface ProjectWithRelations extends Project {
  owner: User;
  members: Array<User & { role: string }>;
  teams: Team[];
}

export interface TeamWithRelations extends Team {
  owner: User;
  members: Array<User & { role: string }>;
  projects: Project[];
}

export interface UserWithRelations extends User {
  ownedProjects: Project[];
  ownedTeams: Team[];
  memberProjects: Array<Project & { role: string }>;
  memberTeams: Array<Team & { role: string }>;
}
