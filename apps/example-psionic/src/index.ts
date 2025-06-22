import { createPsionicApp, css, document, html } from "@openagentsinc/psionic"

const app = createPsionicApp({
  name: "Example Psionic App",
  port: 3100
})

const styles = css`
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
`

app.route("/", () => {
  return document({
    title: "Example Psionic App",
    styles,
    body: html`
      <h1>Hello from Psionic + Effect! ⚡</h1>
    `
  })
})

app.start()