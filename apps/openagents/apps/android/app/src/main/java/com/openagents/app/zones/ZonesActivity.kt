package com.openagents.app.zones

import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.openagents.app.OaStyle
import com.openagents.app.R
import com.openagents.app.SemanticViewAdapter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.openagents_core.CoreZones
import uniffi.openagents_core.ZoneLaunch

/** One launcher list per process, so a refreshed catalog reaches the runner. */
internal object Zones {
    val core: CoreZones by lazy { CoreZones() }
}

/**
 * The zone launcher (ADR 0010): the Grid, the Haunted House, and the Verse as
 * Rust Native cards. Rust lists the zones and resolves a tap; this paints the
 * cards. Choosing the Grid returns [RESULT_GRID] so the main screen shows the
 * sessions list.
 */
class ZonesActivity : ComponentActivity() {
    private lateinit var cards: FrameLayout
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private lateinit var adapter: SemanticViewAdapter

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        fun dp(v: Int) = OaStyle.dp(this, v)
        cards = FrameLayout(this)
        val close = OaStyle.iconButton(this, R.drawable.oa_ic_close, "Close", 44,
            fill = getColor(R.color.oa_chrome), stroke = getColor(R.color.oa_chrome_border)).apply {
            setOnClickListener { finish() }
        }
        val bar = FrameLayout(this).apply {
            setPadding(dp(16), dp(6), dp(16), dp(6))
            addView(close, FrameLayout.LayoutParams(dp(44), dp(44), Gravity.START or Gravity.CENTER_VERTICAL))
        }
        val scroll = ScrollView(this).apply {
            addView(cards, FrameLayout.LayoutParams(-1, -2))
            setPadding(dp(20), dp(8), dp(20), dp(24))
            clipToPadding = false
        }
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(getColor(R.color.oa_background))
            addView(bar, LinearLayout.LayoutParams(-1, -2))
            addView(scroll, LinearLayout.LayoutParams(-1, 0, 1f))
        }
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            WindowInsetsCompat.CONSUMED
        }
        setContentView(root)
        adapter = SemanticViewAdapter(this,
            activate = { instance, revision, key -> activate(instance, revision.toULong(), key) },
            changeField = { _, _, _, _ -> },
            acknowledge = { _, _ -> },
        )
        render()
        // Pick up a newer catalog; the bundled pins stay on any failure.
        scope.launch {
            val updated = withContext(Dispatchers.IO) { runCatching { Zones.core.refresh() }.isSuccess }
            if (updated && !isFinishing) render()
        }
        intent.getStringExtra(EXTRA_ZONE)?.let { if (savedInstanceState == null) openWeb(it) }
    }

    private fun render() {
        runCatching { adapter.mount(cards, Zones.core.launcher().toByteArray(Charsets.UTF_8)) }
    }

    private fun activate(instance: String, revision: ULong, key: String) {
        try {
            when (val launch = Zones.core.activate(instance, revision, key)) {
                is ZoneLaunch.Grid -> { setResult(RESULT_GRID); finish() }
                is ZoneLaunch.Web -> openWeb(launch.zoneId)
            }
        } catch (error: Exception) {
            Toast.makeText(this, error.message ?: "That zone didn't open.", Toast.LENGTH_SHORT).show()
            render()
        }
    }

    private fun openWeb(zoneId: String) {
        startActivity(Intent(this, ZoneWebActivity::class.java).putExtra(ZoneWebActivity.EXTRA_ZONE, zoneId))
    }

    override fun onDestroy() {
        scope.cancel()
        adapter.dispose()
        super.onDestroy()
    }

    companion object {
        const val RESULT_GRID = android.app.Activity.RESULT_FIRST_USER + 1
        /** Debug and screenshot entry: open this zone's runner directly. */
        const val EXTRA_ZONE = "zone"
    }
}
