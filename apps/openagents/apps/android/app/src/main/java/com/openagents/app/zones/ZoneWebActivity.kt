package com.openagents.app.zones

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.View
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.openagents.app.OaStyle
import com.openagents.app.R
import com.openagents.app.SemanticViewAdapter
import uniffi.openagents_core.CoreZoneRunner
import uniffi.openagents_core.ZoneRunnerAction
import uniffi.openagents_core.ZoneRunnerPhase
import uniffi.openagents_core.ZoneWebPlatform

/**
 * The web runner (ADR 0010): one zone's web release in an
 * `android.webkit.WebView`. Rust decides what may load, whether this WebView
 * can run the zone (WebGPU), which card to show, and every bridge frame.
 * This activity forwards WebView events and paints the result.
 *
 * - Exact origin only: other navigations are cancelled.
 * - No credentials: no file or content access, no popups, no third-party
 *   cookies, and nothing from the app's account. Sign-in stays native.
 * - The `aethyrforge-host` bridge runs over `WebViewCompat.addWebMessageListener`,
 *   which gives the page `window.aethyrHost` only on the zone's origin.
 */
class ZoneWebActivity : ComponentActivity() {
    private lateinit var runner: CoreZoneRunner
    private lateinit var web: WebView
    private lateinit var cardHost: FrameLayout
    private lateinit var card: FrameLayout
    private lateinit var spinner: ProgressBar
    private lateinit var back: View
    private lateinit var adapter: SemanticViewAdapter
    private var closed = false

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val zoneId = intent.getStringExtra(EXTRA_ZONE)
        val opened = zoneId?.let { runCatching { Zones.core.runner(it) }.getOrNull() }
        if (opened == null || !WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            finish(); return
        }
        runner = opened
        fun dp(v: Int) = OaStyle.dp(this, v)
        val chrome = getColor(R.color.oa_chrome)
        val border = getColor(R.color.oa_chrome_border)

        web = WebView(this).apply {
            setBackgroundColor(Color.BLACK)
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.allowFileAccess = false
            settings.allowContentAccess = false
            settings.setSupportMultipleWindows(false)
            settings.javaScriptCanOpenWindowsAutomatically = false
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            settings.mediaPlaybackRequiresUserGesture = true
            contentDescription = runner.manifest().title
        }
        android.webkit.CookieManager.getInstance().setAcceptThirdPartyCookies(web, false)
        val origins = setOf(runner.origin())
        WebViewCompat.addWebMessageListener(web, runner.channelName(), origins) { _, message, origin, main, reply ->
            val text = message.data ?: return@addWebMessageListener
            val result = runner.bridge(origin.toString(), main, text, System.currentTimeMillis().toULong())
            for (frame in result.deliver) reply.postMessage(frame)
        }
        WebViewCompat.addWebMessageListener(web, runner.probeChannelName(), origins) { _, message, origin, main, _ ->
            val text = message.data ?: return@addWebMessageListener
            if (runCatching { runner.probe(origin.toString(), main, text) }.isSuccess) {
                val adapter = runner.gpuAdapter()
                Log.i(TAG, "zone ${runner.manifest().id} webgpu=${if (adapter != null) "available" else "missing"} " +
                    "vendor=${adapter?.vendor ?: ""} arch=${adapter?.architecture ?: ""} reason=${runner.gpuMissingReason() ?: ""} " +
                    "android=${Build.VERSION.RELEASE} sdk=${Build.VERSION.SDK_INT} webview=${WebViewCompat.getCurrentWebViewPackage(this)?.versionName ?: ""}")
            }
            refresh()
        }
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
                !runner.allowsNavigation(request.url.toString())
            override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) {
                runner.pageStarted(); refresh()
            }
            override fun onPageFinished(view: WebView, url: String?) {
                runner.pageFinished()
                // The probe reports `'gpu' in navigator` and `requestAdapter()` back over its channel.
                if (url != null && runner.allowsNavigation(url)) {
                    view.evaluateJavascript(runner.probeScript(ZoneWebPlatform.ANDROID), null)
                }
                refresh()
            }
            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (!request.isForMainFrame) return
                val offline = error.errorCode == ERROR_HOST_LOOKUP || error.errorCode == ERROR_CONNECT
                runner.pageFailed(offline); refresh()
            }
            override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                runner.pageFailed(false)
                closeWeb()
                refresh()
                return true
            }
        }

        val close = OaStyle.iconButton(this, R.drawable.oa_ic_close, "Close", 44, fill = chrome, stroke = border).apply {
            setOnClickListener { finishZone() }
        }
        back = OaStyle.iconButton(this, R.drawable.oa_ic_back, "Back", 44, fill = chrome, stroke = border).apply {
            setOnClickListener { if (web.canGoBack()) web.goBack() }
            isEnabled = false
            alpha = 0.4f
        }
        val title = TextView(this).apply {
            text = runner.manifest().title
            gravity = Gravity.CENTER
            OaStyle.text(this, 17f, OaStyle.Weight.SEMIBOLD)
        }
        val bar = FrameLayout(this).apply {
            setBackgroundColor(getColor(R.color.oa_background))
            setPadding(dp(12), dp(6), dp(12), dp(6))
            addView(title, FrameLayout.LayoutParams(-1, dp(44), Gravity.CENTER).apply { marginStart = dp(56); marginEnd = dp(56) })
            addView(back, FrameLayout.LayoutParams(dp(44), dp(44), Gravity.START or Gravity.CENTER_VERTICAL))
            addView(close, FrameLayout.LayoutParams(dp(44), dp(44), Gravity.END or Gravity.CENTER_VERTICAL))
        }
        card = FrameLayout(this)
        spinner = ProgressBar(this).apply { isIndeterminate = true }
        cardHost = FrameLayout(this).apply {
            setBackgroundColor(getColor(R.color.oa_background))
            isClickable = true
            val column = LinearLayout(this@ZoneWebActivity).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER_HORIZONTAL
                addView(spinner, LinearLayout.LayoutParams(dp(32), dp(32)).apply { bottomMargin = dp(16) })
                addView(card, LinearLayout.LayoutParams(-1, -2))
            }
            addView(column, FrameLayout.LayoutParams(-1, -2, Gravity.CENTER).apply { marginStart = dp(20); marginEnd = dp(20) })
        }
        val stage = FrameLayout(this).apply {
            addView(web, FrameLayout.LayoutParams(-1, -1))
            addView(cardHost, FrameLayout.LayoutParams(-1, -1))
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.BLACK)
            addView(bar, LinearLayout.LayoutParams(-1, -2))
            addView(stage, LinearLayout.LayoutParams(-1, 0, 1f))
        }
        // The bar sits below the status bar; the zone runs to the bottom and
        // side edges and pads itself, as on iOS.
        ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            bar.setPadding(dp(12) + bars.left, dp(6) + bars.top, dp(12) + bars.right, dp(6))
            cardHost.setPadding(bars.left, 0, bars.right, bars.bottom)
            insets
        }
        setContentView(root)
        adapter = SemanticViewAdapter(this,
            activate = { instance, revision, key -> activate(instance, revision.toULong(), key) },
            changeField = { _, _, _, _ -> },
            acknowledge = { _, _ -> },
        )
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (!closed && web.canGoBack()) web.goBack() else finishZone()
            }
        })
        load()
    }

    private fun load() {
        val url = runner.url()
        if (!runner.allowsNavigation(url)) { runner.pageFailed(false); refresh(); return }
        web.loadUrl(url)
        refresh()
    }

    /** Show the zone, or Rust's card over it. */
    private fun refresh() {
        if (isFinishing || isDestroyed) return
        val json = runCatching { runner.card() }.getOrNull()
        cardHost.visibility = if (json == null) View.GONE else View.VISIBLE
        spinner.visibility = if (runner.phase() is ZoneRunnerPhase.Loading) View.VISIBLE else View.GONE
        if (json != null) adapter.mount(card, json.toByteArray(Charsets.UTF_8))
        if (!closed) {
            back.isEnabled = web.canGoBack()
            back.alpha = if (back.isEnabled) 1f else 0.4f
        }
    }

    private fun activate(instance: String, revision: ULong, key: String) {
        val action = runCatching { runner.activate(instance, revision, key) }.getOrElse { refresh(); return }
        when (action) {
            is ZoneRunnerAction.Retry -> if (closed) recreate() else load()
            is ZoneRunnerAction.Close -> finishZone()
            is ZoneRunnerAction.OpenInBrowser ->
                runCatching { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(action.url))) }
        }
    }

    private fun closeWeb() {
        if (closed) return
        closed = true
        (web.parent as? FrameLayout)?.removeView(web)
        web.destroy()
    }

    private fun finishZone() {
        closeWeb()
        finish()
    }

    override fun onDestroy() {
        if (::web.isInitialized) closeWeb()
        if (::adapter.isInitialized) adapter.dispose()
        super.onDestroy()
    }

    companion object {
        const val EXTRA_ZONE = "zone"
        private const val TAG = "OpenAgentsZones"
    }
}
