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
        $imageUrl = $this->getImageUrl($currentPath);
        $description = $this->getDescription($currentPath);

        return view('components.social-tags', [
            'imageUrl' => $imageUrl,
            'description' => $description,
        ]);
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
            // Add more paths and their corresponding image URLs here
            // For example: 'about' => 'https://openagents.com/images/about.png',
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
            'goodbye-chatgpt' => 'We unsubscribe from ChatGPT.',
            // Add more paths and their corresponding descriptions here
            // For example: 'about' => 'Learn more about OpenAgents and our missiosn.',
        ];

        return $descriptions[$currentPath] ?? "It's the coolest AI chat. Literally 1000x better than the rest. Try it now or else.";
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
            // Add more paths and their corresponding titles here
            // For example: 'about' => 'About OpenAgents',
        ];

        return $titles[$currentPath] ?? 'Chat with OpenAgents';
    }
}
