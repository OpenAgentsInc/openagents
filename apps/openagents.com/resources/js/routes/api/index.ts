import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../wayfinder'
import smoke from './smoke'
import auth from './auth'
/**
* @see \App\Http\Controllers\ChatApiController::chat
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
export const chat = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: chat.url(options),
    method: 'post',
})

chat.definition = {
    methods: ["post"],
    url: '/api/chat',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\ChatApiController::chat
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
chat.url = (options?: RouteQueryOptions) => {
    return chat.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\ChatApiController::chat
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
chat.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: chat.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\ChatApiController::chat
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
const chatForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: chat.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\ChatApiController::chat
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
chatForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: chat.url(options),
    method: 'post',
})

chat.form = chatForm

const api = {
    chat: Object.assign(chat, chat),
    smoke: Object.assign(smoke, smoke),
    auth: Object.assign(auth, auth),
}

export default api