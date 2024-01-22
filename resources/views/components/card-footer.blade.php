@props(['className' => ''])

    <div
        {{ $attributes->merge(['class' => "flex items-center p-6 pt-0 $className"]) }}>
        {{ $slot }}
    </div>
