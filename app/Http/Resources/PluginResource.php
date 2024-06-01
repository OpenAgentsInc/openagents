<?php

namespace App\Http\Resources;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

class PluginResource extends JsonResource
{
    /**
     * The "data" wrapper that should be applied.
     *
     * @var string
     */
    public static $wrap = null;

    /**
     * Transform the resource into an array.
     *
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {

        // Prepare the sockets data
        $sockets = [
            'in' => json_decode($this->input_template, true),
            'out' =>  json_decode($this->output_template, true)
        ];



        return [
            'meta' => [
                'kind' => 5003,
                'name' => $this->name,
                'description' => $this->description,
                'tos' => $this->tos,
                'privacy' => $this->privacy,
                'author' => $this->author ,
                'web' => $this->web,
                'picture' => $this->picture ?? '',
                'tags' => ['tool'],
                'payment' => 'lightning:'.($this->payment ? $this->payment : $this->user->lightning_address),
            ],
            'mini-template' => [
                'main' => $this->file_link,
                'input' => $this->plugin_input,
            ],
            'sockets' => $sockets,
        ];
    }
}
