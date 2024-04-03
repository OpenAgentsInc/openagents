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



    <div class="mt-8 w-[260px] p-4">
        <ul x-cloak x-bind:class="{
        'hidden': !sidebarOpen
       }" x-bind:class="{
        'my-2 flex flex-col gap-2 items-stretch': true,
        'rounded-md p-2 mx- gap-4': !sidebarOpen,
        'rounded-full p-2 mx- w-10 h-10': sidebarOpen
     }">

     @if($threads)
     <li class="mt-4">
        <span class="text-left leading-6 font-sm text-sm text-[#777A82] px-2" x-cloak x-show="sidebarOpen">
            Today
        </span>
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
        @endif'
    </div>
</div>
