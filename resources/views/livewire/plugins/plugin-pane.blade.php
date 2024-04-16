<div class="m-16">
    {{-- The whole world belongs to you. --}}
    <x-pane title="{{$this->plugin['name']}}">

        <p>
            <b> Description: </b>
            <i>"{{$this->plugin['description']}}"</i>
        </p>

        <a href="{{$this->plugin['url']}}">
            <b>Source code: </b>
            {{$this->plugin['url']}}
        </a>

        <p> 
            <b>Author: </b> 
            {{$this->plugin['author']}}
        </p>
    </x-pane>
</div>