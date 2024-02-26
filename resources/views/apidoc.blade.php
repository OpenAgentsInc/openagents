<x-apidoc-layout>
    <livewire:navbar />

    <div class="py-16 docs flex flex-wrap -mx-4 max-w-[95vw]">
        <div class="w-full lg:w-1/4 px-4">
            placeholder
        </div>

        <div class="w-full lg:w-1/2 px-4">
            <div class="px-24">
                {!! $content->contents !!}
            </div>
        </div>

        <div class="w-full lg:w-1/4 px-4">
            <h3>Curl:</h3>
            {!! $curl !!}

            <h3 class="mt-8">Responses:</h3>
            <p>Response 200:</p>
            {!! $responseSuccessHtml !!}

            <p>Response 400:</p>
            {!! $responseErrorHtml !!}
        </div>
    </div>
</x-apidoc-layout>
