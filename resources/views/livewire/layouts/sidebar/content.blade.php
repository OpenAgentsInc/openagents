<div>
    {{-- To attain knowledge, add things every day; To attain wisdom, subtract things every day. --}}
    <ul x-bind:class="{
        'my-2 flex flex-col gap-2 items-stretch': true,
        'rounded-md p-2 mx- gap-4': !collapsed,
        'rounded-full p-2 mx- w-10 h-10': collapsed
     }">

        <li>
            <span class="text-left font-sm text-[#777A82] px-4" x-show="!collapsed">
                Recent
            </span>
        </li>
        <li x-bind:class="{
      'text-white hover:bg-white/15 bg-black   flex transition-colors duration-300': true
         }">
            <a href="#" class="flex gap-2 p-3 rounded">

                <span x-show="!collapsed"> Why we did it</span>
            </a>
        </li>
    </ul>
</div>
