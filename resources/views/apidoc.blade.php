<x-apidoc-layout>
    <livewire:navbar />

    <div class="pt-16 docs flex flex-wrap -mx-4 max-w-[95vw]">
        <div class="w-full lg:w-1/4 px-4">
            placeholder
        </div>

        <!-- Main Content -->
        <div class="w-full lg:w-1/2 px-4">
            <div class="px-24">
                {!! $content->contents !!}
            </div>
        </div>

        <!-- Right Sidebar for Curl and Responses -->
        <div class="w-full lg:w-1/4 px-4">
            <div class="sticky top-0 pt-12">
                <h3>Curl:</h3>
                {!! $curl !!}

                <h3 class="mt-8">Responses:</h3>
                <p>Response 200:</p>
                {!! $responseSuccessHtml !!}

                <p>Response 400:</p>
                {!! $responseErrorHtml !!}
            </div>
        </div>
    </div>
</x-apidoc-layout>
