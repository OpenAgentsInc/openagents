<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    <script defer src="https://unpkg.com/alpinejs@3/dist/cdn.min.js"></script>
    <script src="https://unpkg.com/htmx.org@2.0.0-beta4/dist/htmx.min.js"></script>
    <script src="https://unpkg.com/htmx-ext-sse@2.0.0/sse.js"></script>
    @include('partials.vite')
</head>

<body>
<div>
    <h1>Explorer</h1>
    <h2>Recent payments</h2>
    <ul>
        @forelse ($recentPayments as $payment)
            <li>{{ $payment->amount }}</li>
        @empty
            <li>No recent payments found.</li>
        @endforelse
    </ul>
</div>
</body>

</html>
