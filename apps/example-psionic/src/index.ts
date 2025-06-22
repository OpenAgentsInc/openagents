import { createPsionicApp, css, document, html } from "@openagentsinc/psionic"

const app = createPsionicApp({
  name: "Example Psionic App",
  port: 3100
})

const styles = css`
  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }
  
  body {
    background: #0a0a0a;
    color: #f0f0f0;
    font-family: system-ui, -apple-system, sans-serif;
    padding: 2rem;
    min-height: 100vh;
  }
  
  .container {
    max-width: 800px;
    margin: 0 auto;
  }
  
  h1 {
    font-size: 2.5rem;
    margin-bottom: 1rem;
    background: linear-gradient(to right, #60a5fa, #a78bfa);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  
  p {
    font-size: 1.125rem;
    line-height: 1.7;
    color: #a0a0a0;
    margin-bottom: 2rem;
  }
  
  .features {
    display: grid;
    gap: 1.5rem;
    margin-top: 2rem;
  }
  
  .feature {
    padding: 1.5rem;
    background: #1a1a1a;
    border-radius: 0.5rem;
    border: 1px solid #2a2a2a;
  }
  
  .feature h2 {
    font-size: 1.25rem;
    margin-bottom: 0.5rem;
    color: #60a5fa;
  }
  
  .feature p {
    font-size: 1rem;
    margin: 0;
  }
  
  code {
    background: #2a2a2a;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    font-family: "Berkeley Mono", monospace;
    font-size: 0.875rem;
  }
`

app.route("/", () => {
  return document({
    title: "Example Psionic App",
    styles,
    body: html`
      <div class="container">
        <h1>Welcome to Psionic</h1>
        <p>
          This is an example app demonstrating the Psionic web framework.
          Psionic is a hypermedia-focused web framework built on Bun and Elysia.
        </p>
        
        <div class="features">
          <div class="feature">
            <h2>Fast & Lightweight</h2>
            <p>Built on Bun and Elysia for maximum performance with minimal overhead.</p>
          </div>
          
          <div class="feature">
            <h2>HTML-First</h2>
            <p>Uses template literals for type-safe HTML generation with the <code>html</code> tag.</p>
          </div>
          
          <div class="feature">
            <h2>Component Explorer</h2>
            <p>Built-in component library explorer for systematic UI development.</p>
          </div>
        </div>
      </div>
    `
  })
})

app.route("/about", () => {
  return document({
    title: "About - Example Psionic App",
    styles,
    body: html`
      <div class="container">
        <h1>About This Example</h1>
        <p>
          This example demonstrates basic routing and HTML generation with Psionic.
          You can extend this app by adding more routes, integrating with the SDK,
          or building interactive components.
        </p>
        <p>
          <a href="/" style="color: #60a5fa;">← Back to Home</a>
        </p>
      </div>
    `
  })
})

app.start()

console.log(`Example Psionic app running at http://localhost:3100`)