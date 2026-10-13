// The phone's shell (#11126), as on iOS (Shell.swift): the top bar (menu,
// the Coder / Verse switch, new chat), the drawer with the main places and
// recent chats, and the link cards under replies. Rust owns the state (`coder_tab::shell`, the packet's `shell`
// and `links`); this file draws it and sends the person's taps back as
// shell actions.
package com.openagents.app

import android.content.Context
import android.graphics.Outline
import android.text.Editable
import android.text.TextUtils
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import org.json.JSONObject

/** A round 44 dp button with a glyph; its label is the spoken name. */
internal fun Context.circleButton(icon: Int, label: String, key: String, action: () -> Unit): FrameLayout =
    FrameLayout(this).apply {
        addView(ImageView(context).apply {
            setImageResource(icon); setColorFilter(Palette.PRIMARY)
            scaleType = ImageView.ScaleType.CENTER
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }, FrameLayout.LayoutParams(-1, -1))
        background = rounded(Palette.RAISED, 22f, Palette.BORDER)
        contentDescription = label; tag = key
        isClickable = true; isFocusable = true
        setOnClickListener { action() }
    }

/**
 * The menu button every place starts with: a tap opens the drawer, and a
 * long press reports a problem with the screen on view.
 */
internal fun Context.menuButton(open: () -> Unit, report: () -> Unit): FrameLayout =
    circleButton(R.drawable.ic_glyph_menu, "Open menu", "shell-menu", open).apply {
        setOnLongClickListener { report(); true }
    }

/**
 * The top bar over the chat and the Verse: the menu, the Coder / Verse
 * switch on a new chat and in the Verse, and New chat in a conversation.
 */
internal class ShellTopBar(private val context: Context, private val bridge: MobileBridge,
                           private val openDrawer: () -> Unit, private val report: () -> Unit) {
    val root = FrameLayout(context)
    private var shown: String? = null

    /** Draws `shell` (Rust's `ShellView`); hidden without one or on every chat's list. */
    fun update(shell: JSONObject?, force: Boolean = false) {
        val screen = shell?.optString("screen")
        root.visibility = if (shell == null || screen == "list") View.GONE else View.VISIBLE
        val place = shell?.optString("place")
        val wanted = "$screen:$place"
        if (wanted == shown && !force) return
        shown = wanted
        root.removeAllViews()
        root.setPadding(context.dp(16), context.dp(4), context.dp(16), context.dp(6))
        if (shell == null) return
        root.addView(context.menuButton(openDrawer, report),
            FrameLayout.LayoutParams(context.dp(44), context.dp(44), Gravity.START or Gravity.CENTER_VERTICAL))
        if (screen == "new" || place == "verse") root.addView(placeSwitch(place == "verse"),
            FrameLayout.LayoutParams(-2, -2, Gravity.CENTER))
        if (screen == "chat" && place != "verse") root.addView(context.circleButton(R.drawable.ic_glyph_compose, "New chat", "shell-new-chat") {
            bridge.shell("new_chat")
        }, FrameLayout.LayoutParams(context.dp(44), context.dp(44), Gravity.END or Gravity.CENTER_VERTICAL))
    }

    /** Coder or Verse: one capsule, the chosen half filled. */
    private fun placeSwitch(verse: Boolean): View = context.row().apply {
        setPadding(context.dp(4), context.dp(4), context.dp(4), context.dp(4))
        background = context.rounded(Palette.RAISED, 23f, Palette.BORDER)
        for ((title, chosen) in listOf("Coder" to !verse, "Verse" to verse)) {
            addView(context.text(title, 16f, if (chosen) Palette.PRIMARY else Palette.SECONDARY).apply {
                gravity = Gravity.CENTER
                setPadding(context.dp(18), 0, context.dp(18), 0)
                if (chosen) {
                    typeface = Fonts.typeface(context, Fonts.BOLD)
                    background = context.rounded((Palette.PRIMARY and 0x00FFFFFF) or 0x1F000000, 19f)
                }
                isSelected = chosen
                tag = "shell-switch-${title.lowercase()}"
                contentDescription = if (chosen) "$title, selected" else title
                isClickable = true; isFocusable = true
                setOnClickListener { bridge.shell("switch", "verse" to (title == "Verse")) }
            }, LinearLayout.LayoutParams(-2, context.dp(38)))
        }
    }
}

/**
 * The drawer: the app's name and search, the main places, recent chats with
 * See all, a new chat, and the account. `go` opens a place by ID (`code`,
 * `computers`, `wallet`, `verse`, `settings`, `chat`).
 */
internal class ShellDrawer(private val context: Context, private val bridge: MobileBridge,
                           private val go: (String) -> Unit) {
    val root = context.column()
    private val header = context.row().apply { gravity = Gravity.CENTER_VERTICAL }
    private val body = context.column()
    private var searching = false
    private var shown: String? = null
    private val search = EditText(context).apply {
        hint = "Search chats"; textSize = 16f; isSingleLine = true
        setTextColor(Palette.PRIMARY); setHintTextColor(Palette.TERTIARY)
        typeface = Fonts.typeface(context)
        background = null
        imeOptions = EditorInfo.IME_ACTION_SEARCH
        tag = "shell-search-field"
        addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) { if (searching) bridge.shell("search", "query" to s.toString()) }
        })
    }

    init {
        build()
    }

    /** Builds the drawer in the current palette. */
    fun build() {
        root.removeAllViews()
        header.removeAllViews()
        root.setBackgroundColor(Palette.BACKGROUND)
        search.setTextColor(Palette.PRIMARY); search.setHintTextColor(Palette.TERTIARY)
        root.addView(header, LinearLayout.LayoutParams(-1, -2).apply {
            setMargins(context.dp(24), context.dp(8), context.dp(24), context.dp(12)) })
        (body.parent as? ViewGroup)?.removeView(body)
        root.addView(ScrollView(context).apply {
            addView(body, FrameLayout.LayoutParams(-1, -2))
        }, LinearLayout.LayoutParams(-1, 0, 1f))
        root.addView(footer(), LinearLayout.LayoutParams(-1, -2).apply {
            setMargins(context.dp(24), context.dp(8), context.dp(24), context.dp(8)) })
        header()
        shown = null
    }

    private fun header() {
        header.removeAllViews()
        if (searching) {
            val field = context.row().apply {
                gravity = Gravity.CENTER_VERTICAL
                background = context.rounded(Palette.RAISED, 22f)
                setPadding(context.dp(14), 0, context.dp(8), 0)
                addView(ImageView(context).apply {
                    setImageResource(R.drawable.ic_glyph_search); setColorFilter(Palette.SECONDARY)
                }, LinearLayout.LayoutParams(context.dp(18), context.dp(18)))
                (search.parent as? ViewGroup)?.removeView(search)
                addView(search, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = context.dp(8) })
            }
            header.addView(field, LinearLayout.LayoutParams(0, context.dp(44), 1f))
            header.addView(context.text("Cancel", 15f).apply {
                setPadding(context.dp(12), context.dp(10), 0, context.dp(10))
                tag = "shell-search-cancel"
                setOnClickListener { stopSearching() }
            })
        } else {
            header.addView(context.text("OpenAgents", 24f).apply {
                typeface = Fonts.typeface(context, Fonts.BOLD)
                if (android.os.Build.VERSION.SDK_INT >= 28) isAccessibilityHeading = true
            }, LinearLayout.LayoutParams(0, -2, 1f))
            header.addView(context.circleButton(R.drawable.ic_glyph_search, "Search chats", "shell-search") {
                searching = true
                header()
                shown = null
                search.requestFocus()
                context.getSystemService(InputMethodManager::class.java)?.showSoftInput(search, 0)
            }, LinearLayout.LayoutParams(context.dp(44), context.dp(44)))
        }
    }

    private fun stopSearching() {
        searching = false
        search.setText("")
        context.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(search.windowToken, 0)
        bridge.shell("search", "query" to "")
        header()
        shown = null
    }

    /** Closing the drawer ends a search. */
    fun closed() { if (searching) stopSearching() }

    private fun footer(): View = context.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        addView(context.text("Chat", 17f, Palette.BACKGROUND).apply {
            typeface = Fonts.typeface(context, Fonts.BOLD)
            gravity = Gravity.CENTER_VERTICAL
            val icon = context.getDrawable(R.drawable.ic_glyph_compose)?.mutate()?.apply {
                setBounds(0, 0, context.dp(18), context.dp(18)); setTint(Palette.BACKGROUND)
            }
            setCompoundDrawables(icon, null, null, null)
            compoundDrawablePadding = context.dp(8)
            setPadding(context.dp(22), 0, context.dp(22), 0)
            background = context.rounded(Palette.PRIMARY, 26f)
            contentDescription = "New chat"; tag = "shell-chat"
            isClickable = true; isFocusable = true
            setOnClickListener {
                bridge.shell("new_chat")
                go("chat")
            }
        }, LinearLayout.LayoutParams(-2, context.dp(52)))
        addView(View(context), LinearLayout.LayoutParams(0, 1, 1f))
        addView(context.circleButton(R.drawable.ic_glyph_person, "Account", "shell-account") { go("settings") }.apply {
            background = context.rounded(Palette.RAISED, 26f, Palette.BORDER)
        }, LinearLayout.LayoutParams(context.dp(52), context.dp(52)))
    }

    /** Draws the drawer's rows from the packet's `shell.drawer`, and the account's (`link`). */
    fun update(shell: JSONObject?, link: JSONObject? = bridge.packet?.objectOrNull("link")) {
        val drawer = shell?.objectOrNull("drawer")
        val account = link?.takeIf { it.optBoolean("signed_in") }
        val accountRows = account?.optJSONArray("drawer")?.objects().orEmpty()
        val encoded = "$searching:${drawer?.toString()}:${Preview.on}:${account?.optInt("running")}:" +
            "${account?.optInt("asking")}:${account?.optJSONArray("drawer")}"
        if (encoded == shown) return
        shown = encoded
        body.removeAllViews()
        body.setPadding(context.dp(24), 0, context.dp(24), context.dp(12))
        if (!searching) {
            val places = buildList {
                add(Triple("code", "Coder", R.drawable.ic_glyph_code))
                add(Triple("computers", "Computers", R.drawable.ic_glyph_computer))
                add(Triple("wallet", "Wallet", R.drawable.ic_glyph_wallet))
                add(Triple("verse", "Verse", R.drawable.ic_tab_verse))
                add(Triple("settings", "Settings", R.drawable.ic_glyph_settings))
            }
            for ((id, title, icon) in places) body.addView(row(title, "shell-place-$id", icon) { go(id) })
            // Running and the account's chats, once signed in (#11107, #11165).
            if (account != null) {
                val running = account.optInt("running")
                val asking = account.optInt("asking")
                val label = when {
                    asking > 0 -> "Running · $asking asking"
                    running > 0 -> "Running · $running"
                    else -> "Running"
                }
                body.addView(row(label, "shell-place-running", R.drawable.ic_glyph_terminal) { go("running") })
                if (accountRows.isNotEmpty()) {
                    body.addView(context.text("On your account", 13f, Palette.SECONDARY).apply {
                        typeface = Fonts.typeface(context, Fonts.BOLD)
                        setPadding(0, context.dp(10), 0, context.dp(4))
                    })
                    for (chat in accountRows) {
                        val id = chat.optString("id")
                        val title = chat.optString("title").ifEmpty { "New chat" }
                        val detail = chat.optString("detail")
                        body.addView(row("$title · $detail", "shell-account-chat-$id", null) { go("link_chat:$id") }
                            .apply { contentDescription = "$title, $detail" })
                    }
                    body.addView(row("All account chats…", "shell-account-chats", null, Palette.SECONDARY) {
                        go("account_chats")
                    })
                }
            }
            body.addView(context.divider(), LinearLayout.LayoutParams(-1, 1).apply {
                topMargin = context.dp(14); bottomMargin = context.dp(14) })
        }
        val recent = drawer?.optJSONArray("recent")?.objects().orEmpty()
        if (recent.isEmpty()) body.addView(context.text(
            if (searching && search.text.isNotEmpty()) "No chats match." else "No chats yet.", 15f, Palette.SECONDARY).apply {
            setPadding(0, context.dp(12), 0, context.dp(12)) })
        for (chat in recent) {
            val index = chat.optInt("index")
            val title = chat.optString("title").ifEmpty { "New chat" }
            body.addView(row(title, "shell-recent-$index", null) {
                bridge.shell("open", "index" to index)
                go("chat")
            }.apply { chat.textOrNull("detail")?.let { contentDescription = "$title, $it" } })
        }
        if (drawer?.optBoolean("more") == true) body.addView(row("See all…", "shell-see-all", null, Palette.SECONDARY) {
            bridge.shell("see_all")
            go("chat")
        })
    }

    private fun row(title: String, key: String, icon: Int?, color: Int = Palette.PRIMARY, action: () -> Unit): View =
        context.row().apply {
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = context.dp(50)
            tag = key
            isClickable = true; isFocusable = true
            contentDescription = title
            if (icon != null) addView(ImageView(context).apply {
                setImageResource(icon); setColorFilter(Palette.PRIMARY)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(context.dp(22), context.dp(22)).apply { marginEnd = context.dp(18) })
            addView(context.text(title, 17f, color).apply {
                maxLines = 1; ellipsize = TextUtils.TruncateAt.END
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, LinearLayout.LayoutParams(0, -2, 1f))
            setOnClickListener { action() }
        }
}

/**
 * A link card under a reply (`link:` surfaces): the page's picture when it
 * has one, then its title and site. Rust chose the link from the reply and
 * read the page; a tap opens the link in the browser.
 */
internal object LinkCards {
    fun card(context: Context, bridge: MobileBridge, resource: String, open: (String) -> Unit): View? {
        val card = bridge.packet?.objectOrNull("links")?.objectOrNull(resource) ?: return null
        val url = card.optString("url")
        val title = card.optString("title")
        val site = card.optString("site")
        val root = context.column().apply {
            background = context.rounded(Palette.RAISED, 14f, Palette.BORDER)
            clipToOutline = true
            outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) =
                    outline.setRoundRect(0, 0, view.width, view.height, context.dpf(14f))
            }
            contentDescription = if (title == site) "$site, link" else "$title, $site, link"
            tag = resource
            isClickable = true; isFocusable = true
            setOnClickListener { if (url.startsWith("https://")) open(url) }
        }
        if (card.optBoolean("image")) {
            val picture = ImageView(context).apply {
                scaleType = ImageView.ScaleType.CENTER_CROP
                setBackgroundColor(Palette.SURFACE)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }
            bridge.image(resource) { bitmap -> bitmap?.let { picture.setImageBitmap(it) } }
            root.addView(picture, LinearLayout.LayoutParams(-1, 0, 1f))
            root.addView(View(context).apply { setBackgroundColor(Palette.BORDER) }, LinearLayout.LayoutParams(-1, 1))
        }
        val words = context.column().apply {
            gravity = Gravity.CENTER_VERTICAL
            setPadding(context.dp(14), context.dp(10), context.dp(14), context.dp(10))
        }
        words.addView(context.text(title, 15f).apply {
            typeface = Fonts.typeface(context, Fonts.BOLD)
            maxLines = if (card.optBoolean("image")) 2 else 1; ellipsize = TextUtils.TruncateAt.END
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        })
        words.addView(context.text(site, 13f, Palette.SECONDARY).apply {
            maxLines = 1; ellipsize = TextUtils.TruncateAt.END
            val icon = context.getDrawable(R.drawable.ic_glyph_link)?.mutate()?.apply {
                setBounds(0, 0, context.dp(13), context.dp(13)); setTint(Palette.SECONDARY)
            }
            setCompoundDrawables(icon, null, null, null)
            compoundDrawablePadding = context.dp(5)
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }, LinearLayout.LayoutParams(-2, -2).apply { topMargin = context.dp(3) })
        root.addView(words, LinearLayout.LayoutParams(-1, if (card.optBoolean("image")) -2 else -1))
        return root
    }
}

/** The theme's background, see-through: the floating composer's fade starts from it. */
internal fun clearOf(color: Int) = color and 0x00FFFFFF

/**
 * A reply's **Enter the Grid** card (Rust's `verse-portal` surface), when
 * the answer offered the Verse: a tap switches to the Grid.
 */
internal object VersePortal {
    fun card(context: Context, label: String, enter: () -> Unit): View = context.row().apply {
        gravity = Gravity.CENTER_VERTICAL
        background = context.rounded(Palette.RAISED, 14f, Palette.BORDER)
        setPadding(context.dp(16), context.dp(12), context.dp(16), context.dp(12))
        contentDescription = label; tag = "verse-portal"
        isClickable = true; isFocusable = true
        setOnClickListener { enter() }
        addView(FrameLayout(context).apply {
            background = context.rounded(Palette.SURFACE, 14f, Palette.BORDER)
            addView(ImageView(context).apply {
                setImageResource(R.drawable.ic_tab_verse); setColorFilter(Palette.PRIMARY)
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            }, FrameLayout.LayoutParams(context.dp(30), context.dp(30), Gravity.CENTER))
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }, LinearLayout.LayoutParams(context.dp(64), context.dp(64)))
        addView(context.column().apply {
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            addView(context.text(label, 16f).apply { typeface = Fonts.typeface(context, Fonts.SEMIBOLD) })
            addView(context.text("The Verse", 13f, Palette.SECONDARY), LinearLayout.LayoutParams(-2, -2).apply {
                topMargin = context.dp(4) })
        }, LinearLayout.LayoutParams(0, -2, 1f).apply { marginStart = context.dp(14) })
    }
}
