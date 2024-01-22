@props(['className' => ''])

    <h3
        {{ $attributes->merge(['class' => "font-semibold leading-none tracking-tight $className"]) }}>
        {{ $slot }}
    </h3>
