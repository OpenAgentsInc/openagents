import { Outlet } from "react-router";
import { Header } from "~/components/header";

export function WithHeaderLayout() {
  return (
    <>
      <Header />
      <main className="pt-12">
        <Outlet />
      </main>
    </>
  );
}
