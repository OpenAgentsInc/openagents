import type { NextConfig } from "next";
import createMDX from '@next/mdx'

const nextConfig: NextConfig = {
  typescript: {
    ignoreBuildErrors: true,
  },
  eslint: {
    ignoreDuringBuilds: true,
  },
  reactStrictMode: false,
  devIndicators: false,
  pageExtensions: ['js', 'jsx', 'ts', 'tsx', 'md', 'mdx']
};

const withMDX = createMDX({
  // Add markdown plugins here, as desired
  options: {
    // @ts-ignore
    jsx: true,
    // @ts-ignore
    providerImportSource: '@mdx-js/react',
  }
})

export default withMDX(nextConfig);
