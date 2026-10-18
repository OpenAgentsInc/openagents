// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.os.Looper
import android.os.SystemClock
import com.google.android.gms.tasks.Tasks
import com.google.android.play.core.integrity.IntegrityManagerFactory
import com.google.android.play.core.integrity.StandardIntegrityException
import com.google.android.play.core.integrity.StandardIntegrityManager.PrepareIntegrityTokenRequest
import com.google.android.play.core.integrity.StandardIntegrityManager.StandardIntegrityTokenRequest
import com.google.android.play.core.integrity.model.StandardIntegrityErrorCode
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import uniffi.openagents_mobile.PlayIntegrityAttester
import uniffi.openagents_mobile.PlayIntegrityResult

/**
 * H1 platform adapter. B1 must include this source directory and
 * com.google.android.play:integrity:1.5.0, generate the UniFFI bindings, and
 * call CoreAccount.setPlayIntegrityAttester before startIdentity.
 * The linked Play Cloud project number is build configuration, not a secret.
 * Keep this adapter unregistered in offline/demo builds and until rollout.
 * Rust chooses fallback, signs the token body, and sends it through the SDK.
 */
class AndroidPlayIntegrityAttester(
    context: Context,
    private val cloudProjectNumber: Long,
) : PlayIntegrityAttester {
    private val manager = IntegrityManagerFactory.createStandard(context.applicationContext)

    override fun attest(requestHash: String): PlayIntegrityResult {
        if (cloudProjectNumber <= 0 || Looper.myLooper() == Looper.getMainLooper()) {
            return PlayIntegrityResult.Unavailable
        }
        // Only the Rust-produced digest crosses into Google Play.
        if (!requestHash.matches(Regex("[0-9a-f]{64}"))) {
            return PlayIntegrityResult.Refused
        }
        val deadline = SystemClock.elapsedRealtime() + 8_000
        fun remaining(): Long {
            val value = deadline - SystemClock.elapsedRealtime()
            if (value <= 0) throw TimeoutException()
            return value
        }
        return try {
            val provider = Tasks.await(
                manager.prepareIntegrityToken(
                    PrepareIntegrityTokenRequest.builder()
                        .setCloudProjectNumber(cloudProjectNumber)
                        .build(),
                ),
                remaining(),
                TimeUnit.MILLISECONDS,
            )
            val response = Tasks.await(
                provider.request(
                    StandardIntegrityTokenRequest.builder()
                        .setRequestHash(requestHash)
                        .build(),
                ),
                remaining(),
                TimeUnit.MILLISECONDS,
            )
            PlayIntegrityResult.Token(response.token())
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
            PlayIntegrityResult.Unavailable
        } catch (error: ExecutionException) {
            when ((error.cause as? StandardIntegrityException)?.errorCode) {
                StandardIntegrityErrorCode.APP_NOT_INSTALLED,
                StandardIntegrityErrorCode.APP_UID_MISMATCH,
                StandardIntegrityErrorCode.CLOUD_PROJECT_NUMBER_IS_INVALID -> PlayIntegrityResult.Refused
                else -> PlayIntegrityResult.Unavailable
            }
        } catch (_: Exception) {
            PlayIntegrityResult.Unavailable
        }
    }
}
