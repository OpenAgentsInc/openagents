import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\Webhooks\ResendWebhookController::store
* @see app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15
* @route '/api/webhooks/resend'
*/
export const store = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

store.definition = {
    methods: ["post"],
    url: '/api/webhooks/resend',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\Webhooks\ResendWebhookController::store
* @see app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15
* @route '/api/webhooks/resend'
*/
store.url = (options?: RouteQueryOptions) => {
    return store.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\Webhooks\ResendWebhookController::store
* @see app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15
* @route '/api/webhooks/resend'
*/
store.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\Webhooks\ResendWebhookController::store
* @see app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15
* @route '/api/webhooks/resend'
*/
const storeForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\Webhooks\ResendWebhookController::store
* @see app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15
* @route '/api/webhooks/resend'
*/
storeForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: store.url(options),
    method: 'post',
})

store.form = storeForm

const ResendWebhookController = { store }

export default ResendWebhookController