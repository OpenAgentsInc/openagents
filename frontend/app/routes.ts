import type { RouteConfig } from "@react-router/dev/routes";

export default [
  {
    file: "routes/_layout.tsx",
    children: [
      { file: "routes/home.tsx", index: true, path: "/" },
      { file: "routes/onyx.tsx", path: "onyx" },
      { file: "routes/video-series.tsx", path: "video-series" },
      { file: "routes/services.tsx", path: "services" },
      { file: "routes/company.tsx", path: "company" },
      { file: "routes/coming-soon.tsx", path: "coming-soon" },
      { file: "routes/cota.tsx", path: "cota" },
      { file: "routes/repomap.tsx", path: "repomap" },
      { file: "routes/login.tsx", path: "login" },
      { file: "routes/components.tsx", path: "components" },
      { file: "routes/thinking.tsx", path: "thinking" },
    ],
  },
  {
    file: "routes/chat/_layout.tsx",
    path: "chat",
    children: [
      { file: "routes/chat/index.tsx", index: true },
      { file: "routes/chat/$id.tsx", path: ":id" },
    ],
  },
] satisfies RouteConfig;
