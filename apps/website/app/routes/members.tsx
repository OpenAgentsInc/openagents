import type { Route } from "./+types/members";
import Header from '@/components/layout/headers/members/header';
import MainLayout from '@/components/layout/main-layout';
import Members from '@/components/common/members/members';
import { getUsers } from "../lib/db/project-helpers.server";
import { getDb } from "../lib/db/team-helpers.server";
import { redirect } from "react-router";

export function meta({ params, location, data }: Route.MetaArgs) {
  return [
    { title: "Members - OpenAgents" },
    { name: "description", content: "Members" },
  ];
}

export async function loader({ request }: Route.LoaderArgs) {
  // Import auth only within loader (server-side only)
  const { requireAuth } = await import('@/lib/auth.server');
  
  // Check authentication with requireAuth helper
  const authResult = await requireAuth(request);
  
  if (authResult.redirect) {
    return redirect(authResult.redirect);
  }
  
  // Get all users from the database
  const users = await getUsers();

  // For each user, get their team memberships
  const db = getDb();
  const usersWithTeams = await Promise.all(
    users.map(async (user) => {
      // Get user's team memberships
      const teamMemberships = await db
        .selectFrom("team_membership")
        .innerJoin("team", "team.id", "team_membership.teamId")
        .select([
          "team.id as teamId",
          "team.name as teamName",
          "team.key as teamKey",
          "team.icon as teamIcon",
          "team_membership.owner"
        ])
        .where("team_membership.userId", "=", user.id)
        .execute();

      // Calculate joined date based on the first team membership (approximate)
      let joinedDate = new Date().toISOString();
      if (teamMemberships.length > 0) {
        const membership = await db
          .selectFrom("team_membership")
          .select(["createdAt"])
          .where("userId", "=", user.id)
          .orderBy("createdAt", "asc")
          .limit(1)
          .executeTakeFirst();

        if (membership) {
          joinedDate = membership.createdAt;
        }
      }

      // Determine role based on team ownership (simplified)
      const isAdmin = teamMemberships.some(membership => membership.owner === 1);

      return {
        ...user,
        teams: teamMemberships,
        teamIds: teamMemberships.map(tm => tm.teamKey),
        joinedDate,
        role: isAdmin ? "Admin" : "Member"
      };
    })
  );

  return { users: usersWithTeams };
}

export default function MembersPage() {
  return (
    <MainLayout header={<Header />}>
      <Members />
    </MainLayout>
  );
}
