@props(['step'])

    <x-card class="relative">
        <x-card-header>
            <x-card-title>{{ $step->name }}</x-card-title>
            <span class="text-sm text-gray-600">Order: {{ $step->order }}</span>
        </x-card-header>
        <x-card-content>
            <div class="text-sm">
                <p><strong>Entry Type:</strong> {{ $step->entry_type }}</p>
                <p><strong>Category:</strong> {{ $step->category }}</p>
                <p><strong>Description:</strong> {{ $step->description }}</p>
                <p><strong>Error Message:</strong> {{ $step->error_message }}</p>
                <p><strong>Success Action:</strong> {{ $step->success_action }}</p>
                <p><strong>Params:</strong> {{ json_encode($step->params) }}</p>
            </div>
        </x-card-content>
        <x-card-footer>
            <x-button variant="default" class="mr-2">Edit</x-button>
            <x-button variant="destructive">Delete</x-button>
        </x-card-footer>
    </x-card>
