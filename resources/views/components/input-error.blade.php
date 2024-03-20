@props(['for'])

@error($for)
    <p {{ $attributes->merge(['class' => 'text-sm text-red-500']) }}>{{ $message }}</p>
@enderror
