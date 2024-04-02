@props(['value'])

<label {{ $attributes->merge(['class' => 'block font-medium text-sm text-text']) }}>
    {{ $value ?? $slot }}
</label>