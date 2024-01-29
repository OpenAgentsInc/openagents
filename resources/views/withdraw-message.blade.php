@fragment('withdraw-message')
@if (isset($successMessage))
    <div class="bg-teal-100 border border-teal-400 text-teal-700 px-4 py-3 rounded relative" role="alert">
        <span class="block sm:inline">{{ $successMessage }}</span>
    </div>
@elseif (isset($errorMessage))
    <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded relative" role="alert">
        <span class="block sm:inline">{{ $errorMessage }}</span>
    </div>
@endif
@endfragment
