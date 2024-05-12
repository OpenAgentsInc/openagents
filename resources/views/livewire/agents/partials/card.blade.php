<div>
    {{-- Stop trying to control. --}}
    <div class="w-full h-[70vh] flex flex-col justify-center">
        <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
            <p class="text-[16px] text-gray">Now speaking with...</p>
            <div class="max-w-[400px] border border-darkgray rounded p-4">
                <img src="{{ $selectedAgent['image'] }}" alt="{{ $selectedAgent['name'] }}"
                     class="w-[100px] h-[100px] rounded-full object-cover">

                <div>
                    <h3 class="mt-4">{{ $selectedAgent['name'] }}</h3>
                    <span class="inline-flex items-center my-1 px-1 py-1 {{ $selectedAgent['is_rag_ready'] == false && $selectedAgent['created_at']->diffInMinutes() > 30 ? 'bg-red text-white' : ($selectedAgent['is_rag_ready'] ? 'bg-white text-black' : 'bg-yellow-500 text-black') }}    text-xs font-bold rounded-md">
                        {{ $selectedAgent['is_rag_ready'] == false && $selectedAgent['created_at']->diffInMinutes() > 30 ? 'Error' : ($selectedAgent['is_rag_ready'] ? 'Ready' : 'Building') }}
                    </span>
                </div>
                <p class="text-[14px] text-gray mb-0">{{ $selectedAgent['description'] }}</p>
                @if (!empty($selectedAgent['capabilities']))
                    <p class="text-[14px] text-gray mb-0">
                        {{ json_encode($selectedAgent['capabilities']) }}</p>
                @endif
            </div>
        </div>
    </div>
</div>
