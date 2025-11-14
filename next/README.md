This is the [assistant-ui](https://github.com/Yonom/assistant-ui) starter project.

## Getting Started (Bun)

This project uses Bun for package management. Make sure Bun is installed, then install dependencies:

```bash
bun install
```

Add your OpenAI API key to `.env.local`:

```
OPENAI_API_KEY=sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

Run the development server:

```bash
bun run dev
```

Build for production:

```bash
bun run build
```

Start the production server:

```bash
bun run start
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

You can start editing the page by modifying `app/page.tsx`. The page auto-updates as you edit the file.
