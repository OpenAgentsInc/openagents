// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.platform

import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Paint
import android.os.Build
import android.view.Choreographer
import android.view.View
import com.openagents.app.SemanticViewAdapter
import org.json.JSONObject
import uniffi.openagents_core.OrbPreset
import uniffi.openagents_core.OrbRenderer
import uniffi.openagents_core.VoiceCallState
import uniffi.openagents_core.VoiceOrb

/** Rust Native surface renderer for the shared Rust orb geometry. */
class AndroidVoiceOrbSurface(context: Context) : SemanticViewAdapter.Extension {
    private val appContext = context.applicationContext
    private var mounted: OrbView? = null

    override fun mount(node: JSONObject, previous: View?): View {
        val props = node.getJSONObject("element").getJSONObject("props")
        require(props.getString("resource") == RESOURCE)
        val view = (previous as? OrbView) ?: OrbView(appContext)
        mounted = view
        return view
    }

    override fun dispose(view: View) {
        (view as? OrbView)?.dispose()
        if (mounted === view) mounted = null
    }

    fun update(state: VoiceCallState) {
        mounted?.update(state.orb, state.microphone, state.speaker)
    }

    private class OrbView(context: Context) : View(context), Choreographer.FrameCallback {
        private val renderer = OrbRenderer(OrbPreset.HERO, VoiceOrb.CONNECTING)
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private var framePending = false
        private var disposed = false
        private var dark = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES

        init {
            contentDescription = "Voice activity"
            isFocusable = false
            postFrame()
        }

        fun update(orb: VoiceOrb, microphone: Float, speaker: Float) {
            renderer.setOrb(orb)
            renderer.setAudioLevels(microphone.coerceIn(0f, 1f), speaker.coerceIn(0f, 1f))
            invalidate()
        }

        override fun onConfigurationChanged(newConfig: Configuration?) {
            super.onConfigurationChanged(newConfig)
            dark = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        }

        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)
            val frame = renderer.nextFrame(isAttachedToWindow, Build.VERSION.SDK_INT >= 26 && !android.animation.ValueAnimator.areAnimatorsEnabled())
            val scale = minOf(width, height).toFloat() / frame.size
            val left = (width - frame.size * scale) / 2f
            val top = (height - frame.size * scale) / 2f
            val lines = frame.lines
            var i = 0
            while (i + 6 < lines.size) {
                val alpha = (lines[i + 6].coerceIn(0f, 1f) * 255).toInt()
                val white = lines[i + 5].coerceIn(0f, 1f)
                val intensity = ((if (dark) 1f - white else white) * 255).toInt()
                paint.color = android.graphics.Color.argb(alpha, intensity, intensity, intensity)
                paint.strokeWidth = lines[i + 4] * scale
                paint.strokeCap = Paint.Cap.ROUND
                canvas.drawLine(left + lines[i] * scale, top + lines[i + 1] * scale, left + lines[i + 2] * scale, top + lines[i + 3] * scale, paint)
                i += 7
            }
            val dots = frame.dots
            i = 0
            while (i + 4 < dots.size) {
                val alpha = (dots[i + 4].coerceIn(0f, 1f) * 255).toInt()
                val white = dots[i + 3].coerceIn(0f, 1f)
                val intensity = ((if (dark) 1f - white else white) * 255).toInt()
                paint.color = android.graphics.Color.argb(alpha, intensity, intensity, intensity)
                canvas.drawCircle(left + dots[i] * scale, top + dots[i + 1] * scale, dots[i + 2] * scale, paint)
                i += 5
            }
        }

        override fun doFrame(frameTimeNanos: Long) {
            framePending = false
            if (!disposed && isAttachedToWindow) {
                invalidate()
                postFrame()
            }
        }

        override fun onAttachedToWindow() {
            super.onAttachedToWindow()
            postFrame()
        }

        override fun onDetachedFromWindow() {
            Choreographer.getInstance().removeFrameCallback(this)
            framePending = false
            super.onDetachedFromWindow()
        }

        fun dispose() {
            disposed = true
            Choreographer.getInstance().removeFrameCallback(this)
            renderer.close()
        }

        private fun postFrame() {
            if (framePending || disposed) return
            framePending = true
            Choreographer.getInstance().postFrameCallback(this)
        }
    }

    companion object {
        const val RESOURCE = "voice-orb"
    }
}
