package com.openagents.app.drawer

import android.app.AlertDialog
import android.content.Context
import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.PopupMenu
import android.widget.TextView
import com.openagents.app.OaStyle
import com.openagents.app.R

/** Android Views renderer; every operation is an intent and waits for core output. */
class DrawerView(context: Context) : LinearLayout(context) {
    var onRequest: (DrawerRequest) -> Unit = {}
    var onIntent: (DrawerIntent) -> Unit = {}
    var onContinue: (DrawerKey) -> Unit = {}
    private var request = DrawerRequest()
    private var binding = false
    private var scroll: android.os.Parcelable? = null
    private var frame = DrawerFrame(emptyList(), null, false)
    private val search = EditText(context).apply {
        hint = "Search"
        isSingleLine = true
        maxLines = 1
        filters = arrayOf(android.text.InputFilter.LengthFilter(200))
        imeOptions = android.view.inputmethod.EditorInfo.IME_ACTION_SEARCH
        OaStyle.text(this, 17f)
        setHintTextColor(context.getColor(R.color.oa_secondary))
        background = OaStyle.rounded(context, context.getColor(R.color.oa_fill), 22f)
        setCompoundDrawablesRelativeWithIntrinsicBounds(context.getDrawable(R.drawable.oa_ic_search)?.mutate()?.apply {
            setTint(context.getColor(R.color.oa_secondary))
            setBounds(0, 0, dp(20), dp(20))
        }, null, null, null)
        compoundDrawablePadding = dp(10)
        setPadding(dp(16), 0, dp(16), 0)
    }
    private val heading = TextView(context).apply {
        OaStyle.text(this, 34f, OaStyle.Weight.BOLD)
        if (android.os.Build.VERSION.SDK_INT >= 28) isAccessibilityHeading = true
    }
    private val tabs = LinearLayout(context).apply { setPadding(dp(16), dp(10), dp(16), dp(2)) }
    private val notice = TextView(context).apply {
        OaStyle.text(this, 15f, colorResource = R.color.oa_secondary)
        gravity = android.view.Gravity.CENTER
    }
    private val rows = Rows()
    private val list = ListView(context).apply {
        adapter = rows
        choiceMode = ListView.CHOICE_MODE_NONE
        divider = null
        dividerHeight = 0
        selector = android.graphics.drawable.ColorDrawable(android.graphics.Color.TRANSPARENT)
        clipToPadding = false
        setPadding(0, dp(4), 0, dp(8))
        setOnItemClickListener { _, _, position, _ -> onIntent(DrawerIntent.Open(frame.rows[position].key)) }
        setOnItemLongClickListener { _, view, position, _ -> menu(view, frame.rows[position]); true }
    }
    private val more = Button(context).apply {
        text = "Load more"
        OaStyle.text(this, 15f, OaStyle.Weight.MEDIUM, R.color.oa_secondary)
        visibility = View.GONE
        setOnClickListener { onIntent(DrawerIntent.More) }
    }

    private val moreProjects = Button(context).apply {
        text = "Load more projects"
        OaStyle.text(this, 15f, OaStyle.Weight.MEDIUM, R.color.oa_secondary)
        visibility = View.GONE
        setOnClickListener { onIntent(DrawerIntent.MoreProjects) }
    }
    private val retry = Button(context).apply {
        text = "Try again"
        OaStyle.text(this, 15f, OaStyle.Weight.MEDIUM)
        visibility = View.GONE
        setOnClickListener { onIntent(DrawerIntent.Refresh) }
    }
    private val tabButtons = mutableListOf<Pair<TextView, (DrawerRequest) -> Boolean>>()

    init {
        orientation = VERTICAL
        fun tab(label: String, selected: (DrawerRequest) -> Boolean, run: () -> Unit) {
            val chip = TextView(context).apply {
                text = label
                OaStyle.text(this, 14f, OaStyle.Weight.MEDIUM)
                gravity = android.view.Gravity.CENTER
                minHeight = dp(34)
                setPadding(dp(14), 0, dp(14), 0)
                isClickable = true
                isFocusable = true
                setOnClickListener { run() }
            }
            tabs.addView(chip, LayoutParams(LayoutParams.WRAP_CONTENT, dp(34)).apply { marginEnd = dp(6) })
            tabButtons += chip to selected
        }
        for ((label, tab) in listOf("Sessions" to DrawerTab.LIVE, "Saved chats" to DrawerTab.SAVED)) {
            tab(label, { it.tab == tab && !it.archived }) { change(request.copy(tab = tab, projectId = null)) }
        }
        tab("Archived", { it.archived }) { change(request.copy(archived = !request.archived)) }
        tab("Projects", { it.projectId != null }) {
            val projects = frame.projects
            val names = arrayOf("All chats", *projects.map { it.name }.toTypedArray())
            AlertDialog.Builder(context).setTitle("Projects").setItems(names) { _, position ->
                change(request.copy(projectId = if (position == 0) null else projects[position - 1].id))
            }.show()
        }
        addView(heading, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply {
            setMargins(dp(20), dp(28), dp(20), dp(14))
        })
        addView(search, LayoutParams(LayoutParams.MATCH_PARENT, dp(46)).apply { setMargins(dp(16), 0, dp(16), 0) })
        addView(android.widget.HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(tabs)
        })
        addView(notice, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply { setMargins(dp(20), dp(8), dp(20), 0) })
        addView(list, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        addView(more)
        addView(moreProjects)
        addView(retry)
        renderTabs()
        search.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                if (!binding) change(request.copy(query = s?.toString().orEmpty()))
            }
            override fun afterTextChanged(s: Editable?) = Unit
        })
    }

    private fun change(next: DrawerRequest) { request = next; renderTabs(); onRequest(next) }

    private fun dp(value: Int) = OaStyle.dp(context, value)

    private fun renderTabs() {
        heading.text = when {
            request.archived -> "Archived"
            request.tab == DrawerTab.SAVED -> "Saved chats"
            else -> "Sessions"
        }
        for ((chip, selected) in tabButtons) {
            val on = selected(request)
            chip.isSelected = on
            chip.setTextColor(context.getColor(if (on) R.color.oa_text else R.color.oa_secondary))
            chip.background = OaStyle.rowBackground(context, if (on) context.getColor(R.color.oa_fill) else 0, 17f)
        }
    }

    fun render(next: DrawerFrame) {
        frame = next
        rows.notifyDataSetChanged()
        if (!next.loading && next.error == null) {
            scroll?.let { list.onRestoreInstanceState(it) }
            scroll = null
        }
        more.visibility = if (next.hasMore) View.VISIBLE else View.GONE
        more.isEnabled = !next.loading
        moreProjects.visibility = if (next.hasMoreProjects) View.VISIBLE else View.GONE
        retry.visibility = if (next.error != null) View.VISIBLE else View.GONE
        notice.text = next.error ?: when {
            next.loading -> "Loading chats…"
            next.offline -> "Offline"
            next.reconnecting -> "Reconnecting…"
            next.rows.isEmpty() && next.hasMore -> "Keep searching with Load more."
            next.rows.isEmpty() -> "No chats found"
            else -> ""
        }
        notice.visibility = if (notice.text.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun menu(anchor: View, row: DrawerRow) {
        PopupMenu(context, anchor).apply {
            if (row.key.origin == DrawerOrigin.LIVE || row.key.origin == DrawerOrigin.SAVED) {
                menu.add("Continue in another chat").setOnMenuItemClickListener {
                    onContinue(row.key); true
                }
            }
            if (DrawerAction.PIN in row.actions) menu.add(if (row.pinned) "Unpin" else "Pin").setOnMenuItemClickListener {
                onIntent(DrawerIntent.Pin(row.key, !row.pinned)); true
            }
            if (DrawerAction.ARCHIVE in row.actions) menu.add(if (row.archived) "Restore" else "Archive").setOnMenuItemClickListener {
                onIntent(DrawerIntent.Archive(row.key, !row.archived)); true
            }
            row.moveEarlier?.let { move -> menu.add("Move up").setOnMenuItemClickListener {
                onIntent(DrawerIntent.MovePin(row.key, move.after, move.before, move.revision)); true
            } }
            row.moveLater?.let { move -> menu.add("Move down").setOnMenuItemClickListener {
                onIntent(DrawerIntent.MovePin(row.key, move.after, move.before, move.revision)); true
            } }
            if (DrawerAction.RENAME in row.actions) menu.add("Rename").setOnMenuItemClickListener {
                val input = EditText(context).apply { setText(row.title); isSingleLine = true }
                AlertDialog.Builder(context).setTitle("Rename chat").setView(input)
                    .setNegativeButton("Cancel", null)
                    .setPositiveButton("Save") { _, _ -> onIntent(DrawerIntent.Rename(row.key, input.text.toString())) }
                    .show()
                true
            }
            show()
        }
    }

    /** Save only UI inputs and the Rust-selected opaque key; never a session list. */
    fun saveUiState(): Bundle = Bundle().apply {
        putString("tab", request.tab.name)
        putString("query", request.query)
        putBoolean("archived", request.archived)
        putString("project", request.projectId)
        frame.selected?.let { putString("origin", it.origin.name); putString("selected", it.id) }
        putParcelable("scroll", list.onSaveInstanceState())
    }

    fun restoreUiState(state: Bundle): Pair<DrawerKey?, DrawerRequest> {
        request = DrawerRequest(
            DrawerTab.entries.firstOrNull { it.name == state.getString("tab") } ?: DrawerTab.LIVE,
            state.getString("query").orEmpty().take(200), state.getBoolean("archived"), state.getString("project"),
        )
        binding = true
        search.setText(request.query)
        binding = false
        renderTabs()
        val origin = DrawerOrigin.entries.firstOrNull { it.name == state.getString("origin") }
        val key = state.getString("selected")?.let { id -> origin?.let { DrawerKey(it, id) } }
        @Suppress("DEPRECATION")
        val restoredScroll = state.getParcelable<android.os.Parcelable>("scroll")
        scroll = restoredScroll
        // Apply position only after the first authorized frame arrives.
        return key to request
    }

    /** One sidebar row: an optional group header, a bold title, a secondary line, and a status. */
    private inner class RowViews(context: Context) : LinearLayout(context) {
        val header = TextView(context).apply {
            OaStyle.text(this, 15f, OaStyle.Weight.SEMIBOLD, R.color.oa_secondary)
            setPadding(dp(20), dp(18), dp(20), dp(6))
            if (android.os.Build.VERSION.SDK_INT >= 28) isAccessibilityHeading = true
        }
        val body = LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = android.view.Gravity.CENTER_VERTICAL
            setPadding(dp(12), dp(10), dp(12), dp(10))
            minimumHeight = dp(60)
            isDuplicateParentStateEnabled = true
        }
        val title = TextView(context).apply {
            OaStyle.text(this, 17f, OaStyle.Weight.SEMIBOLD)
            maxLines = 1
            ellipsize = android.text.TextUtils.TruncateAt.END
        }
        val subtitle = TextView(context).apply {
            OaStyle.text(this, 14f, colorResource = R.color.oa_secondary)
            maxLines = 1
            ellipsize = android.text.TextUtils.TruncateAt.END
        }
        val status = TextView(context).apply {
            OaStyle.text(this, 14f, OaStyle.Weight.MEDIUM, R.color.oa_secondary)
            maxLines = 1
        }
        init {
            orientation = VERTICAL
            val texts = LinearLayout(context).apply {
                orientation = VERTICAL
                addView(title)
                addView(subtitle, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { topMargin = dp(4) })
            }
            body.addView(texts, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            // The status lines up with the title, as on iOS.
            body.addView(status, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                marginStart = dp(10); gravity = android.view.Gravity.TOP; topMargin = dp(2)
            })
            addView(header)
            addView(body, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT).apply { setMargins(dp(8), 0, dp(8), 0) })
        }
    }

    /** Color and mark for the status Rust supplies; the words stay Rust's. */
    private fun statusStyle(status: String?): Pair<String, Int>? = when (status) {
        null -> null
        "Finished" -> "✓ $status" to R.color.oa_success
        "Couldn't finish" -> "● $status" to R.color.oa_danger
        "Waiting for you" -> "● $status" to R.color.oa_warning
        else -> status to R.color.oa_secondary
    }

    private inner class Rows : BaseAdapter() {
        override fun getCount() = frame.rows.size
        override fun getItem(position: Int) = frame.rows[position]
        override fun getItemId(position: Int) = position.toLong()
        override fun hasStableIds() = false
        override fun getView(position: Int, recycled: View?, parent: ViewGroup): View {
            val view = (recycled as? RowViews) ?: RowViews(context)
            val row = getItem(position)
            val selected = frame.selected == row.key
            val group = row.group.takeIf { it.isNotBlank() && (position == 0 || frame.rows[position - 1].group != row.group) }
            view.header.text = group.orEmpty()
            view.header.visibility = if (group == null) View.GONE else View.VISIBLE
            view.title.text = row.title
            val secondary = listOfNotNull(row.agentLine, row.subtitle).filter { it.isNotBlank() }.joinToString("  ·  ")
            view.subtitle.text = secondary
            view.subtitle.visibility = if (secondary.isEmpty()) View.GONE else View.VISIBLE
            val status = statusStyle(row.status) ?: if (row.unread == true) "●" to R.color.oa_accent else null
            view.status.text = status?.first.orEmpty()
            status?.let { view.status.setTextColor(context.getColor(it.second)) }
            view.status.visibility = if (status == null) View.GONE else View.VISIBLE
            view.body.background = OaStyle.rowBackground(context,
                if (selected) context.getColor(R.color.oa_fill) else 0, 14f)
            view.isActivated = selected
            view.isSelected = selected
            view.contentDescription = listOfNotNull(row.group, row.title, row.agentLine, row.subtitle, row.status,
                if (row.unread == true) "Unread" else null).joinToString("\n")
            var down = 0f
            view.setOnTouchListener { _, event ->
                when (event.actionMasked) {
                    MotionEvent.ACTION_DOWN -> down = event.x
                    MotionEvent.ACTION_UP -> if (kotlin.math.abs(event.x - down) > 96 * resources.displayMetrics.density && DrawerAction.ARCHIVE in row.actions) {
                        // Swiping offers the same explicit action; it never removes a row locally.
                        menu(view, row)
                        return@setOnTouchListener true
                    }
                    MotionEvent.ACTION_CANCEL -> down = 0f
                }
                false
            }
            return view
        }
    }
}
