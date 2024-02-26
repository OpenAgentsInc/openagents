<x-apidoc-layout>
    <livewire:navbar />

    <div class="pt-24 px-24">

        <h3>Curl:</h3>
        {!! $curl !!}

        <h3 class="mt-8">Responses:</h3>
        <p>Response 200:</p>
        {!! $responseSuccessHtml !!}

        <p>Response 400:</p>
        {!! $responseErrorHtml !!}

    </div>

    <div class="docs w-full max-w-5xl px-12 py-20">
        {!! $content->contents !!}
    </div>
</x-apidoc-layout>
