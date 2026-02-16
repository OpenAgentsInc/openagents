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
            background-image: radial-gradient(circle at center, rgba(255, 255, 255, 0.1) 1px, transparent 1px);
            background-size: 48px 48px;
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
            <p>OpenAgents is temporarily unavailable while we complete an infrastructure switch. Please check back
                shortly.</p>
            <span class="status">Maintenance in progress</span>
        </section>
    </main>

    @php
        $posthog_key = config('posthog.api_key');
        $posthog_host = config('posthog.host', 'https://us.i.posthog.com');
    @endphp
    @if(!empty($posthog_key))
        <script>
            (function () {
                var k = @json($posthog_key);
                var h = @json($posthog_host);
                if (!k) return;
                var s = document.createElement('script');
                s.id = 'posthog-503';
                s.type = 'text/javascript';
                s.textContent =
                    "!(function(t,e){var o,n,p,r;e.__SV||((window.posthog=e),(e._i=[]),(e.init=function(i,s,a){function g(t,e){var o=e.split('.');2==o.length&&((t=t[o[0]]),(e=o[1])),(t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)));});}((p=t.createElement('script')).type='text/javascript'),(p.crossOrigin='anonymous'),(p.async=!0),(p.src=s.api_host+'/static/array.js'),(r=t.getElementsByTagName('script')[0]).parentNode.insertBefore(p,r);var u=e;void 0!==a?(u=e[a]=[]):(a='posthog');u.people=u.people||[];u.toString=function(t){var e='posthog';return'posthog'!==a&&(e+='.'+a),t||(e+=' (stub)'),e;};u.people.toString=function(){return u.toString(1)+'.people (stub)';};o='capture identify alias people.set people.set_once set_config register register_once unregister opt_out_capturing has_opted_out_capturing opt_in_capturing reset isFeatureEnabled onFeatureFlags getFeatureFlag getFeatureFlagPayload reloadFeatureFlags group updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures getActiveMatchingSurveys getSurveys getNextSurveyStep onSessionId'.split(' ');for(n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a]);}),(e.__SV=1));})(document,window.posthog||[]);" +
                    "posthog.init('" + k.replace(/\'/g, "\\'") + "',{api_host:'" + h.replace(/\'/g, "\\'") +
                    "'});" +
                    "setTimeout(function(){if(window.posthog)posthog.capture('503 maintenance page viewed',{'$current_url':window.location.href});},800);";
                document.head.appendChild(s);
            })();

        </script>
    @endif
</body>

</html>
