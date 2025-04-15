import type { Route } from "./+types/teams";
import Teams from '@/components/common/teams/teams';
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/teams/header';

export function meta({ params, location, data }: Route.MetaArgs) {
  return [
    { title: "Teams" },
    { name: "description", content: "Manage your teams" },
  ];
}

export async function loader({ }: Route.LoaderArgs) {
  // TODO: Implement teams data fetching
  return { teams: [] };
}

export default function TeamsPage() {
  return (
    <MainLayout header={<Header />}>
      <Teams />
    </MainLayout>
  );
}
