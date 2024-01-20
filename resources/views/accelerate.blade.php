<!DOCTYPE html>
<html>

<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents</title>
    @include('partials.css')
</head>

<body class="antialiased font-mono dark:text-white bg-slate-50 dark:bg-haiti">
    <x-header />

    <div class="px-4 sm:px-6 md:px-8">
        <div class="prose dark:prose-invert max-w-xl mx-auto pt-12">
            <h2>Four trends</h2>
            <ul>
                <li>Bitcoin as money</li>
                <li>Bitcoin-adjacent app layers (Nostr, Fedimint)</li>
                <li>Web 4.0 (lol) - Web of 'no thx'?</li>
                <ul class="pl-8">
                    <li>“Big JavaScript” → hypermedia (HTMX)</li>
                    <li>Isolated moats → WASM interop</li>
                </ul>
                <li>Open-source AI</li>
            </ul>

            <h2>No more</h2>
            <ul>
                <li>I don't want your shitcoin/fedcoin/fiat</li>
                <li>I don't want your censorship</li>
                <li>I don't want your abstractions</li>
                <li>I don't want your reinvented wheel</li>
                <li>I don't want your tech debt</li>
                <li>I don't want your silos</li>
                <li>I don't want your performative ethics</li>
            </ul>
        </div>
    </div>
</body>

</html>
