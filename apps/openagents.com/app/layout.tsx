import type { Metadata } from "next";
import { Titillium_Web } from "next/font/google";
import localFont from "next/font/local";
import "./globals.css";
import { ConvexAuthNextjsServerProvider } from "@convex-dev/auth/nextjs/server";
import ConvexClientProvider from "@/components/ConvexClientProvider";

const titilliumWeb = Titillium_Web({
  variable: "--font-titillium",
  subsets: ["latin"],
  weight: ["300", "400", "600", "700"],
});

const berkeleyMono = localFont({
  src: [
    {
      path: "../public/fonts/BerkeleyMono-Regular.woff2",
      weight: "400",
      style: "normal",
    },
    {
      path: "../public/fonts/BerkeleyMono-Italic.woff2",
      weight: "400", 
      style: "italic",
    },
    {
      path: "../public/fonts/BerkeleyMono-Bold.woff2",
      weight: "700",
      style: "normal",
    },
    {
      path: "../public/fonts/BerkeleyMono-BoldItalic.woff2",
      weight: "700",
      style: "italic",
    },
  ],
  variable: "--font-berkeley-mono",
});

export const metadata: Metadata = {
  title: "OpenAgents - Bitcoin-Powered Digital Agents",
  description: "Build autonomous agents that can send and receive Bitcoin payments through the Lightning Network",
  icons: {
    icon: "/favicon.ico",
    shortcut: "/favicon.ico",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <ConvexAuthNextjsServerProvider>
      <html lang="en">
        <body
          className={`${titilliumWeb.variable} ${berkeleyMono.variable} antialiased`}
        >
          <ConvexClientProvider>{children}</ConvexClientProvider>
        </body>
      </html>
    </ConvexAuthNextjsServerProvider>
  );
}
