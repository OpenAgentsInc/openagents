// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.push

/** Push-disabled app source set; this source has no Firebase dependency. */
object AndroidPushFlavor {
    const val enabled = false
    fun currentToken(result: (String?) -> Unit) { result(null) }
}
