@props(['value'])

    <label
        {{ $attributes->merge(['class' => 'block font-medium mb-[6px] text-lightgray text-sm/[12px]']) }}>
        {{ $value ?? $slot }}
    </label>
