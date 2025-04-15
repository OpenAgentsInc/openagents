import type { Route } from "./+types/members";
import MainLayout from '@/components/layout/main-layout';

export function meta({ params, location, data }: Route.MetaArgs) {
  return [
    { title: "Members" },
    { name: "description", content: "Members" },
  ];
}

export async function loader({ }: Route.LoaderArgs) {
  return {};
}

export default function MembersPage() {
  return (
    <MainLayout header={<div />}>
      <div className="container mx-auto py-8">
        {/* Content will go here */}
      </div>
    </MainLayout>
  );
}
