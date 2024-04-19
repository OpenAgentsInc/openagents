<ol class="ml-16 md:ml-0 lg:ms-3 flex items-center whitespace-nowrap" aria-label="Breadcrumb">
    <li class="text-sm font-semibold text-gray-800 truncate select-none" aria-current="page">
        <x-model-dropdown :selectedModel="$this->formattedModel" :models="$models" action="selectModel"/>
    </li>
</ol>
