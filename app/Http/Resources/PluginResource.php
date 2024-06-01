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


        $payment="";
        if($this->payment){
            $payment = "lightning:".$this->payment;
        }else if($this->user->lightning_address){
            $payment = "lightning:".$this->user->lightning_address;
        }else{
            // TODO: pay to user id?
        }

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
                'payment' => $payment,
            ],
            'mini-template' => [
                'main' => $this->file_link,
                'input' => $this->plugin_input,
            ],
            'sockets' => $sockets,
        ];
    }
}
