@props(['step'])

    <div
        {{ $attributes->merge(['class' => 'step-node bg-white shadow-lg rounded-lg p-4 mb-4']) }}>
        <div class="flex justify-between items-center mb-2">
            <h3 class="text-lg font-semibold">{{ $step->name }}</h3>
            <span class="text-sm text-gray-600">Order: {{ $step->order }}</span>
        </div>
        <div class="text-sm">
            <p><strong>Entry Type:</strong> {{ $step->entry_type }}</p>
            <p><strong>Category:</strong> {{ $step->category }}</p>
            <p><strong>Description:</strong> {{ $step->description }}</p>
            <p><strong>Error Message:</strong> {{ $step->error_message }}</p>
            <p><strong>Success Action:</strong> {{ $step->success_action }}</p>
            <p><strong>Params:</strong> {{ json_encode($step->params) }}</p>
        </div>
        <div class="mt-4">
            <button type="button" class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600">Edit</button>
            <button type="button" class="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600">Delete</button>
        </div>
    </div>
