// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.Context
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import com.openagents.app.transcript.TranscriptCanvas
import java.io.Closeable

/** Places the Rust transcript geometry above the native composer and IME controls. */
class AndroidChatPresentation(
    context: Context,
    private val transcript: TranscriptCanvas,
    private val composer: AndroidComposerPresentation,
) : Closeable {
    private val root = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        addView(transcript, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        addView(composer.view(), LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
    }

    fun view(): View = root

    override fun close() {
        composer.close()
        transcript.close()
        root.removeAllViews()
    }
}
