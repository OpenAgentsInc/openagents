<div>




    <div class="w-full">
        <div class="flex gap-2 items-center  overflow-hidden" x-bind:class="{
            'justify-between': sidebarOpen,
            'justify-center': collapsed
           }">
            <button class="z-50 absolute top-0 left-0 cursor-pointer h-[28px] w-[28px] m-4 mt-[18px] mr-12" @click="sidebarOpen = !sidebarOpen">
                <x-icon.menu />
            </button>


            <div  class="relative flex-1 text-right" x-data="{ dropdown: false }">
                    <button @click="dropdown= !dropdown" x-cloak  x-show="sidebarOpen" class="mt-4 p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                        <x-icon.plus class="h-6 w-6"></x-icon.plus>
                    </button>
            </div>
        </div>
    </div>

    <div class="mt-24 w-[260px] p-4">
        <ul x-cloak x-bind:class="{
        'hidden': !sidebarOpen
       }" x-bind:class="{
<div>


    <div class="w-full">
        <div class="flex gap-2 items-center  overflow-hidden" x-bind:class="{
            'justify-between': sidebarOpen,
            'justify-center': collapsed
           }">
            <button class="z-50 absolute top-0 left-0 cursor-pointer h-[28px] w-[28px] m-4 mt-[18px] mr-12" @click="sidebarOpen = !sidebarOpen">
                <x-icon.menu />
            </button>


            <div  class="relative flex-1 text-right" x-data="{ dropdown: false }">
                    <button @click="dropdown= !dropdown" x-cloak  x-show="sidebarOpen" class="mt-4 p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                        <x-icon.plus class="h-6 w-6"></x-icon.plus>
                    </button>
            </div>
        </div>
    </div>

    <div class="flex flex-col gap-2 mt-8 py-4 px-1">
        <span class="text-left text-sm text-[#777A82] px-2" x-cloak>
            Today
        </span>
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
    </div>
</div>
