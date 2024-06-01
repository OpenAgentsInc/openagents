@props(['disabled' => false])
<div
    x-data="{
        selectedOption: @entangle('selectedOption'),
        options: @json($options),
        selectOption(option) {
            this.selectedOption = option;
        }
    }"
>
    <select
        x-model="selectedOption"
        @change="selectOption(selectedOption)"
        {{ $disabled ? 'disabled' : '' }}
        {!! $attributes->merge(['class' => 'border-darkgray bg-black text-white focus:border-white focus:ring-white rounded-md shadow-sm']) !!}
    >
        <option value="">Select an option</option>
        <template x-for="(option, index) in options" :key="index">
            <option :value="option.value" x-text="option.label"></option>
        </template>
    </select>
</div>
