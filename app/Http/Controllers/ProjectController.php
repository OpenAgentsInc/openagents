<?php

namespace App\Http\Controllers;

use App\Models\Project;
use Illuminate\Http\Request;

class ProjectController extends Controller
{
    public function threads(Project $project)
    {
        $threads = $project->threads()->get();
        return response()->json($threads, 200);
    }
}