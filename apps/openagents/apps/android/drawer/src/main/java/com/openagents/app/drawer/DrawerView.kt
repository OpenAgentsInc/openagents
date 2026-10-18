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

/** Android Views renderer; every operation is an intent and waits for core output. */
class DrawerView(context: Context) : LinearLayout(context) {
    var onRequest: (DrawerRequest) -> Unit = {}
    var onIntent: (DrawerIntent) -> Unit = {}
    private var request = DrawerRequest()
    private var binding = false
    private var scroll: android.os.Parcelable? = null
    private var frame = DrawerFrame(emptyList(), null, false)
    private val search = EditText(context).apply {
        hint = "Search chats"
        isSingleLine = true
        maxLines = 1
        filters = arrayOf(android.text.InputFilter.LengthFilter(200))
    }
    private val notice = TextView(context)
    private val rows = Rows()
    private val list = ListView(context).apply {
        adapter = rows
        choiceMode = ListView.CHOICE_MODE_NONE
        setOnItemClickListener { _, _, position, _ -> onIntent(DrawerIntent.Open(frame.rows[position].key)) }
        setOnItemLongClickListener { _, view, position, _ -> menu(view, frame.rows[position]); true }
    }
    private val more = Button(context).apply {
        text = "Load more"
        setOnClickListener { onIntent(DrawerIntent.More) }
    }

    private val moreProjects = Button(context).apply {
        text = "Load more projects"
        setOnClickListener { onIntent(DrawerIntent.MoreProjects) }
    }

    init {
        orientation = VERTICAL
        val tabs = LinearLayout(context)
        for ((label, tab) in listOf("Sessions" to DrawerTab.LIVE, "Saved chats" to DrawerTab.SAVED)) {
            tabs.addView(Button(context).apply {
                text = label
                setOnClickListener { change(request.copy(tab = tab, projectId = null)) }
            })
        }
        tabs.addView(Button(context).apply {
            text = "Archived"
            setOnClickListener { change(request.copy(archived = !request.archived)) }
        })
        tabs.addView(Button(context).apply {
            text = "Projects"
            setOnClickListener {
                val projects = frame.projects
                val names = arrayOf("All chats", *projects.map { it.name }.toTypedArray())
                AlertDialog.Builder(context).setTitle("Projects").setItems(names) { _, position ->
                    change(request.copy(projectId = if (position == 0) null else projects[position - 1].id))
                }.show()
            }
        })
        addView(android.widget.HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(tabs)
        })
        addView(search)
        addView(notice)
        addView(list, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        addView(more)
        addView(moreProjects)
        addView(Button(context).apply {
            text = "Try again"
            setOnClickListener { onIntent(DrawerIntent.Refresh) }
        })
        search.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                if (!binding) change(request.copy(query = s?.toString().orEmpty()))
            }
            override fun afterTextChanged(s: Editable?) = Unit
        })
    }

    private fun change(next: DrawerRequest) { request = next; onRequest(next) }

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
        notice.text = next.error ?: when {
            next.loading -> "Loading chats…"
            next.offline -> "Offline"
            next.reconnecting -> "Reconnecting…"
            next.rows.isEmpty() && next.hasMore -> "Keep searching with Load more."
            next.rows.isEmpty() -> "No chats found"
            else -> ""
        }
    }

    private fun menu(anchor: View, row: DrawerRow) {
        PopupMenu(context, anchor).apply {
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
        val origin = DrawerOrigin.entries.firstOrNull { it.name == state.getString("origin") }
        val key = state.getString("selected")?.let { id -> origin?.let { DrawerKey(it, id) } }
        @Suppress("DEPRECATION")
        val restoredScroll = state.getParcelable<android.os.Parcelable>("scroll")
        scroll = restoredScroll
        // Apply position only after the first authorized frame arrives.
        return key to request
    }

    private inner class Rows : BaseAdapter() {
        override fun getCount() = frame.rows.size
        override fun getItem(position: Int) = frame.rows[position]
        override fun getItemId(position: Int) = position.toLong()
        override fun hasStableIds() = false
        override fun getView(position: Int, recycled: View?, parent: ViewGroup): View {
            val view = (recycled as? TextView) ?: TextView(context).apply {
                setPadding(24, 20, 24, 20)
                minHeight = (64 * resources.displayMetrics.density).toInt()
                setBackgroundResource(android.R.drawable.list_selector_background)
            }
            val row = getItem(position)
            view.text = listOfNotNull(row.group, row.title, row.agentLine, row.subtitle, row.status,
                if (row.unread == true) "Unread" else null).joinToString("\n")
            view.isActivated = frame.selected == row.key
            view.isSelected = frame.selected == row.key
            view.contentDescription = view.text
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
