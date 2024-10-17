<button {{ $attributes->merge(['class' => $classes()]) }}>
    {{ $slot }}
</button>