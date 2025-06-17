import { Elysia } from "elysia"

const app = new Elysia()
  .get("/", ({ set }) => {
    set.headers['content-type'] = 'text/html; charset=utf-8'
    return `
    <!DOCTYPE html>
    <html>
      <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <title>Psionic</title>
        <style>
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
        </style>
      </head>
      <body>
        <h1>Psionic ⚡</h1>
      </body>
    </html>
  `
  })
  .all("*", ({ set }) => {
    set.status = 302
    set.headers['location'] = '/'
  })
  .listen(3002)

console.log(`🧠 Psionic is running at http://${app.server?.hostname}:${app.server?.port}`)