-- Realms live under /realms/<slug> (the plural, as /projects/<slug>; the old /realm/<slug> answers 301
-- at the load balancer). A project's first-realm page takes the module's files from /realms/drop now,
-- without the redirect. Pages already written keep working through the 301; the next PUBLISH rewrites them.

CREATE OR REPLACE FUNCTION drops.realm_page(slug_ text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || drops.html(p.name) || ' · a first realm · OpenAgents</title>
<meta name="description" content="' || drops.html(p.line) || '">
' || drops.preview(slug_, p.name || ': a first realm', '/projects/' || p.slug || '/realm', origin) || '<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/projects/' || p.slug || '/realm/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
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
<p><a href="/projects/' || p.slug || '/realm/index.md">A first realm</a></p>
</main>
<script type="module" data-realm-of="' || p.slug || '" data-name="' || drops.html(p.name) || '">
const me = document.querySelector("script[data-realm-of]"), slug = me.dataset.realmOf;
const t = await (await fetch("/realms/drop", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.realm = slug; s.dataset.slot = "project-" + slug;
s.dataset.assets = "pack:/projects/" + slug + "/realm/realm.pack";
s.dataset.text = me.dataset.name + "\n/projects/" + slug;
document.body.append(s);
</script>
</body>
</html>
' FROM drops.projects p WHERE p.slug = slug_ $$;
