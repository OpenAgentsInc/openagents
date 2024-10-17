<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents</title>
    <link rel="stylesheet" href="{{ asset('css/jbm.css') }}">
    <link rel="stylesheet" href="{{ asset('css/variables.css') }}">
    <script src="{{ asset('js/tailwind.min.js') }}"></script>
    <script src="{{ asset('js/tailwind-config.js') }}"></script>
</head>
<body class="bg-background text-foreground">
    <main class="flex flex-col items-center justify-center min-h-screen p-4">
        <h1 class="text-4xl font-bold mb-8">OpenAgents</h1>
        <div class="flex flex-wrap justify-center gap-4">
            <x-button>
                Default Button
            </x-button>

            <x-button variant="destructive" size="sm">
                Small Destructive
            </x-button>

            <x-button variant="outline" size="lg">
                Large Outline
            </x-button>

            <x-button variant="secondary">
                Secondary Button
            </x-button>

            <x-button variant="ghost">
                Ghost Button
            </x-button>

            <x-button variant="link">
                Link Button
            </x-button>
        </div>
    </main>
</body>
</html>