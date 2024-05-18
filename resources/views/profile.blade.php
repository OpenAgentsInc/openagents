<!doctype html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">
<head>
    <title>OpenAgents</title>
    <script src="https://unpkg.com/htmx.org@1.9.12"></script>
    <style>
        html, body {
            background-color: black;
            color: white;
            font-family: monospace !important;
        }
    </style>
</head>
<body hx-boost="true">
<main>
    <header>
        <h1>{{ auth()->user()->name }}</h1>
        <h2>{{ $username }}</h2>
    </header>
    <p>Hello.</p>
</main>
</body>
</html>