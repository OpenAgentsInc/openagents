import { Elysia } from "elysia"

const app = new Elysia()
  .get("/", () => "Hello from Psionic! 🧠")
  .get("/hypermedia", () => `
    <html>
      <body>
        <h1>Psionic Hypermedia</h1>
        <p>Server-rendered HTML is the future.</p>
        <button onclick="alert('Minimal JS!')">Click me</button>
      </body>
    </html>
  `)
  .listen(3002)

console.log(`🧠 Psionic is running at http://${app.server?.hostname}:${app.server?.port}`)