@props(['value'])

    <label
        {{ $attributes->merge(['class' => 'block font-medium mb-[4px] text-gray text-[14px] pl-[8px]' ]) }}>
        {{ $value ?? $slot }}
    </label>
