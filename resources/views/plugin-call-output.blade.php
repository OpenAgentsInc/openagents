@fragment('output')
    <div>
        @if(isset($output) && $output)
            <p class="text-green-600">{{ $output }}</p>
        @else
            <p class="text-red-600">No output or plugin call failed.</p>
        @endif
    </div>
@endfragment
