@forelse($messages as $message)
    @include('partials.message', ['message' => $message])
@empty
    <x-empty-message-list />
@endforelse