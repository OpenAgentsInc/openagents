export const title = "Navigation"
export const component = "nav"

export const Default = {
  name: "Default State",
  html: `
    <div class="nav-container">
      <nav class="nav-links">
        <a href="/" class="webtui-button webtui-variant-background1">Home</a>
        <a href="/agents" class="webtui-button webtui-variant-background1">Agents</a>
        <a href="/docs" class="webtui-button webtui-variant-background1">Docs</a>
        <a href="/blog" class="webtui-button webtui-variant-background1">Blog</a>
        <a href="/about" class="webtui-button webtui-variant-background1">About</a>
      </nav>
    </div>
  `,
  description: "Navigation bar with all links in default state"
}

export const HomeActive = {
  name: "Home Active",
  html: `
    <div class="nav-container">
      <nav class="nav-links">
        <a href="/" class="webtui-button webtui-variant-foreground1">Home</a>
        <a href="/agents" class="webtui-button webtui-variant-background1">Agents</a>
        <a href="/docs" class="webtui-button webtui-variant-background1">Docs</a>
        <a href="/blog" class="webtui-button webtui-variant-background1">Blog</a>
        <a href="/about" class="webtui-button webtui-variant-background1">About</a>
      </nav>
    </div>
  `,
  description: "Navigation with Home link in active state"
}

export const AgentsActive = {
  name: "Agents Active", 
  html: `
    <div class="nav-container">
      <nav class="nav-links">
        <a href="/" class="webtui-button webtui-variant-background1">Home</a>
        <a href="/agents" class="webtui-button webtui-variant-foreground1">Agents</a>
        <a href="/docs" class="webtui-button webtui-variant-background1">Docs</a>
        <a href="/blog" class="webtui-button webtui-variant-background1">Blog</a>
        <a href="/about" class="webtui-button webtui-variant-background1">About</a>
      </nav>
    </div>
  `,
  description: "Navigation with Agents link in active state"
}

export const DocsActive = {
  name: "Docs Active",
  html: `
    <div class="nav-container">
      <nav class="nav-links">
        <a href="/" class="webtui-button webtui-variant-background1">Home</a>
        <a href="/agents" class="webtui-button webtui-variant-background1">Agents</a>
        <a href="/docs" class="webtui-button webtui-variant-foreground1">Docs</a>
        <a href="/blog" class="webtui-button webtui-variant-background1">Blog</a>
        <a href="/about" class="webtui-button webtui-variant-background1">About</a>
      </nav>
    </div>
  `,
  description: "Navigation with Docs link in active state"
}