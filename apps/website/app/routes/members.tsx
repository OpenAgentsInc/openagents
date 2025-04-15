import type { Route } from "./+types/members";
import Header from '@/components/layout/headers/members/header';
import MainLayout from '@/components/layout/main-layout';
import Members from '@/components/common/members/members';

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
    <MainLayout header={<Header />}>
      <Members />
    </MainLayout>
  );
}
