<?php

namespace App\Livewire;

use Illuminate\Support\Str;
use Laravel\Jetstream\Jetstream;
use Livewire\Component;

class MarkdownPage extends Component
{
    public $markdownContent;

    public $includeTwitterSdk = false;

    public function mount()
    {
        $markdownFile = Jetstream::localizedMarkdownPath(request()->path().'.md');
        $this->markdownContent = Str::markdown(file_get_contents($markdownFile));

        if (Str::contains($this->markdownContent, 'twitter-tweet')) {
            $this->includeTwitterSdk = true;
        }
    }

    public function render()
    {
        return view('livewire.markdown-page');
    }
}
