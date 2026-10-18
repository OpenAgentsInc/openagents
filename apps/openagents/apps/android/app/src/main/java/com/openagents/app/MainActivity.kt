package com.openagents.app

import android.app.Activity
import android.graphics.Typeface
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import uniffi.openagents_core.CloudChatListener
import uniffi.openagents_core.CloudChatSnapshot
import uniffi.openagents_core.CoreCloudChat

class MainActivity : Activity() {
    private val mainHandler = Handler(Looper.getMainLooper())
    private lateinit var transcript: LinearLayout
    private lateinit var input: EditText
    private lateinit var send: Button
    private var demoChat: CoreCloudChat? = null
    private var transcriptScroll: ScrollView? = null

    private val listener = object : CloudChatListener {
        override fun onCloudChatChanged(snapshot: CloudChatSnapshot) {
            mainHandler.post {
                if (!isFinishing && !isDestroyed) render(snapshot)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.statusBarColor = getColor(R.color.oa_background)
        window.navigationBarColor = getColor(R.color.oa_background)
        val debugLaunch = DebugLaunchExtras.read(intent)
        setContentView(createScreen(debugLaunch))

        if (BuildConfig.DEBUG && debugLaunch.demo) {
            // The first shell screen is an offline Rust demo. It has no
            // account, network client, or saved conversation.
            demoChat = CoreCloudChat.demo(listener)
            demoChat?.snapshot()?.let(::render)
        }
    }

    override fun onDestroy() {
        mainHandler.removeCallbacksAndMessages(null)
        demoChat?.close()
        demoChat = null
        transcriptScroll = null
        super.onDestroy()
    }

    private fun createScreen(debugLaunch: DebugLaunchExtras): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(getColor(R.color.oa_background))
            setPadding(dp(24), dp(20), dp(24), dp(18))
        }
        if (BuildConfig.DEBUG) root.tag = "route=${debugLaunch.route};lab=${debugLaunch.lab}"
        root.addView(TextView(this).apply {
            text = getString(R.string.demo_title)
            setTextColor(getColor(R.color.oa_text))
            textSize = 26f
            typeface = face("Geist-SemiBold.ttf", Typeface.create("sans-serif-medium", Typeface.NORMAL))
        })
        if (BuildConfig.DEBUG && debugLaunch.demo) root.addView(TextView(this).apply {
            text = getString(R.string.demo_subtitle)
            setTextColor(getColor(R.color.oa_secondary))
            textSize = 15f
            typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
            setPadding(0, dp(5), 0, dp(12))
        })

        transcript = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, dp(12), 0, dp(12))
        }
        transcriptScroll = ScrollView(this).apply {
            isFillViewport = true
            addView(transcript)
        }
        root.addView(transcriptScroll, LinearLayout.LayoutParams(0, 0, 1f))

        val composer = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(4), dp(4), dp(4))
            setBackgroundColor(getColor(R.color.oa_surface))
        }
        input = EditText(this).apply {
            hint = getString(R.string.demo_message_hint)
            setTextColor(getColor(R.color.oa_text))
            setHintTextColor(getColor(R.color.oa_secondary))
            textSize = 16f
            typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
            setSingleLine(true)
            imeOptions = EditorInfo.IME_ACTION_SEND
            background = null
            setOnEditorActionListener { _, action, _ ->
                if (action == EditorInfo.IME_ACTION_SEND) { sendMessage(); true } else false
            }
        }
        composer.addView(input, LinearLayout.LayoutParams(0, dp(48), 1f))
        send = Button(this).apply {
            text = getString(R.string.demo_send)
            setTextColor(getColor(R.color.oa_background))
            setBackgroundColor(getColor(R.color.oa_accent))
            typeface = face("Geist-Medium.ttf", Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setOnClickListener { sendMessage() }
        }
        composer.addView(send, LinearLayout.LayoutParams(dp(76), dp(44)))
        if (BuildConfig.DEBUG && debugLaunch.demo) root.addView(composer)
        return root
    }

    private fun render(snapshot: CloudChatSnapshot) {
        transcript.removeAllViews()
        snapshot.lines.forEach { line ->
            val row = TextView(this).apply {
                text = line.text
                textSize = 16f
                setTextColor(getColor(R.color.oa_text))
                typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
                setPadding(dp(14), dp(12), dp(14), dp(12))
                setBackgroundColor(if (line.role.name == "USER") getColor(R.color.oa_surface) else getColor(R.color.oa_background))
            }
            transcript.addView(row, LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT)
        }
        if (snapshot.lines.isEmpty()) {
            transcript.addView(TextView(this).apply {
                text = "Try a question. This demo works without an internet connection."
                textSize = 16f
                setTextColor(getColor(R.color.oa_secondary))
                typeface = face("Geist.ttf", Typeface.create("sans-serif", Typeface.NORMAL))
                setPadding(dp(4), dp(12), dp(4), dp(12))
            })
        }
        send.isEnabled = !snapshot.answering
        transcriptScroll?.post { transcriptScroll?.fullScroll(View.FOCUS_DOWN) }
    }

    private fun sendMessage() {
        val text = input.text.toString().trim()
        if (text.isEmpty() || demoChat == null) return
        input.text.clear()
        demoChat?.send(text)
    }

    private fun face(asset: String, fallback: Typeface): Typeface =
        runCatching { Typeface.createFromAsset(assets, asset) }.getOrDefault(fallback)

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
}
