@props(['className' => ''])

    <p
        {{ $attributes->merge(['class' => "text-sm text-muted-foreground $className"]) }}>
        {{ $slot }}
    </p>
