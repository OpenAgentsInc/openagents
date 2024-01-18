@if(count($errors) > 0)
    <div class="">
        <strong>Whoops!</strong> There were some problems with your plugin upload.<br><br>
        <ul>
            @foreach($errors->all() as $error)
                <li>{{ $error }}</li>
            @endforeach
        </ul>
    </div>
@endif
