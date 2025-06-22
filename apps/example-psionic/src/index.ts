import { createPsionicApp } from "@openagentsinc/psionic"

const app = createPsionicApp({
  name: "Example Psionic App",
  port: 3100
})

app.get("/", () => {
  return `
    <!DOCTYPE html>
    <html>
      <head>
        <title>Example Psionic App</title>
        <style>
          body {
            background: #0a0a0a;
            color: #f0f0f0;
            font-family: system-ui, -apple-system, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
          }
          
          h1 {
            font-size: 3rem;
            background: linear-gradient(to right, #60a5fa, #a78bfa);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
          }
        </style>
      </head>
      <body>
        <h1>Hello from Psionic + Effect! ⚡</h1>
      </body>
    </html>
  `
})

app.get("/api/hello", () => {
  return { message: "Hello from Psionic API!", timestamp: new Date().toISOString() }
})

app.get("/api/users/:id", (context) => {
  const userId = context.params.id
  return {
    id: userId,
    name: `User ${userId}`,
    email: `user${userId}@example.com`
  }
})

app.post("/api/echo", async (context) => {
  try {
    const body = await context.request.json()
    return {
      echo: body,
      timestamp: new Date().toISOString()
    }
  } catch (error) {
    return {
      error: "Invalid JSON",
      timestamp: new Date().toISOString()
    }
  }
})

app.start()