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
    <style>
        td {
            padding: 0.5rem;
        }
    </style>
</head>

<body>
<div>
    <h1>Explorer</h1>
    <h2>Recent payments</h2>
    <table class="table-auto">
        <thead>
        <tr>
            <th>Amount</th>
            <th>Currency</th>
            <th>Description</th>
            <th>Date</th>
        </tr>
        </thead>
        <tbody>
        @forelse ($recentPayments as $payment)
            <tr>
                <td>{{ $payment->amount }}</td>
                <td>{{ $payment->currency }}</td>
                <td>{{ $payment->description }}</td>
                <td>{{ $payment->created_at }}</td>
            </tr>
        @empty
            <tr>
                <td colspan="3">No recent payments found.</td>
            </tr>
        @endforelse
        </tbody>
    </table>
</div>
</body>

</html>
