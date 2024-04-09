<?php

namespace App\View\Components;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class SocialTags extends Component
{
    /**
     * Get the view / contents that represent the component.
     *
     * @return View|Closure|string
     */
    public function render()
    {
        $currentPath = request()->path();
        $title = $this->getTitle($currentPath);
        $imageUrl = $this->getImageUrl($currentPath);
        $description = $this->getDescription($currentPath);

        return view('components.social-tags', [
            'imageUrl' => $imageUrl,
            'description' => $description,
            'title' => $title,
        ]);
    }

    /**
     * Get the title based on the current path.
     *
     * @param  string  $currentPath
     * @return string
     */
    protected function getTitle($currentPath)
    {
        $titles = [
            'launch' => 'One agent to rule them all',
            'goodbye-chatgpt' => 'Goodbye ChatGPT',
        ];

        return $titles[$currentPath] ?? 'Chat with OpenAgents';
    }

    /**
     * Get the image URL based on the current path.
     *
     * @param  string  $currentPath
     * @return string
     */
    protected function getImageUrl($currentPath)
    {
        $imagePaths = [
            'launch' => 'https://openagents.com/images/one.png',
            'goodbye-chatgpt' => 'https://openagents.com/images/goodbye-chatgpt.png',
        ];

        return $imagePaths[$currentPath] ?? 'https://openagents.com/images/openagents.png';
    }

    /**
     * Get the description based on the current path.
     *
     * @param  string  $currentPath
     * @return string
     */
    protected function getDescription($currentPath)
    {
        $descriptions = [
            'launch' => "It's the coolest AI chat for launching your product. Literally 1000x better than the rest. Try it now or else.",
            'goodbye-chatgpt' => 'We now launch a chat interface that can replace day-to-day use of ChatGPT.',
        ];

        return $descriptions[$currentPath] ?? "It's the coolest AI chat. Literally 1000x better than the rest. Try it now or else.";
    }
}
