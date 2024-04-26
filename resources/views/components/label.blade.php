@props(['value'])

<label {{ $attributes->merge(['class' => 'block text-[16px] font-bold text-text']) }}>
    {{ $value ?? $slot }}
</label>