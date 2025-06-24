# ProseMirror Change Tracking Example

## Overview

This example demonstrates how ProseMirror allows tracking document changes as first-class values, enabling features like:
- Committing changes with messages
- Reverting individual commits
- Identifying the origin of specific text segments (blame)
- Viewing change history

## HTML Structure

```html
<div id="commit">
  <input type="text" placeholder="Commit message">
  <button>Commit</button>
</div>
<div id="editor"></div>
<div id="commits"></div>
```

## CSS Styles

```css
.commit { 
  margin-bottom: 4px; 
}
.commit:hover { 
  background: #ff8; 
}
.commit-revert { 
  color: #a22; 
}
.commit-time { 
  background: #5ab; 
  padding: 0 5px; 
  color: white; 
  font-size: 90%; 
}
.commit-blame { 
  background: #ff8; 
}
```

## JavaScript Code

```javascript
import {Plugin} from "prosemirror-state"
import {Mapping} from "prosemirror-transform"
import {ReplaceStep, ReplaceAroundStep} from "prosemirror-transform"

// Represents a range in the document attributed to a specific commit
class Span {
  constructor(from, to, commit) {
    this.from = from
    this.to = to
    this.commit = commit
  }

  map(mapping) {
    let from = mapping.mapPos(this.from)
    let to = mapping.mapPos(this.to)
    return from < to ? new Span(from, to, this.commit) : null
  }
}

// Represents a single commit
class Commit {
  constructor(message, time, steps, maps) {
    this.message = message
    this.time = time
    this.steps = steps
    this.maps = maps
  }
}

// Tracks the state of all commits and uncommitted changes
class TrackState {
  constructor(blameMap, commits, uncommittedSteps, uncommittedMaps) {
    this.blameMap = blameMap
    this.commits = commits
    this.uncommittedSteps = uncommittedSteps
    this.uncommittedMaps = uncommittedMaps
  }

  // Apply a transform to the tracked state
  applyTransform(transform) {
    // Invert the steps so we can replay them to revert
    let inverted = transform.steps.map((step, i) => step.invert(transform.docs[i]))
    let newBlame = updateBlameMap(this.blameMap, transform, this.commits.length)
    return new TrackState(newBlame, this.commits,
                          this.uncommittedSteps.concat(inverted),
                          this.uncommittedMaps.concat(transform.mapping.maps))
  }

  // Create a commit from uncommitted changes
  applyCommit(message, time) {
    if (this.uncommittedSteps.length == 0) return this
    let commit = new Commit(message, time, this.uncommittedSteps,
                            this.uncommittedMaps)
    return new TrackState(this.blameMap, this.commits.concat(commit), [], [])
  }
}

// Update blame map when document changes
function updateBlameMap(blameMap, transform, commitID) {
  let result = []
  // Map existing spans through the transform
  for (let span of blameMap) {
    let mapped = span.map(transform.mapping)
    if (mapped) result.push(mapped)
  }
  
  // Add new spans for inserted content
  for (let i = 0; i < transform.steps.length; i++) {
    let step = transform.steps[i], map = transform.mapping.maps[i]
    let from = null, to = null
    
    if (step instanceof ReplaceStep) {
      from = step.from
      to = step.to
    } else if (step instanceof ReplaceAroundStep) {
      from = step.from
      to = step.to
    }
    
    if (from != null) {
      let mappedFrom = map.map(from), mappedTo = map.map(to)
      if (mappedTo > mappedFrom)
        result.push(new Span(mappedFrom, mappedTo, commitID))
    }
  }
  
  return result.sort((a, b) => a.from - b.from)
}

// Plugin to track changes
const trackPlugin = new Plugin({
  state: {
    init(_, instance) {
      return new TrackState([new Span(0, instance.doc.content.size, null)], [], [], [])
    },
    apply(tr, tracked) {
      if (tr.docChanged) tracked = tracked.applyTransform(tr)
      let commitMessage = tr.getMeta(this)
      if (commitMessage) tracked = tracked.applyCommit(commitMessage, new Date(tr.time))
      return tracked
    }
  }
})

// Revert a specific commit
function revertCommit(commit, state, dispatch) {
  let trackState = trackPlugin.getState(state)
  let index = trackState.commits.indexOf(commit)
  
  // Can't revert if not in history
  if (index == -1) return

  // Must commit current changes first
  if (trackState.uncommittedSteps.length)
    return alert("Commit your changes first!")

  // Build mapping from commit time to now
  let remap = new Mapping(trackState.commits.slice(index)
                          .reduce((maps, c) => maps.concat(c.maps), []))
  let tr = state.tr
  
  // Apply inverted steps in reverse order
  for (let i = commit.steps.length - 1; i >= 0; i--) {
    let remapped = commit.steps[i].map(remap.slice(i + 1))
    if (!remapped) continue
    let result = tr.maybeStep(remapped)
    if (result.doc) remap.appendMap(remapped.getMap(), i)
  }
  
  // Dispatch revert as a new commit
  if (tr.docChanged)
    dispatch(tr.setMeta(trackPlugin, `Revert '${commit.message}'`))
}

// Highlight text from a specific commit
function highlightCommit(state, commit) {
  let trackState = trackPlugin.getState(state)
  let decorations = []
  
  trackState.blameMap.forEach(span => {
    if (span.commit === trackState.commits.indexOf(commit)) {
      decorations.push(
        Decoration.inline(span.from, span.to, {class: "commit-blame"})
      )
    }
  })
  
  return DecorationSet.create(state.doc, decorations)
}

// UI Setup
function setupCommitUI(view) {
  const commitButton = document.querySelector("#commit button")
  const commitInput = document.querySelector("#commit input")
  const commitList = document.querySelector("#commits")
  
  commitButton.addEventListener("click", () => {
    let message = commitInput.value
    if (!message) return
    
    view.dispatch(view.state.tr.setMeta(trackPlugin, message))
    commitInput.value = ""
    updateCommitList()
  })
  
  function updateCommitList() {
    let trackState = trackPlugin.getState(view.state)
    commitList.innerHTML = ""
    
    trackState.commits.forEach((commit, index) => {
      let div = commitList.appendChild(document.createElement("div"))
      div.className = "commit"
      div.innerHTML = `
        <span class="commit-time">${commit.time.toLocaleString()}</span>
        ${commit.message}
        <button class="commit-revert">revert</button>
      `
      
      div.querySelector(".commit-revert").addEventListener("click", () => {
        revertCommit(commit, view.state, view.dispatch)
      })
      
      div.addEventListener("mouseover", () => {
        view.dom.classList.add("blame-mode")
        // Highlight text from this commit
      })
      
      div.addEventListener("mouseout", () => {
        view.dom.classList.remove("blame-mode")
      })
    })
  }
  
  updateCommitList()
}

// Initialize editor
import {EditorState} from "prosemirror-state"
import {EditorView} from "prosemirror-view"
import {schema} from "prosemirror-schema-basic"
import {exampleSetup} from "prosemirror-example-setup"

window.view = new EditorView(document.querySelector("#editor"), {
  state: EditorState.create({
    schema,
    plugins: exampleSetup({schema}).concat(trackPlugin)
  })
})

setupCommitUI(window.view)
```

## Key Concepts

### Blame Map
- Maintains a list of `Span` objects
- Each span tracks which commit introduced specific text
- Updated incrementally as document changes
- Survives through subsequent edits

### Change Tracking
- **Uncommitted changes**: Stored as inverted steps
- **Commits**: Bundle steps with message and timestamp
- **Transform tracking**: Updates blame map on every change
- **Incremental updates**: Efficient tracking without full recalculation

### Reversion Algorithm
1. Find all commits after the target commit
2. Build a mapping through all subsequent changes
3. Rebase the inverted steps to current document
4. Apply rebased steps in reverse order
5. Create new commit for the reversion

### Architecture Benefits
- **Non-destructive**: Reversion creates new commits
- **Transparent**: All changes are tracked
- **Flexible**: Can revert any commit in history
- **Efficient**: Incremental blame updates

## Usage
1. Make edits to the document
2. Enter a commit message and click "Commit"
3. Hover over commits to see attributed text
4. Click "revert" to undo a specific commit
5. Continue editing with full history preserved

## Limitations
- Cannot revert with uncommitted changes
- Reversion may fail if conflicts arise
- Blame tracking increases memory usage
- Complex transforms may be expensive to track