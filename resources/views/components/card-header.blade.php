@props(['className' => ''])

    <div
        {{ $attributes->merge(['class' => "flex flex-col space-y-1.5 p-6 $className"]) }}>
        {{ $slot }}
    </div>
