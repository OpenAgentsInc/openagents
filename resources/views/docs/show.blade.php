<x-blank-layout>
    <livewire:navbar />
    <div class="flex justify-center min-h-screen">
        <div class="docs w-full max-w-5xl pt-20 px-12">
            {!! $content->contents !!}
        </div>
    </div>
</x-blank-layout>
