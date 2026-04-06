const POSTS = [
  {
    title: "The OpenAgents Mobile App",
    date: "2025-07-21",
    href: "/blog/the-openagents-mobile-app/",
  },
  {
    title: "American DeepSeek?",
    date: "2025-07-04",
    href: "/blog/american-deepseek/",
  },
  {
    title: "Analyzing the June 12 Internet Outage",
    date: "2025-06-13",
    href: "/blog/outage-lessons/",
  },
  {
    title: "AI Agents @ OFF 2025",
    date: "2025-06-02",
    href: "/blog/ai-agents-at-off-2025/",
  },
  {
    title: "GPUtopia 2.0",
    date: "2025-05-14",
    href: "/blog/gputopia/",
  },
  {
    title: "We're on Discord",
    date: "2025-05-10",
    href: "/blog/discord/",
  },
  {
    title: "Intro to AI Coding Agents",
    date: "2025-04-30",
    href: "/blog/intro-to-ai-coding-agents/",
  },
];

function formatDate(date) {
  return date.replaceAll("-", ".");
}

function renderPosts() {
  return POSTS.map(
    (post) => `
      <li>
        <a href="${post.href}">
          <div class="post-item">
            <p class="title">${post.title}</p>
            <div class="divider"></div>
            <p class="date">${formatDate(post.date)}</p>
          </div>
        </a>
      </li>
    `,
  ).join("");
}

function renderHtml() {
  const year = new Date().getFullYear();

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>OpenAgents</title>
  <meta
    name="description"
    content="Your agent dealer. Download our beta mobile app on TestFlight now."
  >
  <link rel="icon" href="/favicon.ico" type="image/x-icon">
  <style>
    @font-face {
      font-family: 'Berkeley Mono';
      font-style: normal;
      font-weight: 400;
      font-display: swap;
      src: url('/fonts/BerkeleyMono-Regular.woff2') format('woff2');
    }
    @font-face {
      font-family: 'Berkeley Mono';
      font-style: italic;
      font-weight: 400;
      font-display: swap;
      src: url('/fonts/BerkeleyMono-Italic.woff2') format('woff2');
    }
    @font-face {
      font-family: 'Berkeley Mono';
      font-style: normal;
      font-weight: 700;
      font-display: swap;
      src: url('/fonts/BerkeleyMono-Bold.woff2') format('woff2');
    }
    @font-face {
      font-family: 'Berkeley Mono';
      font-style: italic;
      font-weight: 700;
      font-display: swap;
      src: url('/fonts/BerkeleyMono-BoldItalic.woff2') format('woff2');
    }
    :root {
      --content-width: 35rem;
      --bg: oklch(0.1 0 0);
      --text-primary: oklch(0.9 0 0);
      --text-secondary: oklch(0.75 0 0);
      --text-tertiary: oklch(0.4 0 0);
      --border: oklch(0.2 0 0);
      --selection: oklch(0.2 0 0);
      --font-size-s: 0.8125rem;
      --font-size-m: 0.9375rem;
      --font-weight-light: 400;
      --font-weight-bold: 700;
      --spacing-s: -0.08em;
      --spacing-m: -0.02em;
    }
    * {
      box-sizing: border-box;
    }
    html {
      background-color: var(--bg);
      scroll-behavior: smooth;
      scrollbar-gutter: stable;
      overscroll-behavior-y: contain;
      -webkit-overflow-scrolling: touch;
    }
    body {
      background-color: var(--bg);
      color: var(--text-primary);
      font-family: 'Berkeley Mono', -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif;
      font-feature-settings: 'ss03' 1;
      font-size: var(--font-size-m);
      line-height: 1.75;
      text-rendering: optimizeLegibility;
      margin: 0 auto;
      min-height: 100vh;
      max-width: var(--content-width);
      display: flex;
      flex-direction: column;
      letter-spacing: var(--spacing-m);
      padding: 6rem 1.5rem 1.5rem 1.5rem;
      word-wrap: break-word;
      overflow-wrap: break-word;
      overscroll-behavior-y: contain;
      transition: background-color 0.2s ease-out;
    }
    @media (max-width: 768px) {
      body {
        padding: 4rem 1.35rem 1.35rem 1.35rem;
      }
    }
    @media (prefers-reduced-motion: reduce) {
      * {
        animation-duration: 0.01ms !important;
        transition-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        scroll-behavior: auto !important;
      }
    }
    ::selection {
      background-color: var(--selection);
    }
    :focus {
      outline: 2px solid var(--text-tertiary);
      outline-offset: 2px;
    }
    :focus:not(:focus-visible) {
      outline: none;
    }
    :focus-visible {
      outline: 2px solid var(--text-tertiary);
      outline-offset: 2px;
    }
    .layout-wrapper {
      display: flex;
      flex: 1;
      flex-direction: column;
      min-height: calc(100vh - 7.5rem);
    }
    @media (max-width: 768px) {
      .layout-wrapper {
        min-height: calc(100vh - 5.5rem);
      }
    }
    .page-content {
      display: flex;
      flex: 1;
      flex-direction: column;
    }
    .page-content main {
      flex: 1;
      display: flex;
      flex-direction: column;
    }
    .gradient-mask {
      position: fixed;
      top: 0;
      left: 0;
      width: 100%;
      height: 6rem;
      z-index: 99;
      pointer-events: none;
      background: linear-gradient(to bottom, var(--bg) 0%, transparent 100%);
      mask-image: linear-gradient(
        to bottom,
        black 0%,
        rgba(0, 0, 0, 0.8) 20%,
        rgba(0, 0, 0, 0.6) 40%,
        rgba(0, 0, 0, 0.4) 60%,
        rgba(0, 0, 0, 0.2) 80%,
        transparent 100%
      );
      -webkit-mask-image: linear-gradient(
        to bottom,
        black 0%,
        rgba(0, 0, 0, 0.8) 20%,
        rgba(0, 0, 0, 0.6) 40%,
        rgba(0, 0, 0, 0.4) 60%,
        rgba(0, 0, 0, 0.2) 80%,
        transparent 100%
      );
      opacity: 0;
      transition: opacity 0.3s ease;
    }
    header {
      margin: 0 0 1.25rem 0;
    }
    nav {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    header a {
      color: var(--text-primary);
      display: inline-block;
      font-weight: var(--font-weight-bold);
      min-width: 3rem;
      text-decoration: none;
    }
    .nav-spacer {
      display: inline-block;
      width: 1rem;
      height: 1rem;
    }
    .about {
      margin-bottom: 1.25rem;
    }
    .about p {
      margin: 0 0 1.25rem 0;
      color: var(--text-primary);
    }
    .about ul {
      list-style-type: disc;
      list-style-position: outside;
      padding-left: 1.25em;
      margin: 0 0 1.25rem 0;
      line-height: 1.75;
    }
    .about li {
      color: var(--text-primary);
      margin-bottom: 0.5em;
      padding-left: 0.25em;
      position: relative;
    }
    .about li:last-child {
      margin-bottom: 0;
    }
    .about a {
      color: var(--text-primary);
      text-decoration: none;
    }
    .about .image-link {
      display: block;
      margin: 1em 0 1.25rem 0;
    }
    .about .image-link img {
      border: 1px solid var(--border);
      display: block;
      width: 100%;
      height: auto;
    }
    .posts {
      list-style-type: none;
      margin: 0;
      padding: 0;
      display: flex;
      flex-direction: column;
      gap: 0;
    }
    .posts a {
      color: var(--text-primary);
      display: block;
      text-decoration: none;
      transition: opacity 0.15s ease-out;
      opacity: 0.65;
    }
    @media (hover: hover) and (pointer: fine) {
      .posts a:hover {
        opacity: 1;
      }
      .posts a:hover .divider {
        background-color: var(--text-tertiary);
        opacity: 0.75;
      }
      .posts a:hover .date {
        color: var(--text-secondary);
        opacity: 1;
      }
    }
    .post-item {
      height: 2.75rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.75rem;
    }
    .title {
      margin: 0;
      flex-shrink: 1;
      min-width: 0;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .divider {
      flex: 1 1 auto;
      min-width: 3rem;
      margin: 0 0.25rem;
      height: 0.5px;
      background-color: var(--border);
    }
    .date {
      margin: 0;
      color: var(--text-secondary);
      opacity: 0.75;
      letter-spacing: var(--spacing-s);
      flex-shrink: 0;
      white-space: nowrap;
      font-variant-numeric: tabular-nums;
      font-feature-settings:
        'tnum' 1,
        'zero' 0,
        'cv01' 1,
        'cv02' 1,
        'calt' 1,
        'ss03' 1,
        'ordn' 1;
    }
    .placeholder {
      height: 3rem;
    }
    footer {
      color: var(--text-secondary);
      font-size: var(--font-size-s);
      font-weight: var(--font-weight-light);
      line-height: 1.75;
      margin-top: 4rem;
      opacity: 0.75;
    }
    .footer-content {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-direction: row;
      flex-wrap: nowrap;
      width: 100%;
      gap: 1rem;
      min-height: 2rem;
    }
    .copyright,
    .powered-by {
      white-space: nowrap;
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .copyright .date {
      opacity: 1;
    }
    .social-links {
      display: flex;
      align-items: center;
      justify-content: center;
      flex: 0 0 auto;
    }
    .social-links a {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 2rem;
      height: 2rem;
      border-radius: 50%;
      color: var(--text-secondary);
      text-decoration: none;
      transition: all 0.2s ease-out;
    }
    .social-links a:hover {
      color: var(--text-primary);
      background-color: var(--border);
    }
    .social-links svg {
      width: 18px;
      height: 18px;
    }
    @media (max-width: 640px) {
      .footer-content {
        flex-direction: column;
        gap: 0.75rem;
      }
    }
  </style>
</head>
<body>
  <div class="gradient-mask"></div>
  <div class="layout-wrapper">
    <div class="page-content">
      <header>
        <nav>
          <a href="/">OpenAgents</a>
          <span class="nav-spacer" aria-hidden="true"></span>
        </nav>
      </header>
      <main>
        <section class="about" aria-label="About OpenAgents">
          <p>We build AI agents on open protocols.</p>
          <a class="image-link" href="/blog/the-openagents-mobile-app/">
            <img src="/talktoit.png" alt="The OpenAgents mobile app">
          </a>
          <p>The OpenAgents mobile app gives you a voice agent that:</p>
          <ul>
            <li>Remembers all your conversations</li>
            <li>Evolves based on how you use it</li>
            <li>(Soon) Uses tools and discovers new tools via MCP</li>
            <li>(Soon) Writes code and controls Claude Code</li>
            <li>(Soon) Interacts with other agents</li>
            <li>(Soon) Earns you bitcoin</li>
          </ul>
          <p>The app is available now on TestFlight for iOS. Coming soon to Android.</p>
          <p><a href="https://testflight.apple.com/join/dvQdns5B">Join the TestFlight: https://testflight.apple.com/join/dvQdns5B</a></p>
          <p><a href="/blog/the-openagents-mobile-app/">Read more about the app in our announcement.</a></p>
        </section>
        <section aria-label="Recent posts">
          <ul class="posts">
            ${renderPosts()}
          </ul>
          <div class="placeholder"></div>
        </section>
      </main>
      <footer>
        <div class="footer-content">
          <div class="copyright">
            <span class="date">&copy; ${year}</span>
            <span>OpenAgents, Inc.</span>
          </div>
          <div class="social-links">
            <a href="https://x.com/OpenAgentsInc" target="_blank" rel="noopener noreferrer" aria-label="Follow us on X">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" fill="currentColor"/>
              </svg>
            </a>
          </div>
          <div class="powered-by">Made in Austin, Texas</div>
        </div>
      </footer>
    </div>
  </div>
  <script>
    const gradientMask = document.querySelector('.gradient-mask');
    const updateMask = () => {
      if (!gradientMask) {
        return;
      }
      gradientMask.style.opacity = window.scrollY >= 64 ? '1' : '0';
    };
    updateMask();
    window.addEventListener('scroll', updateMask, { passive: true });
  </script>
</body>
</html>`;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/" || url.pathname === "") {
      return new Response(renderHtml(), {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      });
    }
    return env.ASSETS.fetch(request);
  },
};
