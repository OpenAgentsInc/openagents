@props(['plugin'])

    <a href="/plugin/{{ $plugin->id }}">
        <p>{{ $plugin->name }}</p>
        <p>{{ $plugin->description }}</p>
        <p>{{ $plugin->fee }}</p>
        <p>{{ $plugin->created_at->format('M d, Y') }}</p>
    </a>
