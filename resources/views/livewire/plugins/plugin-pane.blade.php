<div class="m-16">
    <x-pane title="{{$this->plugin['name']}}">
        <p>
            <b> Description: </b>
            <i>"{{$this->plugin['description']}}"</i>
        </p>

        <a href="{{$this->plugin['url']}}" target="_blank">
            <b>Source code: </b>
            {{$this->plugin['url']}}
        </a>

        <p>
            <b>Author: </b>
            {{$this->plugin['author']}}
        </p>
    </x-pane>
</div>