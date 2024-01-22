@props(['className' => ''])

    <div
        {{ $attributes->merge(['class' => "rounded-xl border bg-card text-card-foreground shadow $className"]) }}>
        {{ $slot }}
    </div>
