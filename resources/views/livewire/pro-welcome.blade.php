<div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
    <div class="min-h-[80vh] flex flex-col items-center pt-6 sm:pt-0">
        <a href="/" wire:navigate class="mt-12">
            <x-icon.logo class="w-20 h-20 text-white"/>
        </a>

        <h1 class="mt-8 text-center">You're a pro!</h1>

        <p class="text-gray">A message from the founder </p>

        <div class="w-full sm:max-w-2xl p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
            <div style="padding:56.25% 0 0 0;position:relative;">
                <iframe src="https://player.vimeo.com/video/932242103?badge=0&amp;autopause=0&amp;player_id=0&amp;app_id=58479"
                        frameborder="0" allow="autoplay; fullscreen; picture-in-picture; clipboard-write"
                        style="position:absolute;top:0;left:0;width:100%;height:100%;"
                        title="You're a pro."></iframe>
            </div>
        </div>

        <p class="text-text"><a href="https://twitter.com/OpenAgentsInc" target="_blank"
                                class="text-white underline">DM or tag
                @OpenAgentsInc on
                X</a> any
            complaints/insults/requests</p>
    </div>

    <script src="https://player.vimeo.com/api/player.js"></script>
    @include('partials.x-pixel')

    <script type="text/javascript">
        twq('event', 'tw-om4wz-om4xz', {
            email_address: '{{auth()->user()->email}}'
        });
    </script>
</div>