import { createPsionicApp, css, document, html } from "../src"

const app = createPsionicApp({
  name: "Psionic Hello World",
  port: 3002
})

const styles = css`
  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }
  html, body {
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  body {
    background: black;
    color: white;
    font-family: "Berkeley Mono", monospace;
    display: flex;
    align-items: center;
    justify-content: center;
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
  }
  h1 {
    font-size: 3rem;
    font-weight: normal;
  }
`

app.route("/", () => {
  return document({
    title: "Psionic",
    styles,
    body: html`<h1>Psionic ⚡</h1>`
  })
})

app.start()
