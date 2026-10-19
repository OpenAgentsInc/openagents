// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.push

/** Push-enabled app source set. */
object AndroidPushFlavor {
    const val enabled = true
    fun currentToken(result: (String?) -> Unit) {
        try {
            com.google.firebase.messaging.FirebaseMessaging.getInstance().token.addOnCompleteListener { task ->
                result(if (task.isSuccessful) task.result else null)
            }
        } catch (_: Exception) { result(null) }
    }
}
