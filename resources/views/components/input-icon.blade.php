
@props([
    'type' => 'text',
    'disabled' => false,
    'className' => '',
    'iconName' => '',
    'name' => '',
    'id' => null
])


<div>
    <div class="mt-1 relative rounded-md shadow-sm">
      <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
        <!-- Heroicon name: solid/mail -->
        <x-icon :name="$iconName" class="w-[24px] h-[24px] m-0.5 flex flex-col justify-center items-center"/>
      </div>

      <input

            type="{{ $type }}"

            {{ $disabled ? 'disabled' : '' }}

            {!! $attributes->merge([
                'class' => "h-[48px]  text-[16px] border border-2 bg-transparent placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white border-[#3D3E42] rounded-md shadow-sm block w-full pl-10  mt-1 sm:text-sm rounded-md $className"
            ]) !!}

    />

    </div>
  </div>
