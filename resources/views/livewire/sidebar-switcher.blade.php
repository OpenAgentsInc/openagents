<div>
    <div x-data="{ currentPage: 'chat' }">
        <ul class="flex gap-4 my-5 px-12 justify-center">
          <li :class="{ 'text-gray-700 font-bold': currentPage === 'chat' }">
            <a href="#" @click.prevent="currentPage = 'chat'">Chat</a>
          </li>
          <li :class="{ 'text-gray-700 font-bold': currentPage === 'index' }">
            <a href="#" @click.prevent="currentPage = 'index'">Agents</a>
          </li>
        </ul>
        <div x-show="currentPage === 'chat'">
            <div>
                
            </div>
        </div>
        <div x-show="currentPage === 'index'">
            {{-- <div x-show="currentPage === 'page2'">
                @include('livewire.agents.index') 
              </div> --}}
              --
        </div>
      </div>
</div>
