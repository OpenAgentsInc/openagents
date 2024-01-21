<?php

namespace App\Http\Controllers;

use App\Models\User;
use GrahamCampbell\Markdown\Facades\Markdown;
use Inertia\Inertia;

class StaticController extends Controller
{
    public function newsplash()
    {
        return view('splash');
    }

    public function blog()
    {
        $markdownContent = file_get_contents(resource_path('blog/page.md'));
        $htmlContent = Markdown::convert($markdownContent)->getContent();

        return view('blog', ['htmlContent' => $htmlContent]);
    }

    public function thesis()
    {
        return view('thesis');
    }

    public function splash()
    {
        return Inertia::render('Splash');
    }

    public function terms()
    {
        return Inertia::render('Terms');
    }

    public function privacy()
    {
        return Inertia::render('Privacy');
    }

    public function epstein()
    {
        return redirect('/agent/2');
    }
}
