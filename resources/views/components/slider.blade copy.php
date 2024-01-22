<!-- uses alpine -->

@props(['min' => 0, 'max' => 100, 'step' => 1, 'value' => 0])

<div x-data="{ range: {{ $value }} }" class="w-full">
<input type="range" x-model="range" min="{{ $min }}" max="{{ $max }}" step="{{ $step }}" class="appearance-none accent-teal-500 w-full h-2 bg-gray-200 rounded-lg cursor-pointer" /> 
<span x-text="range"></span>
</div>