import type { Route } from "./+types/home";
import { Welcome } from "../welcome/welcome";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents" },
    { name: "description", content: "Your agent dealer" },
  ];
}

export default function Home() {
  return <Welcome />;
}
