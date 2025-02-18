import type { RouteConfig } from "@react-router/dev/routes"

export default [{
  file: "routes/_layout.tsx",
  children: [
    { file: "routes/home.tsx", index: true },
    { file: "routes/onyx.tsx" },
    { file: "routes/video-series.tsx" },
    { file: "routes/services.tsx" },
    { file: "routes/company.tsx" },
    { file: "routes/coming-soon.tsx" }
  ]
}] satisfies RouteConfig;
