import React from "react";
import { ImageResponse, cache } from "@cf-wasm/og/workers";

interface BlogPostMeta {
  title: string;
  episode?: number;
  date: string;
  description: string;
}

// Registry of blog posts - add new posts here
const BLOG_POSTS: Record<string, BlogPostMeta> = {
  "the-agent-network": {
    title: "The Agent Network",
    episode: 200,
    date: "January 1, 2026",
    description:
      "Predictions for 2026, Reed's Law of group-forming networks, and how agent networks will pay you",
  },
  "fracking-apple-silicon": {
    title: "Fracking Apple Silicon",
    episode: 201,
    date: "January 4, 2026",
    description:
      "Stranded compute, compute fracking, wildcatters, and why 110M Macs matter",
  },
  // Add future blog posts here
};

function getBlogMeta(slug: string): BlogPostMeta | null {
  return BLOG_POSTS[slug] || null;
}

export default {
  async fetch(
    request: Request,
    env: unknown,
    ctx: ExecutionContext
  ): Promise<Response> {
    // Set execution context for caching
    cache.setExecutionContext(ctx);

    const url = new URL(request.url);
    const path = url.pathname;

    // Handle root path - show available endpoints
    if (path === "/" || path === "") {
      return new Response(
        JSON.stringify({
          available: Object.keys(BLOG_POSTS).map((slug) => `/og/${slug}.png`),
        }),
        {
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    // Extract slug from path: /og/the-agent-network.png -> the-agent-network
    // Also handle direct paths like /the-agent-network.png
    const match = path.match(/^(?:\/og)?\/(.+?)(?:\.png)?$/);
    if (!match) {
      return new Response("Not found", { status: 404 });
    }

    const slug = match[1];
    const meta = getBlogMeta(slug);

    if (!meta) {
      return new Response(`Blog post not found: ${slug}`, { status: 404 });
    }

    // Build the content elements
    const elements = [];

    // Episode label
    if (meta.episode) {
      elements.push(
        <div
          key="episode"
          style={{
            display: "flex",
            color: "#888",
            fontSize: "28px",
            letterSpacing: "0.1em",
          }}
        >
          EPISODE {meta.episode}
        </div>
      );
    }

    // Title
    elements.push(
      <div
        key="title"
        style={{
          display: "flex",
          fontSize: "72px",
          fontWeight: "bold",
          marginTop: meta.episode ? "16px" : "0",
          lineHeight: 1.1,
        }}
      >
        {meta.title}
      </div>
    );

    // Description
    elements.push(
      <div
        key="description"
        style={{
          display: "flex",
          color: "#aaa",
          fontSize: "32px",
          marginTop: "32px",
          lineHeight: 1.4,
        }}
      >
        {meta.description}
      </div>
    );

    // Footer with orange accent
    elements.push(
      <div
        key="footer"
        style={{
          marginTop: "auto",
          display: "flex",
          flexDirection: "column",
          width: "100%",
        }}
      >
        <div
          style={{
            display: "flex",
            width: "100%",
            height: "4px",
            backgroundColor: "#f80",
            marginBottom: "24px",
          }}
        />
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            width: "100%",
          }}
        >
          <div style={{ display: "flex", fontSize: "28px", fontWeight: "bold" }}>
            OpenAgents
          </div>
          <div style={{ display: "flex", fontSize: "20px", color: "#666" }}>
            {meta.date}
          </div>
        </div>
      </div>
    );

    // Load Square721 font from static assets
    const fontUrl = "https://openagents.com/static/webFonts/Square721StdRoman/font.woff";
    const fontData = await fetch(fontUrl).then((res) => res.arrayBuffer());

    // Generate dots grid background
    const dotSpacing = 40;
    const dotSize = 2;
    const dots = [];
    for (let y = 0; y < 630; y += dotSpacing) {
      for (let x = 0; x < 1200; x += dotSpacing) {
        dots.push(
          <div
            key={`dot-${x}-${y}`}
            style={{
              position: "absolute",
              left: x,
              top: y,
              width: dotSize,
              height: dotSize,
              backgroundColor: "rgba(255, 255, 255, 0.25)",
            }}
          />
        );
      }
    }

    return new ImageResponse(
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          backgroundColor: "#000",
          color: "#fff",
          fontFamily: "Square721",
          padding: "120px 60px 60px 60px",
          position: "relative",
        }}
      >
        {/* Dots background */}
        <div style={{ position: "absolute", top: 0, left: 0, width: "100%", height: "100%", display: "flex" }}>
          {dots}
        </div>
        {/* Content */}
        <div style={{ display: "flex", flexDirection: "column", flex: 1, zIndex: 1 }}>
          {elements}
        </div>
      </div>,
      {
        width: 1200,
        height: 630,
        fonts: [
          {
            name: "Square721",
            data: fontData,
            style: "normal",
          },
        ],
      }
    );
  },
};
