import type { Route } from "./+types/teams";
import Teams from '@/components/common/teams/teams';
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/teams/header';
import { getTeams } from '@/lib/db/team-helpers.server';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Teams - OpenAgents" },
    { name: "description", content: "Manage your teams" },
  ];
}

export async function loader({ context }: Route.LoaderArgs) {
  try {
    const teams = await getTeams();
    return { teams };
  } catch (error) {
    console.error('Error loading teams:', error);
    return { teams: [], error: 'Failed to load teams' };
  }
}

export default function TeamsPage() {
  return (
    <MainLayout header={<Header />}>
      <Teams />
    </MainLayout>
  );
}