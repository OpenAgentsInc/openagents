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

        return view('components.social-tags', [
            'imageUrl' => $imageUrl,
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
            // Add more paths and their corresponding image URLs here
            // For example: 'about' => 'https://openagents.com/images/about.png',
        ];

        return $imagePaths[$currentPath] ?? 'https://openagents.com/images/openagents.png';
    }
}
