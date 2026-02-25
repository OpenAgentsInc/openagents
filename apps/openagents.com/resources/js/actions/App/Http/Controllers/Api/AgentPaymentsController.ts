import { queryParams, type RouteQueryOptions, type RouteDefinition, type RouteFormDefinition } from './../../../../../wayfinder'
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
const wallet71796d51f6df38c20ba8217104f03f39 = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'get',
})

wallet71796d51f6df38c20ba8217104f03f39.definition = {
    methods: ["get","head"],
    url: '/api/agent-payments/wallet',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
wallet71796d51f6df38c20ba8217104f03f39.url = (options?: RouteQueryOptions) => {
    return wallet71796d51f6df38c20ba8217104f03f39.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
wallet71796d51f6df38c20ba8217104f03f39.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
wallet71796d51f6df38c20ba8217104f03f39.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: wallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
const wallet71796d51f6df38c20ba8217104f03f39Form = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
wallet71796d51f6df38c20ba8217104f03f39Form.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agent-payments/wallet'
*/
wallet71796d51f6df38c20ba8217104f03f39Form.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet71796d51f6df38c20ba8217104f03f39.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

wallet71796d51f6df38c20ba8217104f03f39.form = wallet71796d51f6df38c20ba8217104f03f39Form
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
const wallet629cbc97c2b6b633b9ddd2ec0190e17a = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'get',
})

wallet629cbc97c2b6b633b9ddd2ec0190e17a.definition = {
    methods: ["get","head"],
    url: '/api/agents/me/wallet',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
wallet629cbc97c2b6b633b9ddd2ec0190e17a.url = (options?: RouteQueryOptions) => {
    return wallet629cbc97c2b6b633b9ddd2ec0190e17a.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
wallet629cbc97c2b6b633b9ddd2ec0190e17a.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
wallet629cbc97c2b6b633b9ddd2ec0190e17a.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
const wallet629cbc97c2b6b633b9ddd2ec0190e17aForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
wallet629cbc97c2b6b633b9ddd2ec0190e17aForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::wallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:36
* @route '/api/agents/me/wallet'
*/
wallet629cbc97c2b6b633b9ddd2ec0190e17aForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: wallet629cbc97c2b6b633b9ddd2ec0190e17a.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

wallet629cbc97c2b6b633b9ddd2ec0190e17a.form = wallet629cbc97c2b6b633b9ddd2ec0190e17aForm

export const wallet = {
    '/api/agent-payments/wallet': wallet71796d51f6df38c20ba8217104f03f39,
    '/api/agents/me/wallet': wallet629cbc97c2b6b633b9ddd2ec0190e17a,
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agent-payments/wallet'
*/
const upsertWallet71796d51f6df38c20ba8217104f03f39 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertWallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'post',
})

upsertWallet71796d51f6df38c20ba8217104f03f39.definition = {
    methods: ["post"],
    url: '/api/agent-payments/wallet',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agent-payments/wallet'
*/
upsertWallet71796d51f6df38c20ba8217104f03f39.url = (options?: RouteQueryOptions) => {
    return upsertWallet71796d51f6df38c20ba8217104f03f39.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agent-payments/wallet'
*/
upsertWallet71796d51f6df38c20ba8217104f03f39.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertWallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agent-payments/wallet'
*/
const upsertWallet71796d51f6df38c20ba8217104f03f39Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertWallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agent-payments/wallet'
*/
upsertWallet71796d51f6df38c20ba8217104f03f39Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertWallet71796d51f6df38c20ba8217104f03f39.url(options),
    method: 'post',
})

upsertWallet71796d51f6df38c20ba8217104f03f39.form = upsertWallet71796d51f6df38c20ba8217104f03f39Form
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agents/me/wallet'
*/
const upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'post',
})

upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.definition = {
    methods: ["post"],
    url: '/api/agents/me/wallet',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agents/me/wallet'
*/
upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.url = (options?: RouteQueryOptions) => {
    return upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agents/me/wallet'
*/
upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agents/me/wallet'
*/
const upsertWallet629cbc97c2b6b633b9ddd2ec0190e17aForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::upsertWallet
* @see app/Http/Controllers/Api/AgentPaymentsController.php:67
* @route '/api/agents/me/wallet'
*/
upsertWallet629cbc97c2b6b633b9ddd2ec0190e17aForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.url(options),
    method: 'post',
})

upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a.form = upsertWallet629cbc97c2b6b633b9ddd2ec0190e17aForm

export const upsertWallet = {
    '/api/agent-payments/wallet': upsertWallet71796d51f6df38c20ba8217104f03f39,
    '/api/agents/me/wallet': upsertWallet629cbc97c2b6b633b9ddd2ec0190e17a,
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
const balance6376fd3e6d351c3821af094b6750298b = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: balance6376fd3e6d351c3821af094b6750298b.url(options),
    method: 'get',
})

balance6376fd3e6d351c3821af094b6750298b.definition = {
    methods: ["get","head"],
    url: '/api/agent-payments/balance',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
balance6376fd3e6d351c3821af094b6750298b.url = (options?: RouteQueryOptions) => {
    return balance6376fd3e6d351c3821af094b6750298b.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
balance6376fd3e6d351c3821af094b6750298b.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: balance6376fd3e6d351c3821af094b6750298b.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
balance6376fd3e6d351c3821af094b6750298b.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: balance6376fd3e6d351c3821af094b6750298b.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
const balance6376fd3e6d351c3821af094b6750298bForm = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6376fd3e6d351c3821af094b6750298b.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
balance6376fd3e6d351c3821af094b6750298bForm.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6376fd3e6d351c3821af094b6750298b.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agent-payments/balance'
*/
balance6376fd3e6d351c3821af094b6750298bForm.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6376fd3e6d351c3821af094b6750298b.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

balance6376fd3e6d351c3821af094b6750298b.form = balance6376fd3e6d351c3821af094b6750298bForm
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
const balance6c41a44bc900d5b5175193d92c8f3c99 = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: balance6c41a44bc900d5b5175193d92c8f3c99.url(options),
    method: 'get',
})

balance6c41a44bc900d5b5175193d92c8f3c99.definition = {
    methods: ["get","head"],
    url: '/api/agents/me/balance',
} satisfies RouteDefinition<["get","head"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
balance6c41a44bc900d5b5175193d92c8f3c99.url = (options?: RouteQueryOptions) => {
    return balance6c41a44bc900d5b5175193d92c8f3c99.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
balance6c41a44bc900d5b5175193d92c8f3c99.get = (options?: RouteQueryOptions): RouteDefinition<'get'> => ({
    url: balance6c41a44bc900d5b5175193d92c8f3c99.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
balance6c41a44bc900d5b5175193d92c8f3c99.head = (options?: RouteQueryOptions): RouteDefinition<'head'> => ({
    url: balance6c41a44bc900d5b5175193d92c8f3c99.url(options),
    method: 'head',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
const balance6c41a44bc900d5b5175193d92c8f3c99Form = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6c41a44bc900d5b5175193d92c8f3c99.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
balance6c41a44bc900d5b5175193d92c8f3c99Form.get = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6c41a44bc900d5b5175193d92c8f3c99.url(options),
    method: 'get',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::balance
* @see app/Http/Controllers/Api/AgentPaymentsController.php:114
* @route '/api/agents/me/balance'
*/
balance6c41a44bc900d5b5175193d92c8f3c99Form.head = (options?: RouteQueryOptions): RouteFormDefinition<'get'> => ({
    action: balance6c41a44bc900d5b5175193d92c8f3c99.url({
        [options?.mergeQuery ? 'mergeQuery' : 'query']: {
            _method: 'HEAD',
            ...(options?.query ?? options?.mergeQuery ?? {}),
        }
    }),
    method: 'get',
})

balance6c41a44bc900d5b5175193d92c8f3c99.form = balance6c41a44bc900d5b5175193d92c8f3c99Form

export const balance = {
    '/api/agent-payments/balance': balance6376fd3e6d351c3821af094b6750298b,
    '/api/agents/me/balance': balance6c41a44bc900d5b5175193d92c8f3c99,
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/agent-payments/invoice'
*/
const createInvoice4f80a1fdc2d2525413e003e4ba425fe2 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: createInvoice4f80a1fdc2d2525413e003e4ba425fe2.url(options),
    method: 'post',
})

createInvoice4f80a1fdc2d2525413e003e4ba425fe2.definition = {
    methods: ["post"],
    url: '/api/agent-payments/invoice',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/agent-payments/invoice'
*/
createInvoice4f80a1fdc2d2525413e003e4ba425fe2.url = (options?: RouteQueryOptions) => {
    return createInvoice4f80a1fdc2d2525413e003e4ba425fe2.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/agent-payments/invoice'
*/
createInvoice4f80a1fdc2d2525413e003e4ba425fe2.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: createInvoice4f80a1fdc2d2525413e003e4ba425fe2.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/agent-payments/invoice'
*/
const createInvoice4f80a1fdc2d2525413e003e4ba425fe2Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: createInvoice4f80a1fdc2d2525413e003e4ba425fe2.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/agent-payments/invoice'
*/
createInvoice4f80a1fdc2d2525413e003e4ba425fe2Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: createInvoice4f80a1fdc2d2525413e003e4ba425fe2.url(options),
    method: 'post',
})

createInvoice4f80a1fdc2d2525413e003e4ba425fe2.form = createInvoice4f80a1fdc2d2525413e003e4ba425fe2Form
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/payments/invoice'
*/
const createInvoice1876607da20c9a189d6df008ab2ea833 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: createInvoice1876607da20c9a189d6df008ab2ea833.url(options),
    method: 'post',
})

createInvoice1876607da20c9a189d6df008ab2ea833.definition = {
    methods: ["post"],
    url: '/api/payments/invoice',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/payments/invoice'
*/
createInvoice1876607da20c9a189d6df008ab2ea833.url = (options?: RouteQueryOptions) => {
    return createInvoice1876607da20c9a189d6df008ab2ea833.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/payments/invoice'
*/
createInvoice1876607da20c9a189d6df008ab2ea833.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: createInvoice1876607da20c9a189d6df008ab2ea833.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/payments/invoice'
*/
const createInvoice1876607da20c9a189d6df008ab2ea833Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: createInvoice1876607da20c9a189d6df008ab2ea833.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::createInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:160
* @route '/api/payments/invoice'
*/
createInvoice1876607da20c9a189d6df008ab2ea833Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: createInvoice1876607da20c9a189d6df008ab2ea833.url(options),
    method: 'post',
})

createInvoice1876607da20c9a189d6df008ab2ea833.form = createInvoice1876607da20c9a189d6df008ab2ea833Form

export const createInvoice = {
    '/api/agent-payments/invoice': createInvoice4f80a1fdc2d2525413e003e4ba425fe2,
    '/api/payments/invoice': createInvoice1876607da20c9a189d6df008ab2ea833,
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/agent-payments/pay'
*/
const payInvoice952d860a437cff87a373ab8a09555e5f = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: payInvoice952d860a437cff87a373ab8a09555e5f.url(options),
    method: 'post',
})

payInvoice952d860a437cff87a373ab8a09555e5f.definition = {
    methods: ["post"],
    url: '/api/agent-payments/pay',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/agent-payments/pay'
*/
payInvoice952d860a437cff87a373ab8a09555e5f.url = (options?: RouteQueryOptions) => {
    return payInvoice952d860a437cff87a373ab8a09555e5f.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/agent-payments/pay'
*/
payInvoice952d860a437cff87a373ab8a09555e5f.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: payInvoice952d860a437cff87a373ab8a09555e5f.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/agent-payments/pay'
*/
const payInvoice952d860a437cff87a373ab8a09555e5fForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: payInvoice952d860a437cff87a373ab8a09555e5f.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/agent-payments/pay'
*/
payInvoice952d860a437cff87a373ab8a09555e5fForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: payInvoice952d860a437cff87a373ab8a09555e5f.url(options),
    method: 'post',
})

payInvoice952d860a437cff87a373ab8a09555e5f.form = payInvoice952d860a437cff87a373ab8a09555e5fForm
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/payments/pay'
*/
const payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.url(options),
    method: 'post',
})

payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.definition = {
    methods: ["post"],
    url: '/api/payments/pay',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/payments/pay'
*/
payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.url = (options?: RouteQueryOptions) => {
    return payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/payments/pay'
*/
payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/payments/pay'
*/
const payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::payInvoice
* @see app/Http/Controllers/Api/AgentPaymentsController.php:213
* @route '/api/payments/pay'
*/
payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.url(options),
    method: 'post',
})

payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7.form = payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7Form

export const payInvoice = {
    '/api/agent-payments/pay': payInvoice952d860a437cff87a373ab8a09555e5f,
    '/api/payments/pay': payInvoice6c5c3dec5e34b4bf78aa15c3b78077b7,
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/agent-payments/send-spark'
*/
const sendSpark4ab236ad9fd80df05ccd4d39ba56f86c = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.url(options),
    method: 'post',
})

sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.definition = {
    methods: ["post"],
    url: '/api/agent-payments/send-spark',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/agent-payments/send-spark'
*/
sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.url = (options?: RouteQueryOptions) => {
    return sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/agent-payments/send-spark'
*/
sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/agent-payments/send-spark'
*/
const sendSpark4ab236ad9fd80df05ccd4d39ba56f86cForm = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/agent-payments/send-spark'
*/
sendSpark4ab236ad9fd80df05ccd4d39ba56f86cForm.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.url(options),
    method: 'post',
})

sendSpark4ab236ad9fd80df05ccd4d39ba56f86c.form = sendSpark4ab236ad9fd80df05ccd4d39ba56f86cForm
/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/payments/send-spark'
*/
const sendSparka0fab0c7d9aa9b9e093aee653b3c7e97 = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.url(options),
    method: 'post',
})

sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.definition = {
    methods: ["post"],
    url: '/api/payments/send-spark',
} satisfies RouteDefinition<["post"]>

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/payments/send-spark'
*/
sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.url = (options?: RouteQueryOptions) => {
    return sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.definition.url + queryParams(options)
}

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/payments/send-spark'
*/
sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.post = (options?: RouteQueryOptions): RouteDefinition<'post'> => ({
    url: sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/payments/send-spark'
*/
const sendSparka0fab0c7d9aa9b9e093aee653b3c7e97Form = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.url(options),
    method: 'post',
})

/**
* @see \App\Http\Controllers\Api\AgentPaymentsController::sendSpark
* @see app/Http/Controllers/Api/AgentPaymentsController.php:309
* @route '/api/payments/send-spark'
*/
sendSparka0fab0c7d9aa9b9e093aee653b3c7e97Form.post = (options?: RouteQueryOptions): RouteFormDefinition<'post'> => ({
    action: sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.url(options),
    method: 'post',
})

sendSparka0fab0c7d9aa9b9e093aee653b3c7e97.form = sendSparka0fab0c7d9aa9b9e093aee653b3c7e97Form

export const sendSpark = {
    '/api/agent-payments/send-spark': sendSpark4ab236ad9fd80df05ccd4d39ba56f86c,
    '/api/payments/send-spark': sendSparka0fab0c7d9aa9b9e093aee653b3c7e97,
}

const AgentPaymentsController = { wallet, upsertWallet, balance, createInvoice, payInvoice, sendSpark }

export default AgentPaymentsController