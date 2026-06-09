import { isRecord, optionalString, safeJsonRecord } from './json-boundary'

export type TeamRole = 'owner' | 'admin' | 'member' | 'viewer'

export type TeamMembershipStatus = 'active' | 'invited' | 'removed'

export type UserTeamMember = Readonly<{
  userId: string
  name: string
  email: string | null
  avatarUrl: string | null
  githubUsername: string | null
  githubId: string | null
  role: TeamRole
  status: TeamMembershipStatus
  joinedAt: string | null
}>

export type UserTeamProject = Readonly<{
  id: string
  teamId: string
  name: string
  slug: string | null
  description: string
  status: 'active' | 'archived'
  agent?: UserTeamProjectAgent
}>

export type UserTeamProjectAgent = Readonly<{
  id: string
  name: string
  status: string
  scope: string
  runtime: string
  backend: string
  repository: string
  focus: string
}>

export type UserTeam = Readonly<{
  id: string
  name: string
  slug: string | null
  role: TeamRole
  members: ReadonlyArray<UserTeamMember>
  projects: ReadonlyArray<UserTeamProject>
}>

export const userTeamProjectAgentFromMetadata = (
  metadata: Record<string, unknown> | undefined,
): UserTeamProjectAgent | undefined => {
  const agent = metadata?.agent

  if (!isRecord(agent)) {
    return undefined
  }

  const id = optionalString(agent.id)
  const name = optionalString(agent.name)
  const status = optionalString(agent.status)
  const scope = optionalString(agent.scope)
  const runtime = optionalString(agent.runtime)
  const backend = optionalString(agent.backend)
  const repository = optionalString(agent.repository)
  const focus = optionalString(agent.focus)

  return id === undefined ||
    name === undefined ||
    status === undefined ||
    scope === undefined ||
    runtime === undefined ||
    backend === undefined ||
    repository === undefined ||
    focus === undefined
    ? undefined
    : {
        id,
        name,
        status,
        scope,
        runtime,
        backend,
        repository,
        focus,
      }
}

export const readTeamsForUser = async (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<UserTeam>> => {
  const teamRows = await db
    .prepare(
      `SELECT
         teams.id,
         teams.name,
         teams.slug,
         team_memberships.role
       FROM team_memberships
       INNER JOIN teams ON teams.id = team_memberships.team_id
       WHERE team_memberships.user_id = ?
         AND team_memberships.status = 'active'
         AND teams.status = 'active'
         AND teams.archived_at IS NULL
       ORDER BY teams.name`,
    )
    .bind(userId)
    .all<
      Readonly<{
        id: string
        name: string
        slug: string | null
        role: TeamRole
      }>
    >()

  return Promise.all(
    teamRows.results.map(async team => {
      const members = await db
        .prepare(
          `SELECT
             users.id AS user_id,
             users.display_name,
             users.primary_email,
             users.avatar_url,
             team_memberships.role,
             team_memberships.status,
             team_memberships.joined_at,
             auth_identities.provider_subject AS github_id,
             auth_identities.provider_username AS github_username
           FROM team_memberships
           INNER JOIN users ON users.id = team_memberships.user_id
           LEFT JOIN auth_identities
             ON auth_identities.user_id = users.id
            AND auth_identities.provider = 'github'
            AND auth_identities.deleted_at IS NULL
           WHERE team_memberships.team_id = ?
             AND team_memberships.status = 'active'
             AND users.kind = 'human'
             AND users.status = 'active'
             AND users.deleted_at IS NULL
           ORDER BY users.display_name`,
        )
        .bind(team.id)
        .all<
          Readonly<{
            user_id: string
            display_name: string
            primary_email: string | null
            avatar_url: string | null
            role: TeamRole
            status: TeamMembershipStatus
            joined_at: string | null
            github_id: string | null
            github_username: string | null
          }>
        >()
      const projects = await db
        .prepare(
          `SELECT
             id,
             team_id,
             slug,
             name,
             description,
             status,
             metadata_json
           FROM team_projects
           WHERE team_id = ?
             AND status = 'active'
             AND archived_at IS NULL
           ORDER BY name`,
        )
        .bind(team.id)
        .all<
          Readonly<{
            id: string
            team_id: string
            slug: string | null
            name: string
            description: string
            status: 'active' | 'archived'
            metadata_json: string
          }>
        >()

      return {
        id: team.id,
        name: team.name,
        slug: team.slug,
        role: team.role,
        members: members.results.map(member => ({
          userId: member.user_id,
          name: member.display_name,
          email: member.primary_email,
          avatarUrl: member.avatar_url,
          githubUsername: member.github_username,
          githubId: member.github_id,
          role: member.role,
          status: member.status,
          joinedAt: member.joined_at,
        })),
        projects: projects.results.map(project => {
          const agent = userTeamProjectAgentFromMetadata(
            safeJsonRecord(project.metadata_json),
          )

          return {
            id: project.id,
            teamId: project.team_id,
            name: project.name,
            slug: project.slug,
            description: project.description,
            status: project.status,
            ...(agent === undefined ? {} : { agent }),
          }
        }),
      }
    }),
  )
}

export const readActiveTeamProject = async (
  db: D1Database,
  teamId: string,
  projectId: string,
): Promise<UserTeamProject | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         id,
         team_id,
         slug,
         name,
         description,
         status,
         metadata_json
       FROM team_projects
       WHERE team_id = ?
         AND id = ?
         AND status = 'active'
         AND archived_at IS NULL
       LIMIT 1`,
    )
    .bind(teamId, projectId)
    .first<
      Readonly<{
        id: string
        team_id: string
        slug: string | null
        name: string
        description: string
        status: 'active' | 'archived'
        metadata_json: string
      }>
    >()

  if (row === null) {
    return undefined
  }

  const agent = userTeamProjectAgentFromMetadata(
    safeJsonRecord(row.metadata_json),
  )

  return {
    id: row.id,
    teamId: row.team_id,
    name: row.name,
    slug: row.slug,
    description: row.description,
    status: row.status,
    ...(agent === undefined ? {} : { agent }),
  }
}

export const readActiveTeamMembershipRole = async (
  db: D1Database,
  teamId: string,
  userId: string,
): Promise<TeamRole | undefined> => {
  const row = await db
    .prepare(
      `SELECT team_memberships.role
       FROM team_memberships
       INNER JOIN teams ON teams.id = team_memberships.team_id
       WHERE team_memberships.team_id = ?
         AND team_memberships.user_id = ?
         AND team_memberships.status = 'active'
         AND teams.status = 'active'
         AND teams.archived_at IS NULL
       LIMIT 1`,
    )
    .bind(teamId, userId)
    .first<Readonly<{ role: TeamRole }>>()

  return row?.role
}
