-- A published project's page takes the engine's files from a page every build makes (rc-publish):
-- they are named by their content, so a page written once by the API can't name them itself.
CREATE OR REPLACE FUNCTION drops.publish_page(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>' || drops.html(p.name) || ' · OpenAgents project</title>
<meta name="description" content="' || drops.html(p.line) || '">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/projects/' || p.slug || '/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ff8c1f}
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>' || drops.html(p.name) || '</h1>
<p><a href="/projects/' || p.slug || '/index.md">Tasks</a></p>
</main>
<script type="module" data-project="' || p.pubkey || ' ' || p.slug || '">
// The engine''s files are named by their content (each release names them anew): this page takes the
// project app''s script from a page every build makes, /projects/coax, and names its own project.
const t = await (await fetch("/projects/coax", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.text = document.currentScript?.dataset.project || document.querySelector("script[data-project]").dataset.project;
document.body.append(s);
</script>
</body>
</html>
' FROM drops.projects p WHERE p.slug = slug_ $$;

