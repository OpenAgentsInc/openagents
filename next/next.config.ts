import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  /* Silence monorepo root inference */
  outputFileTracingRoot: path.join(__dirname, ".."),
};

export default nextConfig;
