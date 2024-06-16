<ol class="z-[9990] ml-6 sm:ml-4 lg:ml-0 lg:ms-3 flex items-center whitespace-nowrap" aria-label="Breadcrumb">
    <li class="text-sm font-semibold text-gray-800 truncate select-none" x-data="{ dropdown: false }"
        aria-current="page">
        <livewire:model-dropdown :selected-agent="$this->thread->agent"
                                 :selected-model="$this->thread->model" :models="$models" action="selectModel"
                                 :show-agents="true"/>
    </li>
</ol>
