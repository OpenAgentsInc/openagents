<?php

namespace App\Http\Controllers;

class CampaignController extends Controller
{
    public function land($id)
    {
        // Generate a unique subid
        $subid = uniqid();

        // Save a session var with id and subid separated by dots
        $var = $id.'.'.$subid;

        session()->put('campaign_subid', $var);

        return redirect()->route('home');
    }
}
