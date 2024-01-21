@props(['className' => ''])

    <div
        {{ $attributes->merge(['class' => "p-6 pt-0 $className"]) }}>
        {{ $slot }}
    </div>
