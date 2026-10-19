package com.openagents.app.composer

import android.content.Context
import android.net.Uri
import android.graphics.Typeface
import android.os.Build
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsAnimation
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.TextView
import androidx.core.view.OnReceiveContentListener
import androidx.core.view.ViewCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.io.Closeable
import java.nio.charset.StandardCharsets
import kotlin.math.max

/** An action already admitted by the current Rust view. */
data class ComposerAction(
    val node: String,
    val label: String,
    val enabled: Boolean = true,
)

/** A send option whose meaning and token remain in Rust. */
data class ComposerChoice(val token: String, val label: String)

/** A display-only chip supplied by Rust, such as a project or model. */
data class ComposerChip(val key: String, val label: String)

/** Bounded attachment metadata. File bytes and paths stay in Rust. */
data class ComposerAttachment(
    val key: String,
    val label: String,
    val removeAction: ComposerAction? = null,
)

/** A bounded file payload ready for the existing Rust session send call. */
data class ComposerAttachmentPayload(
    val name: String,
    val mimeType: String,
    val data: ByteArray,
)

data class ComposerMentionFile(val path: String, val isDirectory: Boolean)

/** File search and canonical link encoding stay in the signed-in Rust core. */
interface ComposerMentionPort {
    suspend fun searchFiles(query: String): List<ComposerMentionFile>
    fun fileMentionLink(file: ComposerMentionFile): String
}

data class ActiveComposerMentionQuery(val startUtf16: Int, val endUtf16: Int, val query: String)

/** Finds a bounded @query at the current UTF-16 cursor without parsing paths. */
object ComposerMentionQuery {
    fun active(text: String, cursorUtf16: Int): ActiveComposerMentionQuery? {
        if (cursorUtf16 !in 0..text.length) return null
        var index = cursorUtf16 - 1
        while (index >= 0) {
            when (text[index]) {
                '@' -> {
                    if (index > 0 && !text[index - 1].isWhitespace()) return null
                    val query = text.substring(index + 1, cursorUtf16)
                    if (query.length > MAX_QUERY_CHARS || query.any(Char::isWhitespace)) return null
                    return ActiveComposerMentionQuery(index, cursorUtf16, query)
                }
                else -> if (text[index].isWhitespace()) return null
            }
            index--
        }
        return null
    }

    private const val MAX_QUERY_CHARS = 128
}

/** The editor identity shared with Rust's UTF-16 mirror. */
data class ComposerEditorStamp(
    val instance: String,
    val viewRevision: Long,
    val node: String,
    val inputToken: String,
    val editingLifetime: String,
    val editorRevision: Long,
)

/** One complete native text-field update, including marked text and selection. */
data class ComposerEditorUpdate(
    val stamp: ComposerEditorStamp,
    val text: String,
    val selectionStartUtf16: Int,
    val selectionEndUtf16: Int,
    val markedStartUtf16: Int?,
    val markedEndUtf16: Int?,
)

/** The canonical editor state returned by Rust after an edit. */
data class ComposerEditorAck(
    val stamp: ComposerEditorStamp,
    val text: String,
    val selectionStartUtf16: Int,
    val selectionEndUtf16: Int,
    val markedStartUtf16: Int?,
    val markedEndUtf16: Int?,
)

/** Immutable presentation state projected from Rust. */
data class ComposerFrame(
    val stamp: ComposerEditorStamp,
    val initialDraft: String,
    val maxBytes: Int,
    val placeholder: String,
    val enabled: Boolean,
    val busy: Boolean,
    val primaryLabel: String,
    val stopAction: ComposerAction?,
    val sendChoices: List<ComposerChoice> = emptyList(),
    val contextChips: List<ComposerChip> = emptyList(),
    val attachments: List<ComposerAttachment> = emptyList(),
    val attachmentActions: List<ComposerAction> = emptyList(),
    val mentionSuggestions: List<ComposerAction> = emptyList(),
    val sendOnEnter: Boolean = false,
    /** Validated Rust Native JSON for question, approval, or queued-message views. */
    val sharedCardsJson: String? = null,
)

/** Narrow boundary to Rust's composer editor and current-view intent resolver. */
interface ComposerCorePort {
    fun mountEditor(stamp: ComposerEditorStamp, maxBytes: Int, draft: String): ComposerEditorAck =
        ComposerEditorAck(stamp, draft, draft.length, draft.length, null, null)
    fun edit(update: ComposerEditorUpdate): ComposerEditorAck
    /** Returns false or throws when Rust cannot accept the send. */
    fun submit(
        stamp: ComposerEditorStamp,
        choiceToken: String?,
        text: String,
        attachments: List<ComposerAttachmentPayload>,
    ): Boolean
    fun activate(instance: String, revision: Long, node: String)
    fun activateAttachment(instance: String, revision: Long, node: String): Boolean {
        activate(instance, revision, node)
        return true
    }
    fun changeField(instance: String, revision: Long, node: String, value: String)
    fun acknowledge(instance: String, revision: Long)
}

/** Mounts shared cards and sheets through the Rust Native adapter. */
fun interface RustNativeComposerMount {
    fun mount(validatedViewJson: String, target: FrameLayout): Closeable
}

/** Android Views for the chat composer. It presents Rust state and forwards events. */
class AndroidComposerPresentation(
    context: Context,
    private val core: ComposerCorePort,
    private val rustNativeMount: RustNativeComposerMount,
    private val mentionPort: ComposerMentionPort? = null,
) : Closeable {
    private val context = context
    private val density = context.resources.displayMetrics.density
    private val root = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        clipChildren = false
        clipToPadding = false
        isFocusableInTouchMode = true
    }
    private val cards = FrameLayout(context)
    private val contextRow = horizontalRow()
    private val contextScroll = horizontalScroll(contextRow)
    private val attachmentRow = horizontalRow()
    private val attachmentScroll = horizontalScroll(attachmentRow)
    private val mentionRow = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    private val mentionScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val sendError = TextView(context).apply {
        text = "Couldn't send. Try again."
        visibility = View.GONE
        contentDescription = text
        setPadding(dp(8), dp(4), dp(8), dp(4))
    }
    private val inputRow = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.BOTTOM
    }
    private val attachmentButton = Button(context).apply {
        text = "＋"
        contentDescription = "Add attachment"
        minimumWidth = dp(48)
        minimumHeight = dp(48)
    }
    private val input = EditText(context).apply {
        inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
        isSingleLine = false
        minLines = 1
        maxLines = 6
        imeOptions = EditorInfo.IME_ACTION_SEND or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        setHorizontallyScrolling(false)
        typeface = Typeface.DEFAULT
        contentDescription = "Message"
        setPadding(dp(12), dp(10), dp(12), dp(10))
    }
    private val sendButton = Button(context).apply {
        minimumWidth = dp(48)
        minimumHeight = dp(48)
    }
    private var frame: ComposerFrame? = null
    private var editorStamp: ComposerEditorStamp? = null
    private var sharedCardsJson: String? = null
    private var sharedCardsMount: Closeable? = null
    private var applyingAck = false
    private var disposed = false
    private val pendingAttachments = mutableListOf<ComposerAttachmentPayload>()
    private var attachmentPickerListener: (() -> Unit)? = null
    private var pastedImageListener: ((Uri) -> Unit)? = null
    private var mentionSearch: Job? = null
    private var activeMention: ActiveComposerMentionQuery? = null
    private var mentionGeneration = 0L
    private val baseBottomPadding = root.paddingBottom

    private val textWatcher = object : TextWatcher {
        override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
        override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
        override fun afterTextChanged(editable: Editable?) {
            if (disposed || applyingAck || editable == null) return
            val stamp = editorStamp ?: return
            if (frame?.enabled != true) return
            val selectionStart = input.selectionStart.coerceIn(0, editable.length)
            val selectionEnd = input.selectionEnd.coerceIn(0, editable.length)
            val markedStart = BaseInputConnection.getComposingSpanStart(editable)
            val markedEnd = BaseInputConnection.getComposingSpanEnd(editable)
            val ack = core.edit(
                ComposerEditorUpdate(
                    stamp = stamp,
                    text = editable.toString(),
                    selectionStartUtf16 = selectionStart,
                    selectionEndUtf16 = selectionEnd,
                    markedStartUtf16 = markedStart.takeIf { it >= 0 },
                    markedEndUtf16 = markedEnd.takeIf { it >= 0 },
                ),
            )
            editorStamp = ack.stamp
            applyAck(ack)
            refreshMentionQuery()
        }
    }

    init {
        root.addView(cards, matchWrap())
        root.addView(contextScroll, matchWrap())
        root.addView(attachmentScroll, matchWrap())
        root.addView(mentionRow, matchWrap())
        root.addView(sendError, matchWrap())
        inputRow.addView(attachmentButton, wrapWrap())
        inputRow.addView(input, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        inputRow.addView(sendButton, wrapWrap())
        root.addView(inputRow, matchWrap())
        input.addTextChangedListener(textWatcher)
        ViewCompat.setOnReceiveContentListener(input, arrayOf("image/*"), OnReceiveContentListener { _, payload ->
            val (images, remaining) = payload.partition { item ->
                item.uri != null && payload.clip.description.hasMimeType("image/*")
            }
            images?.clip?.let { clip ->
                for (index in 0 until clip.itemCount) {
                    clip.getItemAt(index).uri?.let { pastedImageListener?.invoke(it) }
                }
            }
            remaining
        })
        input.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_SEND && !isComposing()) {
                activatePrimary()
                true
            } else false
        }
        input.setOnKeyListener { _, keyCode, event ->
            if (event.action == KeyEvent.ACTION_UP) {
                input.post(::refreshMentionQuery)
            }
            val current = frame
            if (current != null && current.sendOnEnter &&
                keyCode == KeyEvent.KEYCODE_ENTER && event.action == KeyEvent.ACTION_DOWN &&
                (!event.isShiftPressed || event.isCtrlPressed || event.isMetaPressed) && !isComposing()
            ) {
                activatePrimary()
                true
            } else false
        }
        input.setOnTouchListener { _, event ->
            if (event.action == android.view.MotionEvent.ACTION_UP) {
                input.post(::refreshMentionQuery)
            }
            false
        }
        attachmentButton.setOnClickListener { showActions(it, frame?.attachmentActions.orEmpty()) }
        sendButton.setOnClickListener { activatePrimary() }
        sendButton.setOnLongClickListener {
            showChoices(sendButton)
            frame?.sendChoices?.isNotEmpty() == true
        }
        installInsets()
    }

    /** Returns the root Android View to attach to the chat screen. */
    fun view(): View = root

    fun setAttachmentPickerListener(listener: (() -> Unit)?) {
        checkMainThread()
        attachmentPickerListener = listener
    }

    fun setPastedImageListener(listener: ((Uri) -> Unit)?) {
        checkMainThread()
        pastedImageListener = listener
    }

    /** Applies a newer Rust snapshot while preserving the draft for the same input token. */
    fun render(next: ComposerFrame) {
        checkMainThread()
        check(!disposed) { "Composer presentation is closed" }
        validate(next)
        val previous = frame
        val newInput = previous == null ||
            previous.stamp.instance != next.stamp.instance ||
            previous.stamp.inputToken != next.stamp.inputToken ||
            previous.stamp.editingLifetime != next.stamp.editingLifetime
        if (previous != null && previous.stamp.instance == next.stamp.instance) {
            val sameView = next.copy(stamp = previous.stamp) == previous
            require(next.stamp.viewRevision > previous.stamp.viewRevision ||
                (next.stamp.viewRevision == previous.stamp.viewRevision && sameView)
            ) {
                "Composer view revision moved backwards"
            }
        }
        // The Rust editor accepts edits only for the view the Rust Native
        // adapter has mounted and acknowledged.
        renderSharedCards(next.sharedCardsJson)
        val mounted = core.mountEditor(next.stamp, next.maxBytes, next.initialDraft)
        frame = next.copy(stamp = mounted.stamp)
        editorStamp = mounted.stamp
        input.hint = next.placeholder
        input.isEnabled = next.enabled
        input.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
        if (newInput) {
            pendingAttachments.clear()
            renderAttachments(next.attachments)
            sendError.visibility = View.GONE
            setInputText(mounted.text, mounted.selectionStartUtf16, mounted.selectionEndUtf16)
        } else {
            applyAck(mounted)
        }

        val primary = if (next.busy) next.stopAction else null
        sendButton.text = primary?.label ?: next.primaryLabel
        sendButton.contentDescription = sendButton.text
        sendButton.isEnabled = if (next.busy) primary?.enabled == true else next.enabled
        attachmentButton.isEnabled = next.enabled && next.attachmentActions.isNotEmpty()
        attachmentButton.visibility = if (next.attachmentActions.isEmpty()) View.GONE else View.VISIBLE

        renderChips(contextRow, next.contextChips)
        renderAttachments(next.attachments)
        renderMentions(next.mentionSuggestions)
    }

    override fun close() {
        checkMainThread()
        if (disposed) return
        disposed = true
        sharedCardsMount?.close()
        sharedCardsMount = null
        input.removeTextChangedListener(textWatcher)
        ViewCompat.setOnReceiveContentListener(input, null, null)
        input.setOnEditorActionListener(null)
        input.setOnKeyListener(null)
        input.setOnTouchListener(null)
        input.setText("")
        attachmentButton.setOnClickListener(null)
        sendButton.setOnClickListener(null)
        sendButton.setOnLongClickListener(null)
        root.setOnApplyWindowInsetsListener(null)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            root.setWindowInsetsAnimationCallback(null)
        }
        root.removeAllViews()
        pendingAttachments.clear()
        attachmentPickerListener = null
        pastedImageListener = null
        mentionSearch?.cancel()
        mentionScope.cancel()
        activeMention = null
        frame = null
        editorStamp = null
    }

    private fun activatePrimary() {
        val current = frame ?: return
        if (!current.enabled) return
        if (current.busy) {
            current.stopAction?.takeIf { it.enabled }?.let { activate(it) }
        } else {
            submit(null)
        }
    }

    private fun activate(action: ComposerAction) {
        val current = frame ?: return
        val currentAction = sequenceOf(
            current.attachmentActions.asSequence(),
            current.mentionSuggestions.asSequence(),
            current.attachments.mapNotNull { it.removeAction }.asSequence(),
            listOfNotNull(current.stopAction).asSequence(),
        ).flatten().firstOrNull { it.node == action.node } ?: return
        if (!currentAction.enabled) return
        try {
            if (current.attachmentActions.any { it.node == currentAction.node }) {
                if (core.activateAttachment(
                        current.stamp.instance,
                        current.stamp.viewRevision,
                        currentAction.node,
                    )
                ) {
                    attachmentPickerListener?.invoke()
                }
            } else {
                core.activate(current.stamp.instance, current.stamp.viewRevision, currentAction.node)
            }
        } catch (_: Exception) {
            sendError.visibility = View.VISIBLE
        }
    }

    private fun showActions(anchor: View, actions: List<ComposerAction>) {
        if (actions.isEmpty()) return
        val menu = PopupMenu(context, anchor)
        actions.forEachIndexed { index, action ->
            menu.menu.add(0, index, index, action.label).isEnabled = action.enabled
        }
        menu.setOnMenuItemClickListener { item ->
            actions.getOrNull(item.itemId)?.let(::activate) != null
        }
        menu.show()
    }

    private fun showChoices(anchor: View) {
        if (frame?.sendChoices.isNullOrEmpty()) return
        val stamp = editorStamp ?: return
        val tokens = frame?.sendChoices.orEmpty().map(ComposerChoice::token)
        val menu = PopupMenu(context, anchor)
        frame?.sendChoices.orEmpty().forEachIndexed { index, choice ->
            menu.menu.add(0, index, index, choice.label)
        }
        menu.setOnMenuItemClickListener { item ->
            val current = frame ?: return@setOnMenuItemClickListener false
            if (current.stamp.instance != stamp.instance || current.stamp.inputToken != stamp.inputToken) {
                return@setOnMenuItemClickListener false
            }
            val token = tokens.getOrNull(item.itemId)?.takeIf { oldToken ->
                current.sendChoices.any { it.token == oldToken }
            }
                ?: return@setOnMenuItemClickListener false
            submit(token)
            true
        }
        menu.show()
    }

    /** Accept bytes from a platform picker after bounded URI reading/conversion. */
    fun addAttachment(payload: ComposerAttachmentPayload): Boolean {
        checkMainThread()
        if (disposed || frame?.enabled != true || payload.data.isEmpty() ||
            payload.data.size > ComposerAttachmentLimits.maxPerFileBytes ||
            pendingAttachments.size >= ComposerAttachmentLimits.MAX_ATTACHMENTS ||
            pendingAttachments.sumOf { it.data.size.toLong() } + payload.data.size >
            ComposerAttachmentLimits.MAX_TOTAL_ATTACHMENT_BYTES ||
            !byteBound(payload.name) || payload.name.isBlank() || !byteBound(payload.mimeType)
        ) return false
        pendingAttachments += payload.copy(data = payload.data.copyOf())
        renderAttachments(frame?.attachments.orEmpty())
        sendError.visibility = View.GONE
        return true
    }

    /** Shows a bounded attachment intake error without changing the current draft. */
    fun showAttachmentError(message: String) {
        checkMainThread()
        if (disposed || !byteBound(message) || message.isBlank()) return
        sendError.text = message
        sendError.visibility = View.VISIBLE
    }

    private fun submit(choiceToken: String?) {
        val current = frame ?: return
        if (!current.enabled || disposed) return
        val accepted = try {
            core.submit(
                editorStamp ?: current.stamp,
                choiceToken,
                input.text.toString(),
                pendingAttachments.toList(),
            )
        } catch (_: Exception) {
            false
        }
        if (accepted) {
            pendingAttachments.clear()
            renderAttachments(current.attachments)
            sendError.visibility = View.GONE
            try {
                val mounted = core.mountEditor(current.stamp, current.maxBytes, current.initialDraft)
                editorStamp = mounted.stamp
                applyAck(mounted)
            } catch (_: Exception) {
                setInputText("", 0, 0)
            }
        } else {
            sendError.visibility = View.VISIBLE
        }
    }

    private fun renderChips(target: LinearLayout, chips: List<ComposerChip>) {
        target.removeAllViews()
        chips.forEach { chip ->
            target.addView(TextView(context).apply {
                text = chip.label
                contentDescription = chip.label
                setPadding(dp(10), dp(4), dp(10), dp(4))
                importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_YES
            }, wrapWrap())
        }
        contextScroll.visibility = if (chips.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun renderAttachments(attachments: List<ComposerAttachment>) {
        attachmentRow.removeAllViews()
        attachments.forEach { attachment ->
            val row = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                contentDescription = attachment.label
            }
            row.addView(TextView(context).apply {
                text = attachment.label
                maxLines = 1
                setPadding(dp(8), dp(4), dp(8), dp(4))
            }, wrapWrap())
            attachment.removeAction?.let { action ->
                row.addView(Button(context).apply {
                    text = "×"
                    contentDescription = "Remove ${attachment.label}"
                    minimumWidth = dp(40)
                    minimumHeight = dp(40)
                    isEnabled = action.enabled
                    setOnClickListener { activate(action) }
                }, wrapWrap())
            }
            attachmentRow.addView(row, wrapWrap())
        }
        pendingAttachments.forEachIndexed { index, payload ->
            val row = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                contentDescription = payload.name
            }
            row.addView(TextView(context).apply {
                text = payload.name
                maxLines = 1
                setPadding(dp(8), dp(4), dp(8), dp(4))
            }, wrapWrap())
            row.addView(Button(context).apply {
                text = "×"
                contentDescription = "Remove ${payload.name}"
                minimumWidth = dp(40)
                minimumHeight = dp(40)
                setOnClickListener {
                    if (pendingAttachments.getOrNull(index) === payload) {
                        pendingAttachments.removeAt(index)
                        renderAttachments(frame?.attachments.orEmpty())
                    }
                }
            }, wrapWrap())
            attachmentRow.addView(row, wrapWrap())
        }
        attachmentScroll.visibility = if (attachments.isEmpty() && pendingAttachments.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun renderMentions(actions: List<ComposerAction>) {
        mentionRow.removeAllViews()
        actions.forEach { action ->
            mentionRow.addView(Button(context).apply {
                text = action.label
                contentDescription = "Insert ${action.label}"
                isEnabled = action.enabled && frame?.enabled == true
                minimumHeight = dp(44)
                setOnClickListener { activate(action) }
            }, matchWrap())
        }
        mentionRow.visibility = if (actions.isEmpty()) View.GONE else View.VISIBLE
    }

    private fun refreshMentionQuery() {
        val port = mentionPort ?: return
        mentionSearch?.cancel()
        val generation = ++mentionGeneration
        val query = ComposerMentionQuery.active(input.text.toString(), input.selectionStart)
        activeMention = query
        if (query == null || query.query.isEmpty() || BaseInputConnection.getComposingSpanStart(input.text) >= 0) {
            renderMentionFiles(emptyList())
            return
        }
        mentionSearch = mentionScope.launch {
            delay(MENTION_SEARCH_DELAY_MS)
            val files = try {
                port.searchFiles(query.query).take(MAX_MENTION_RESULTS)
            } catch (_: Exception) {
                emptyList()
            }
            if (disposed || generation != mentionGeneration || activeMention != query) return@launch
            renderMentionFiles(files)
        }
    }

    private fun renderMentionFiles(files: List<ComposerMentionFile>) {
        mentionRow.removeAllViews()
        files.forEach { file ->
            val label = file.path.substringAfterLast('/').ifBlank { file.path }
            val parent = file.path.substringBeforeLast('/', "")
            mentionRow.addView(Button(context).apply {
                text = if (parent.isBlank()) label else "$label  ·  $parent"
                contentDescription = if (file.isDirectory) "Insert folder $label" else "Insert file $label"
                isEnabled = frame?.enabled == true
                minimumHeight = dp(44)
                setOnClickListener { insertMention(file) }
            }, matchWrap())
        }
        if (files.isNotEmpty()) {
            mentionRow.visibility = View.VISIBLE
        } else if (frame?.mentionSuggestions.isNullOrEmpty()) {
            mentionRow.visibility = View.GONE
        }
    }

    private fun insertMention(file: ComposerMentionFile) {
        val port = mentionPort ?: return
        val query = activeMention ?: return
        val text = input.text.toString()
        if (query.endUtf16 > text.length || text.substring(query.startUtf16, query.endUtf16) != "@${query.query}") return
        val link = try {
            port.fileMentionLink(file)
        } catch (_: Exception) {
            sendError.visibility = View.VISIBLE
            return
        }
        if (link.isBlank() || !byteBound(link)) {
            sendError.visibility = View.VISIBLE
            return
        }
        val updated = text.replaceRange(query.startUtf16, query.endUtf16, link)
        applyingAck = true
        try {
            input.setText(updated)
            input.setSelection((query.startUtf16 + link.length).coerceAtMost(updated.length))
        } finally {
            applyingAck = false
        }
        val stamp = editorStamp ?: return
        val ack = try {
            core.edit(
                ComposerEditorUpdate(
                    stamp = stamp,
                    text = updated,
                    selectionStartUtf16 = input.selectionStart,
                    selectionEndUtf16 = input.selectionEnd,
                    markedStartUtf16 = null,
                    markedEndUtf16 = null,
                ),
            )
        } catch (_: Exception) {
            setInputText(text, query.endUtf16, query.endUtf16)
            activeMention = query
            sendError.visibility = View.VISIBLE
            return
        }
        editorStamp = ack.stamp
        applyAck(ack)
        if (ack.text != updated) {
            activeMention = ComposerMentionQuery.active(input.text.toString(), input.selectionStart)
            sendError.visibility = View.VISIBLE
            return
        }
        activeMention = null
        renderMentionFiles(emptyList())
    }

    private fun renderSharedCards(json: String?) {
        if (json == sharedCardsJson) return
        sharedCardsMount?.close()
        sharedCardsMount = null
        cards.removeAllViews()
        sharedCardsJson = json
        if (json != null) sharedCardsMount = rustNativeMount.mount(json, cards)
        cards.visibility = if (json == null) View.GONE else View.VISIBLE
    }

    private fun applyAck(ack: ComposerEditorAck) {
        val currentText = input.text.toString()
        val start = ack.selectionStartUtf16.coerceIn(0, ack.text.length)
        val end = ack.selectionEndUtf16.coerceIn(0, ack.text.length)
        val markedStart = ack.markedStartUtf16?.coerceIn(0, ack.text.length)
        val markedEnd = ack.markedEndUtf16?.coerceIn(0, ack.text.length)
        val currentMarkedStart = BaseInputConnection.getComposingSpanStart(input.text)
        val currentMarkedEnd = BaseInputConnection.getComposingSpanEnd(input.text)
        if (ack.text == currentText && input.selectionStart == start && input.selectionEnd == end &&
            currentMarkedStart == (markedStart ?: -1) && currentMarkedEnd == (markedEnd ?: -1)
        ) return
        applyingAck = true
        try {
            if (ack.text != currentText) input.setText(ack.text)
            val connection = BaseInputConnection(input, true)
            if (markedStart != null && markedEnd != null && markedEnd >= markedStart) {
                connection.setComposingRegion(markedStart, markedEnd)
            } else if (BaseInputConnection.getComposingSpanStart(input.text) >= 0) {
                connection.finishComposingText()
            }
            input.setSelection(minOf(start, end), max(start, end))
        } finally {
            applyingAck = false
        }
    }

    private fun setInputText(value: String, start: Int, end: Int) {
        applyingAck = true
        input.setText(value)
        input.setSelection(start.coerceIn(0, input.length()), end.coerceIn(0, input.length()))
        applyingAck = false
    }

    private fun isComposing(): Boolean =
        BaseInputConnection.getComposingSpanStart(input.text) >= 0

    private fun validate(next: ComposerFrame) {
        require(validId(next.stamp.instance) && validId(next.stamp.node))
        require(next.stamp.viewRevision > 0 && next.stamp.editorRevision >= 0)
        require(validId(next.stamp.inputToken) && validId(next.stamp.editingLifetime))
        require(next.stamp.editingLifetime.toULongOrNull() != null)
        require(next.primaryLabel.isNotBlank())
        require(next.maxBytes in 1..MAX_TEXT_BYTES)
        require(next.initialDraft.toByteArray(StandardCharsets.UTF_8).size <= next.maxBytes)
        require(byteBound(next.placeholder) && byteBound(next.primaryLabel))
        require(next.contextChips.size <= MAX_CHIPS)
        require(next.attachments.size <= MAX_ATTACHMENTS)
        require(next.attachmentActions.size <= MAX_ACTIONS)
        require(next.mentionSuggestions.size <= MAX_ACTIONS)
        require(next.sendChoices.size <= MAX_CHOICES)
        require(next.sendChoices.map(ComposerChoice::token).distinct().size == next.sendChoices.size)
        require(next.contextChips.map(ComposerChip::key).distinct().size == next.contextChips.size)
        require(next.attachments.map(ComposerAttachment::key).distinct().size == next.attachments.size)
        val actionKeys = (next.attachmentActions + next.mentionSuggestions + next.attachments.mapNotNull { it.removeAction } +
            listOfNotNull(next.stopAction)).map(ComposerAction::node)
        require(actionKeys.distinct().size == actionKeys.size)
        next.contextChips.forEach { require(validId(it.key) && byteBound(it.label)) }
        next.attachments.forEach {
            require(validId(it.key) && byteBound(it.label))
            it.removeAction?.let { action -> require(validId(action.node) && byteBound(action.label)) }
        }
        require(next.attachmentActions.size + next.mentionSuggestions.size + next.attachments.count { it.removeAction != null } <= MAX_ACTIONS * 2)
        (next.attachmentActions + next.mentionSuggestions + next.attachments.mapNotNull { it.removeAction } + listOfNotNull(next.stopAction)).forEach {
            require(validId(it.node) && byteBound(it.label))
        }
        next.sendChoices.forEach { require(validId(it.token) && byteBound(it.label)) }
        require((next.sharedCardsJson?.toByteArray(StandardCharsets.UTF_8)?.size ?: 0) <= MAX_VIEW_BYTES)
    }

    private fun installInsets() {
        root.setOnApplyWindowInsetsListener { view, insets ->
            val ime = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                insets.getInsets(WindowInsets.Type.ime()).bottom
            } else insets.systemWindowInsetBottom
            val bars = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                insets.getInsets(WindowInsets.Type.systemBars()).bottom
            } else 0
            setBottomInset(view, max(ime, bars))
            insets
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            root.setWindowInsetsAnimationCallback(object : WindowInsetsAnimation.Callback(
                WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE,
            ) {
                override fun onProgress(
                    insets: WindowInsets,
                    runningAnimations: MutableList<WindowInsetsAnimation>,
                ): WindowInsets {
                    val ime = insets.getInsets(WindowInsets.Type.ime()).bottom
                    val bars = insets.getInsets(WindowInsets.Type.systemBars()).bottom
                    setBottomInset(root, max(ime, bars))
                    return insets
                }
            })
        }
    }

    private fun setBottomInset(view: View, inset: Int) {
        view.setPadding(view.paddingLeft, view.paddingTop, view.paddingRight, baseBottomPadding + inset)
    }

    private fun checkMainThread() {
        check(android.os.Looper.myLooper() == android.os.Looper.getMainLooper()) {
            "Composer presentation must update on the main thread"
        }
    }

    private fun validId(value: String): Boolean =
        value.length in 1..96 && value.all { it.code in 0x21..0x7e && it in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" }

    private fun byteBound(value: String): Boolean =
        value.toByteArray(StandardCharsets.UTF_8).size <= MAX_TEXT_BYTES

    private fun horizontalRow() = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
    }

    private fun horizontalScroll(content: LinearLayout) = HorizontalScrollView(context).apply {
        isHorizontalScrollBarEnabled = false
        addView(content, FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT))
    }

    private fun dp(value: Int): Int = (value * density).toInt()

    private fun matchWrap() = LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT)
    private fun wrapWrap() = LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT)

    private companion object {
        const val MAX_TEXT_BYTES = 64 * 1024
        const val MAX_VIEW_BYTES = 512 * 1024
        const val MAX_CHIPS = 16
        const val MAX_ATTACHMENTS = ComposerAttachmentLimits.MAX_ATTACHMENTS
        const val MAX_ACTIONS = 16
        const val MAX_CHOICES = 4
        const val MAX_MENTION_RESULTS = 5
        const val MENTION_SEARCH_DELAY_MS = 120L
        const val MATCH_PARENT = ViewGroup.LayoutParams.MATCH_PARENT
        const val WRAP_CONTENT = ViewGroup.LayoutParams.WRAP_CONTENT
    }
}
