import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../wayfinder'
/**
* @see \App\Http\Controllers\ChatApiController::stream
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
export const stream = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream.url(options),
    method: 'post',
})

stream.definition = {
    methods: ["post"],
    url: '/api/chat',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\ChatApiController::stream
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
stream.url = (options?: RouteQueryOptions) => {
    return stream.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\ChatApiController::stream
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
stream.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: stream.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\ChatApiController::stream
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
const streamForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\ChatApiController::stream
* @see app/Http/Controllers/ChatApiController.php:21
* @route '/api/chat'
*/
streamForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: stream.url(options),
    method: 'post',
})

stream.form = streamForm

const ChatApiController = { stream }

export default ChatApiController