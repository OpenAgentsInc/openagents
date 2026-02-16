<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>We'll be right back | OpenAgents</title>
    <style>
        :root {
            color-scheme: dark;
        }

        * {
            box-sizing: border-box;
        }

        html,
        body {
            margin: 0;
            min-height: 100%;
            width: 100%;
            background: #0a0a0a;
            color: #f5f5f5;
            font-family: 'Berkeley Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace;
        }

        body {
            position: relative;
            overflow: hidden;
        }

        .bg {
            position: fixed;
            inset: 0;
            pointer-events: none;
            background:
                radial-gradient(120% 85% at 50% 0%, rgba(255, 255, 255, 0.07) 0%, rgba(255, 255, 255, 0) 55%),
                radial-gradient(ellipse 100% 100% at 50% 50%, transparent 12%, rgba(0, 0, 0, 0.55) 60%, rgba(0, 0, 0, 0.88) 100%);
        }

        .grid {
            position: fixed;
            inset: 0;
            pointer-events: none;
            background-image: radial-gradient(circle at center, rgba(255, 255, 255, 0.15) 1px, transparent 1px);
            background-size: 36px 36px;
        }

        .container {
            position: fixed;
            inset: 0;
            z-index: 2;
            display: grid;
            place-items: center;
            padding: 2rem;
        }

        .panel {
            width: min(640px, 100%);
            border: 1px solid rgba(255, 255, 255, 0.2);
            background: rgba(10, 10, 10, 0.75);
            backdrop-filter: blur(2px);
            border-radius: 12px;
            padding: 2rem;
            box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.04) inset;
        }

        h1 {
            margin: 0;
            font-size: clamp(1.6rem, 2.2vw, 2.25rem);
            letter-spacing: 0.01em;
            line-height: 1.15;
        }

        p {
            margin: 0.9rem 0 0;
            color: rgba(255, 255, 255, 0.8);
            line-height: 1.5;
            font-size: 0.98rem;
        }

        .status {
            margin-top: 1.4rem;
            display: inline-block;
            font-size: 0.8rem;
            letter-spacing: 0.08em;
            text-transform: uppercase;
            border: 1px solid rgba(255, 255, 255, 0.25);
            padding: 0.45rem 0.65rem;
            border-radius: 8px;
            color: rgba(255, 255, 255, 0.88);
        }
    </style>
</head>

<body>
    <div class="bg" aria-hidden="true"></div>
    <div class="grid" aria-hidden="true"></div>

    <main class="container">
        <section class="panel" role="status" aria-live="polite">
            <h1>We'll be right back.</h1>
            <p>OpenAgents is temporarily unavailable while we complete an infrastructure switch. Please check back shortly.</p>
            <span class="status">Maintenance in progress</span>
        </section>
    </main>
</body>

</html>
