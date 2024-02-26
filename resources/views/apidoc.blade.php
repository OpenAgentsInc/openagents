<x-apidoc-layout>
    <livewire:navbar />

    <div class="pt-24" />
    <p>CURL: {{ $content->curl }}</p>
    <p>Response 200: {{ $content->responses[0]["200"] }}</p>
    <p>Response 400: {{ $content->responses[1]["400"] }}</p>

    <div class="docs w-full max-w-5xl px-12 py-20">
        {!! $content->contents !!}
    </div>
</x-apidoc-layout>
