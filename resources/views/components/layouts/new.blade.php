<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
    @include('partials.ogtags')
</head>

<body class="h-full bg-black">

<livewire:navbar/>
{{$slot}}

</body>