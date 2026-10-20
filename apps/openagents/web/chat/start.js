window.__oaVisualUrl = new URL("./visuals/start.js", import.meta.url).href;
// Starts the GPUI web chat (crates/openagents-web/src/pages/gpui_chat.rs).
// It selects WebGPU or WebGL2 before fetching the module.
// The module and the fonts download together,
// and the canvas replaces the page's outline once the app has started.
import { loadRenderer } from "./renderer.js";
var chatModule = null;
function gpuiRecordedFrame() { return chatModule && chatModule.gpuiRecordedFrame(); }
function chatShortcut(ctrl, alt, shift, meta, key) {
  return !!chatModule && chatModule.chatShortcut(ctrl, alt, shift, meta, key);
}

const FONTS = [
  "Geist.ttf",
  "Geist-Italic.ttf",
  "Geist-Medium.ttf",
  "Geist-SemiBold.ttf",
  "Geist-SemiBoldItalic.ttf",
  "Geist-Bold.ttf",
  "Geist-BoldItalic.ttf",
  "GeistMono.ttf",
  "GeistMono-Medium.ttf",
  "GeistMono-SemiBold.ttf",
];

// The fonts the chat fetched, which the file panel draws with too.
var loadedFonts = null;

function say(text) {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.textContent = text;
  }
}

// If neither renderer starts, the HTML chat takes over: the page goes to
// its address (the note's link), and shows the note if it can't.
function unsupported() {
  var params = new URLSearchParams(location.search);
  var settingsPage = params.get("settings") || (params.get("page") || "").replace(/^keys\//, "");
  if (isKeysPage(settingsPage)) {
    location.replace("/settings/" + settingsPage + "?ui=html");
    return;
  }
  var status = document.getElementById("gpui-status");
  if (status) {
    status.remove();
  }
  var note = document.getElementById("gpui-unsupported");
  if (note) {
    note.hidden = false;
    var link = note.querySelector("a");
    if (link) {
      if (settingsSection()) htmlSettings();
      else location.replace(link.href);
    }
  }
}

function start() {
  var started = performance.now();
  // The site's shared font files when the page names them (preloads with
  // `data-gpui-font`, in FONTS order), else the build's own copies.
  var shared = document.querySelectorAll("link[data-gpui-font]");
  var urls = shared.length === FONTS.length
    ? Array.prototype.map.call(shared, function (link) { return link.href; })
    : FONTS.map(function (name) { return new URL("fonts/" + name, import.meta.url); });
  var fonts = Promise.all(
    urls.map(function (url, at) {
      var name = FONTS[at];
      return fetch(url).then(function (response) {
        if (!response.ok) {
          throw new Error("font " + name + ": " + response.status);
        }
        return response.arrayBuffer();
      }).then(function (buffer) {
        return new Uint8Array(buffer);
      });
    })
  );
  document.addEventListener("gpui-first-frame", function () {
    // run_embedded opens its window after launch returns.
    chatInput = document.activeElement;
    var shell = document.getElementById("gpui-shell");
    if (shell) {
      shell.style.display = "none";
    }
    // Measure the actual first frame, including renderer initialization.
    window.__gpuiReadyAt = performance.now();
    window.__gpuiReadyMs = window.__gpuiReadyAt - started;
    document.documentElement.dataset.gpui = "ready";
    if (!document.querySelector('meta[name="openagents-settings"]')) {
      if (!openAddressedPage() && settingsSection()) openPage(settingsSection());
    }
  }, { once: true });
  // Once the chats and the open chat have arrived (`gpui-ready`), keep
  // that frame in this browser, so the page can paint it before the module
  // on the next visit to this address at this size (#12244). It never
  // leaves the browser; a change in the page's outline of chats retires it.
  document.addEventListener("gpui-ready", function () {
    var scene = document.getElementById("first-frame-scene");
    if (!scene || !scene.dataset.cache || !window.caches || !window.CompressionStream || !window.__firstFrameKey) {
      return;
    }
    var frame = gpuiRecordedFrame();
    if (!frame) {
      return;
    }
    var rect = document.body.getBoundingClientRect();
    var address = window.__firstFrameKey(scene, { w: rect.width, h: rect.height, dpr: window.devicePixelRatio });
    var prefix = location.origin + scene.dataset.cache + "?";
    // Frames kept for another account on this browser go too.
    var account = location.origin + scene.dataset.cache.split("/").slice(0, 4).join("/") + "/";
    var packed = new Response(
      new Blob([frame]).stream().pipeThrough(new CompressionStream("gzip")),
    ).arrayBuffer();
    Promise.all([caches.open("openagents-first-frame"), packed])
      .then(function (ready) {
        var store = ready[0];
        return store.keys().then(function (requests) {
          // One frame an address: the older sizes and outlines go, and
          // any other account's.
          return Promise.all(requests.filter(function (request) {
            var url = request.url;
            return url.indexOf(account) !== 0 || (url.indexOf(prefix) === 0 && url !== address);
          }).map(function (request) {
            return store.delete(request);
          }));
        }).then(function () {
          return store.put(address, new Response(ready[1], {
            headers: { "content-type": "application/octet-stream" },
          }));
        });
      })
      .then(function () {
        performance.mark("first-frame-kept-for-next-visit");
      })
      .catch(function (error) {
        console.warn("couldn't keep this frame for the next visit", error);
      });
  }, { once: true });
  loadedFonts = fonts;
  if (document.querySelector('meta[name="openagents-settings"]')) {
    var page = (new URLSearchParams(location.search).get("page") || document.querySelector('meta[name="openagents-settings"]').content || "general").replace(/^keys\//, "");
    if (PAGES[page.split("/")[0]] && !isKeysPage(page)) {
      openPage(page);
    } else {
      openSettings({ detail: JSON.stringify({ page: page, shared: { signedIn: true } }) });
    }
    window.addEventListener("oa-settings-close", function () { location.assign("/?ui=gpui"); });
    window.addEventListener("oa-settings-page", function (event) {
      history.replaceState(null, "", "/settings?ui=gpui&page=" + encodeURIComponent(event.detail));
    });
    return;
  }
  Promise.all([loadRenderer(new URL("./openagents_web_chat.js", import.meta.url), new URL("./openagents_web_chat_bg.wasm", import.meta.url), new URL("./openagents_web_chat_webgl.js", import.meta.url), new URL("./openagents_web_chat_webgl_bg.wasm", import.meta.url)), fonts])
    .then(function (loaded) {
      chatModule = loaded[0];
      chatModule.launch(loaded[1]);
    })
    .catch(function (error) {
      unsupported();
      console.error(error);
    });
}

// The side panel and the text view follow the open chat: the canvas moves
// between chats with history.pushState, and Back and Forward fire popstate;
// either tells the page's fragments (`oa-chat`) to read again.
function chatChanged() {
  window.dispatchEvent(new Event("oa-chat"));
}
var pushState = history.pushState;
history.pushState = function () {
  var result = pushState.apply(this, arguments);
  chatChanged();
  return result;
};
window.addEventListener("popstate", chatChanged);

// A message sent from the side panel's "Send with files" goes through the
// HTML chat's own form; the canvas then reads the chat again (as Back does)
// and follows the new answer.
document.addEventListener("htmx:afterRequest", function (event) {
  var form = event.detail && event.detail.elt;
  if (!form || form.id !== "chat-form" || !event.detail.successful) {
    return;
  }
  var files = document.getElementById("gpui-files");
  if (files) {
    files.open = false;
  }
  window.dispatchEvent(new PopStateEvent("popstate"));
});

// The text view: its button, Cmd+F or Ctrl+F (the browser's find then reads
// the chat's text there), and Escape to close it.
var textView = document.getElementById("gpui-text");
var textToggle = document.getElementById("gpui-text-toggle");
function showText(open) {
  if (!textView || !textToggle) {
    return;
  }
  textView.classList.toggle("g-open", open);
  textToggle.setAttribute("aria-expanded", open ? "true" : "false");
  textToggle.textContent = open ? "Back to the chat" : "Show the chat as text";
}
if (textToggle) {
  textToggle.addEventListener("click", function () {
    showText(!textView.classList.contains("g-open"));
  });
}
// A card in the canvas opens the text view at the newest card (`#text`),
// where the card draws itself.
window.addEventListener("hashchange", function () {
  if (location.hash !== "#text") {
    return;
  }
  history.replaceState(null, "", location.pathname + location.search);
  showText(true);
  var cards = textView ? textView.querySelectorAll("[class*='card']") : [];
  if (cards.length) {
    cards[cards.length - 1].scrollIntoView({ block: "center" });
  } else if (textView) {
    textView.scrollTop = textView.scrollHeight;
  }
});
window.addEventListener("keydown", function (event) {
  if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === "f") {
    showText(true);
  } else if (event.key === "Escape" && textView && textView.classList.contains("g-open")) {
    showText(false);
  }
}, true);

// The file panel (#12271): its own module, loaded the first time the
// chat's folder button asks for it, in a box on the right. It shows the
// open chat's files on the computer the chat runs on (`oa-host`).
var chatInput = null;
var paletteInputBefore = null;
var host = "";
var files = null;
var filesReady = false;
var filesBox = null;
var pendingFile = null;
function revealFile() {
  if (!files || !filesReady || !pendingFile) return;
  var parts = hostParts();
  var hit = pendingFile;
  pendingFile = null;
  if (hit.device === parts[0] && hit.chat === parts[1]) {
    files.open_file(hit.device, hit.chat, hit.path, hit.line == null ? undefined : hit.line);
  }
}
function hostParts() {
  var at = host.indexOf("\n");
  return at < 0 ? ["", ""] : [host.slice(0, at), host.slice(at + 1)];
}
function showFiles(open) {
  if (filesBox) {
    filesBox.hidden = !open;
  }
}
function openFiles() {
  if (filesBox) {
    showFiles(filesBox.hidden);
    return;
  }
  filesBox = document.createElement("div");
  filesBox.id = filesBox.dataset.box = "gpui-files-panel";
  filesBox.setAttribute("aria-label", "Files");
  filesBox.style.cssText = "position:fixed;top:8px;right:8px;bottom:8px;width:min(560px,46vw);z-index:5;border-radius:14px;overflow:hidden;";
  document.body.appendChild(filesBox);
  var moduleLoading = loadRenderer(new URL("./openagents_web_files.js", import.meta.url), new URL("./openagents_web_files_bg.wasm", import.meta.url), new URL("./openagents_web_files_webgl.js", import.meta.url), new URL("./openagents_web_files_webgl_bg.wasm", import.meta.url));
  Promise.all([moduleLoading, loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return Promise.resolve().then(function () {
        files = module;
        var parts = hostParts();
        mountIn(filesBox, function () {
          module.mount(loaded[1], parts[0], parts[1]);
          revealFile();
        });
      });
    })
    .catch(function (error) {
      console.error("the file panel couldn't start", error);
      filesBox.remove();
      filesBox = null;
      filesReady = false;
    });
}
// Other GPUI canvases have their own inputs. Forward an unhandled app
// shortcut to the chat, whose Rust keymap decides whether it is assigned.
window.addEventListener("keydown", function (event) {
  if (!chatInput || event.defaultPrevented || event.isComposing || event.repeat) return;
  if (chatShortcut(event.ctrlKey, event.altKey, event.shiftKey, event.metaKey, event.key)) {
    event.preventDefault();
  }
});
window.addEventListener("oa-chat-focus", function () {
  queueMicrotask(function () { if (chatInput) chatInput.focus(); });
});
var paletteFilesWereOpen = false;
window.addEventListener("oa-palette-open", function () {
  paletteInputBefore = document.activeElement;
  paletteFilesWereOpen = !!filesBox && !filesBox.hidden;
  showFiles(false);
  queueMicrotask(function () { if (chatInput) chatInput.focus(); });
});
window.addEventListener("oa-palette-close", function () {
  if (paletteFilesWereOpen) showFiles(true);
  paletteFilesWereOpen = false;
  var before = paletteInputBefore;
  queueMicrotask(function () { if (before && before.isConnected) before.focus(); });
});
window.addEventListener("oa-files-ready", function () {
  // The window opens after mount returns. Wait until its Rust callback
  // finishes before entering that app again, and use the latest chat.
  queueMicrotask(function () {
    filesReady = true;
    var parts = hostParts();
    files.show(parts[0], parts[1]);
    revealFile();
  });
});
window.addEventListener("oa-files-toggle", openFiles);
window.addEventListener("oa-files-open", function (event) {
  var hit;
  try { hit = JSON.parse(event.detail); } catch (_) { return; }
  var parts = hostParts();
  if (hit.device !== parts[0] || hit.chat !== parts[1] || typeof hit.path !== "string") return;
  pendingFile = hit;
  if (!filesBox) openFiles();
  showFiles(true);
  revealFile();
});
window.addEventListener("oa-files-close", function () {
  showFiles(false);
});
window.addEventListener("oa-host", function (event) {
  pendingFile = null;
  host = event.detail || "";
  if (files) {
    var parts = hostParts();
    files.show(parts[0], parts[1]);
  }
  if (!host) {
    showFiles(false);
  }
});

// A computer chat's live transcript (#12224): its own module, loaded the
// first time a computer chat opens (`oa-room-open`, with the chat's id); it
// sends each change back as `oa-room`.
var room = null;
var roomLoading = null;
var roomChat = "";
function loadRoom() {
  if (!roomLoading) {
    var glue = new URL("./openagents_web_room.js", import.meta.url);
    roomLoading = import(glue.href).then(function (module) {
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_room_bg.wasm", import.meta.url))),
      }).then(function () {
        room = module;
        return module;
      });
    });
  }
  return roomLoading;
}
// Both run after the canvas's own work: the canvas sends them from inside
// its update, which must not re-enter either module.
window.addEventListener("oa-room-open", function (event) {
  var chat = event.detail || "";
  setTimeout(function () {
    roomChat = chat;
    loadRoom()
      .then(function (module) {
        if (roomChat === chat) {
          module.open(chat);
        }
      })
      .catch(function (error) {
        console.error("the chat's transcript couldn't load", error);
        roomLoading = null;
        window.dispatchEvent(new CustomEvent("oa-room", {
          detail: JSON.stringify({ chat: chat, entries: [], caught_up: false, unavailable: true }),
        }));
      });
  }, 0);
});
window.addEventListener("oa-room-close", function () {
  setTimeout(function () {
    roomChat = "";
    if (room) {
      room.close();
    }
  }, 0);
});

// Dictation (#12292): its own module, loaded the first time the composer's
// microphone asks for it (`oa-dictation-start`, with the chat's computer).
// The AudioContext is made here, during the click, so Safari lets it run.
// The module sends each step back as `oa-dictation`.
var dictation = null;
var dictationLoading = null;
function loadDictation() {
  if (!dictationLoading) {
    var glue = new URL("./openagents_web_dictation.js", import.meta.url);
    dictationLoading = import(glue.href).then(function (module) {
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_dictation_bg.wasm", import.meta.url))),
      }).then(function () {
        dictation = module;
        return module;
      });
    });
  }
  return dictationLoading;
}
function dictationFailed(message) {
  window.dispatchEvent(new CustomEvent("oa-dictation", {
    detail: JSON.stringify({ state: "failed", message: message }),
  }));
}
window.addEventListener("oa-dictation-start", function (event) {
  var device = event.detail || "";
  var Context = window.AudioContext || window.webkitAudioContext;
  if (!Context) {
    dictationFailed("This browser can't record.");
    return;
  }
  var context = new Context();
  loadDictation()
    .then(function (module) {
      module.start(device, context);
    })
    .catch(function (error) {
      console.error("dictation couldn't load", error);
      dictationLoading = null;
      context.close();
      dictationFailed("Dictation didn't start.");
    });
});
window.addEventListener("oa-dictation-stop", function () {
  if (dictation) {
    dictation.stop();
  }
});
window.addEventListener("oa-dictation-cancel", function () {
  if (dictation) {
    dictation.cancel();
  }
});

// The work panel (#12225): work runs, environments, and Mac jobs. Its
// own module, loaded the first time the header's work button asks for it,
// in a box on the right.
var workBox = null;
var workModule = null;
// `target` names an item to open (#12332): `work:ID`, `env:ID`,
// `env:ID/run:RUN`, or `mac:ID`, as `?work=` carries it.
function openWork(target) {
  target = typeof target === "string" ? target : "";
  if (workBox) {
    if (target) {
      workBox.hidden = false;
      if (workModule) {
        workModule.focus(target);
      }
    } else {
      workBox.hidden = !workBox.hidden;
    }
    return;
  }
  workBox = document.createElement("div");
  workBox.id = workBox.dataset.box = "gpui-work";
  workBox.setAttribute("aria-label", "Work");
  workBox.style.cssText = "position:fixed;top:8px;right:8px;bottom:8px;width:min(560px,46vw);z-index:6;border-radius:14px;overflow:hidden;";
  document.body.appendChild(workBox);
  var moduleLoading = loadRenderer(new URL("./openagents_web_work.js", import.meta.url), new URL("./openagents_web_work_bg.wasm", import.meta.url), new URL("./openagents_web_work_webgl.js", import.meta.url), new URL("./openagents_web_work_webgl_bg.wasm", import.meta.url));
  Promise.all([moduleLoading, loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return Promise.resolve().then(function () {
        workModule = module;
        mountIn(workBox, function () {
          module.mount(loaded[1], target || undefined);
        });
      });
    })
    .catch(function (error) {
      console.error("the work panel couldn't start", error);
      workBox.remove();
      workBox = null;
    });
}
window.addEventListener("oa-work-toggle", function () {
  openWork("");
});
window.addEventListener("oa-work-open", function (event) {
  openWork(event.detail || "env");
});
window.addEventListener("oa-work-close", function () {
  if (workBox) {
    workBox.hidden = true;
  }
});
window.addEventListener("oa-work-failed", function () {
  if (workBox) {
    workBox.remove();
    workBox = null;
  }
});

// A lazily loaded module's canvas goes in the box named `gpui-mount` when
// its window opens, which happens after `mount` returns. The box keeps the
// name until its canvas is in it, then takes its own (`data-box`), so the
// next module finds its own box.
function mountIn(box, mount) {
  document.querySelectorAll("#gpui-mount").forEach(function (other) {
    other.id = other.dataset.box || "";
  });
  box.id = "gpui-mount";
  mount();
  var tries = 0;
  (function settle() {
    if (box.querySelector("canvas") || tries++ > 200) {
      box.id = box.dataset.box;
    } else {
      setTimeout(settle, 25);
    }
  })();
}

// Settings (#12288, #12289, #12290): its own module, loaded the first time
// the chat opens it (`oa-settings-open`, with the page and what the chat
// knows), in a box over the chat. It saves choices to this browser's
// storage and tells the chat (`oa-settings-changed`); its Back goes
// through the chat (`oa-settings-close`), which hides the box
// (`oa-settings-hide`).
var settings = null;
var settingsBox = null;
var settingsWanted = null;
function settingsSection() {
  var path = location.pathname;
  if (path === "/settings") return new URLSearchParams(location.search).get("page") || "general";
  if (path === "/settings/connections") return "connections";
  if (path === "/settings/terminal") return "terminal";
  if (path === "/settings/vault" || /^\/projects\/[^/]+\/vault$/.test(path)) return "vault";
  return null;
}

function htmlSettings() {
  var url = new URL(location.href);
  var page = url.searchParams.get("page");
  if (url.pathname === "/settings" && ["connections", "vault", "terminal"].indexOf(page) !== -1) {
    url.pathname += "/" + page;
  }
  url.searchParams.set("ui", "html");
  url.searchParams.delete("page");
  location.replace(url.pathname + url.search + url.hash);
}

function openSettings(event) {
  var asked = {};
  try {
    asked = JSON.parse(event.detail || "{}");
  } catch (error) {
    asked = {};
  }
  var page = asked.page || "general";
  if (["connections", "vault", "terminal"].indexOf(page) !== -1) { openPage(page); return; }
  var shared = JSON.stringify(asked.shared || {});
  settingsShared = asked.shared || settingsShared;
  if (isKeysPage(page)) {
    if (settingsBox) settingsBox.hidden = true;
    openPage("keys/" + page);
    return;
  }
  hideKeys();
  if (settingsBox) {
    settingsBox.hidden = false;
    if (settings) {
      settings.show(page, shared);
    } else {
      settingsWanted = [page, shared];
    }
    return;
  }
  settingsWanted = [page, shared];
  settingsBox = document.createElement("div");
  settingsBox.id = settingsBox.dataset.box = "gpui-settings";
  settingsBox.setAttribute("aria-label", "Settings");
  settingsBox.style.cssText = "position:fixed;inset:0;z-index:7;";
  if (document.querySelector('meta[name="openagents-settings"]')) {
    settingsBox.style.zIndex = "100";
  }
  document.body.appendChild(settingsBox);
  var moduleLoading = loadRenderer(new URL("./openagents_web_settings.js", import.meta.url), new URL("./openagents_web_settings_bg.wasm", import.meta.url), new URL("./openagents_web_settings_webgl.js", import.meta.url), new URL("./openagents_web_settings_webgl_bg.wasm", import.meta.url));
  Promise.all([moduleLoading, loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return Promise.resolve().then(function () {
        settings = module;
        var wanted = settingsWanted;
        mountIn(settingsBox, function () {
          module.mount(loaded[1], wanted[0], wanted[1]);
        });
      });
    })
    .catch(function (error) {
      console.error("Settings couldn't start", error);
      settingsBox.remove();
      settingsBox = null;
      if (settingsSection()) htmlSettings();
    });
}
window.addEventListener("oa-settings-open", openSettings);

// Transcript extras: the footer and host watches load only for
// a chat on a computer. Keep the latest context while the module starts.
var transcriptExtras = null;
var extrasBox = null;
var extrasLoading = null;
var extrasReady = false;
var extrasWanted = {};
var extrasOpen = false;
var extrasBounds = null;
function showExtras() {
  if (!extrasBox) return;
  extrasBox.style.visibility = extrasWanted.device ? "visible" : "hidden";
  var center = innerWidth > 820 ? (innerWidth + 272) / 2 : innerWidth / 2;
  var width = extrasBounds ? extrasBounds.width : Math.min(704, innerWidth - 48);
  var height = extrasOpen ? 442 : 76;
  extrasBox.style.width = width + "px";
  extrasBox.style.left = (extrasBounds ? extrasBounds.x : Math.max(8, center - width / 2)) + "px";
  extrasBox.style.bottom = (extrasBounds ? innerHeight - extrasBounds.bottom : 24) + "px";
  extrasBox.style.height = height + "px";
}
function flushExtras() {
  if (transcriptExtras && extrasReady) transcriptExtras.show(JSON.stringify(extrasWanted));
}
function loadExtras() {
  if (extrasLoading || transcriptExtras || !extrasWanted.device) return;
  extrasBox = document.createElement("div");
  extrasBox.id = extrasBox.dataset.box = "gpui-transcript-extras";
  extrasBox.setAttribute("aria-label", "Chat controls");
  extrasBox.style.cssText = "position:fixed;bottom:24px;z-index:6;";
  document.body.appendChild(extrasBox);
  showExtras();
  var moduleLoading = loadRenderer(
    new URL("./openagents_web_transcript_extras.js", import.meta.url),
    new URL("./openagents_web_transcript_extras_bg.wasm", import.meta.url),
    new URL("./openagents_web_transcript_extras_webgl.js", import.meta.url),
    new URL("./openagents_web_transcript_extras_webgl_bg.wasm", import.meta.url));
  extrasLoading = Promise.all([moduleLoading, loadedFonts]).then(function (loaded) {
    transcriptExtras = loaded[0];
    mountIn(extrasBox, function () { transcriptExtras.mount(loaded[1], JSON.stringify(extrasWanted)); });
  }).catch(function () {
    extrasBox.remove();
    extrasBox = null;
    extrasLoading = null;
    extrasReady = false;
    transcriptExtras = null;
    window.dispatchEvent(new CustomEvent("oa-extras-state", {detail: JSON.stringify({
      chat: extrasWanted.chat, generation: extrasWanted.generation,
      notice: "The chat controls couldn't load. Reconnect to try again.",
    })}));
  });
}
window.addEventListener("oa-extras-context", function (event) {
  try { extrasWanted = JSON.parse(event.detail || "{}"); } catch (_) { return; }
  extrasOpen = false;
  showExtras();
  flushExtras();
  loadExtras();
});
window.addEventListener("oa-extras-usage", function (event) {
  var usage;
  try { usage = JSON.parse(event.detail); } catch (_) { return; }
  if (usage.chat !== extrasWanted.chat || usage.generation !== extrasWanted.generation) return;
  Object.assign(extrasWanted, usage);
  flushExtras();
});
window.addEventListener("oa-extras-ready", function () {
  queueMicrotask(function () {
    extrasReady = true;
    flushExtras();
    if (chatInput) chatInput.focus();
  });
});
window.addEventListener("oa-extras-state", function (event) {
  var state;
  try { state = JSON.parse(event.detail); } catch (_) { return; }
  if (state.chat !== extrasWanted.chat || state.generation !== extrasWanted.generation) return;
  extrasOpen = !!state.open;
  showExtras();
});
window.addEventListener("oa-extras-close", function () {
  extrasOpen = false;
  showExtras();
  queueMicrotask(function () { if (chatInput) chatInput.focus(); });
});
window.addEventListener("oa-extras-bounds", function (event) {
  try { extrasBounds = JSON.parse(event.detail); } catch (_) { return; }
  showExtras();
});
window.addEventListener("oa-settings-changed", flushExtras);
if (window.matchMedia) window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", flushExtras);
window.addEventListener("resize", showExtras);
window.__oaReadTranscriptImage = function (device, chat, path) {
  loadExtras();
  return Promise.resolve(extrasLoading).then(function () {
    if (!transcriptExtras) throw new Error("The image reader couldn't load. Reconnect to try again.");
    return transcriptExtras.readImage(device, chat, path);
  });
};
// End transcript extras.

// The account's pages (#12329, #12330, #12331): Traces, Agents, and
// Projects, each its own module, loaded the first time the address asks for
// it (`?page=traces/ID`) or something opens it (`oa-page-open`, with the
// path). Its box covers the chat; its Back to chat (`oa-page-close`) hides
// the box, and moving within the page (`oa-page-path`) keeps the address.
var PAGES = {
  "admin-drive-access": ["./openagents_web_admin_drive_access.js", "./openagents_web_admin_drive_access_bg.wasm", "./openagents_web_admin_drive_access_webgl.js", "./openagents_web_admin_drive_access_webgl_bg.wasm"],
  "admin-attribution": ["./openagents_web_admin_attribution.js", "./openagents_web_admin_attribution_bg.wasm", "./openagents_web_admin_attribution_webgl.js", "./openagents_web_admin_attribution_webgl_bg.wasm"],
  "admin-analytics": ["./openagents_web_admin_analytics.js", "./openagents_web_admin_analytics_bg.wasm", "./openagents_web_admin_analytics_webgl.js", "./openagents_web_admin_analytics_webgl_bg.wasm"],
  keys: ["./openagents_web_keys.js", "./openagents_web_keys_bg.wasm", "./openagents_web_keys_webgl.js", "./openagents_web_keys_webgl_bg.wasm"],
  "claude": ["./openagents_web_claude.js", "./openagents_web_claude_bg.wasm", "./openagents_web_claude_webgl.js", "./openagents_web_claude_webgl_bg.wasm"],
  "data": ["./openagents_web_data.js", "./openagents_web_data_bg.wasm", "./openagents_web_data_webgl.js", "./openagents_web_data_webgl_bg.wasm"],
  "traces": ["./openagents_web_traces.js", "./openagents_web_traces_bg.wasm", "./openagents_web_traces_webgl.js", "./openagents_web_traces_webgl_bg.wasm"],
  "agents": ["./openagents_web_agents.js", "./openagents_web_agents_bg.wasm", "./openagents_web_agents_webgl.js", "./openagents_web_agents_webgl_bg.wasm"],
  "projects": ["./openagents_web_projects.js", "./openagents_web_projects_bg.wasm", "./openagents_web_projects_webgl.js", "./openagents_web_projects_webgl_bg.wasm"],
  "agent-lab": ["./openagents_web_agent_lab.js", "./openagents_web_agent_lab_bg.wasm", "./openagents_web_agent_lab_webgl.js", "./openagents_web_agent_lab_webgl_bg.wasm"],
  "cad": ["./openagents_web_cad.js", "./openagents_web_cad_bg.wasm", "./openagents_web_cad_webgl.js", "./openagents_web_cad_webgl_bg.wasm"],
  "labs": ["./openagents_web_lab.js", "./openagents_web_lab_bg.wasm", "./openagents_web_lab_webgl.js", "./openagents_web_lab_webgl_bg.wasm"],
  "voice": ["./openagents_web_voice.js", "./openagents_web_voice_bg.wasm", "./openagents_web_voice_webgl.js", "./openagents_web_voice_webgl_bg.wasm"],
  "connections": ["./openagents_web_connections.js", "./openagents_web_connections_bg.wasm", "./openagents_web_connections_webgl.js", "./openagents_web_connections_webgl_bg.wasm"],
  "vault": ["./openagents_web_vault.js", "./openagents_web_vault_bg.wasm", "./openagents_web_vault_webgl.js", "./openagents_web_vault_webgl_bg.wasm"],
  "terminal": ["./openagents_web_terminal.js", "./openagents_web_terminal_bg.wasm", "./openagents_web_terminal_webgl.js", "./openagents_web_terminal_webgl_bg.wasm"],
};
// Pages a reload shouldn't reopen: a call starts only from its button.
var UNADDRESSED = { voice: true };
var pageBoxes = {};
var pageModules = {};
var pageWanted = {};
function pageParam(path) {
  var url = new URL(window.location.href);
  if (path) {
    url.searchParams.set("page", path);
  } else {
    url.searchParams.delete("page");
  }
  window.history.replaceState(window.history.state, "", url.pathname + url.search + url.hash);
}
function openPage(path) {
  var name = (path || "").split("/")[0];
  var files = PAGES[name];
  if (!files) {
    return;
  }
  var rest = path.slice(name.length + 1);
  if ((name === "claude" || name === "data") && settingsBox) { settingsBox.hidden = true; }
  pageWanted[name] = rest;
  if (name === "keys") {
    var address = new URL(location.href);
    address.searchParams.delete("settings");
    history.replaceState(history.state, "", address.pathname + address.search);
  }
  if (!UNADDRESSED[name]) {
    pageParam(path);
  }
  closeVault();
  if (settingsBox) settingsBox.hidden = true;
  Object.keys(pageBoxes).forEach(function (other) {
    if (other !== name && pageBoxes[other]) {
      pageBoxes[other].hidden = true;
      if (pageModules[other] && pageModules[other].hide) Promise.resolve().then(function () {
        if (pageBoxes[other].hidden) pageModules[other].hide();
      });
    }
  });
  if (pageBoxes[name]) {
    pageBoxes[name].hidden = false;
    if (pageModules[name]) {
      pageModules[name].show(rest);
    }
    return;
  }
  var box = document.createElement("div");
  box.id = box.dataset.box = "gpui-page-" + name;
  box.setAttribute("aria-label", name);
  box.style.cssText = "position:fixed;inset:0;z-index:7;";
  if (name === "claude" || name === "data" || document.querySelector('meta[name="openagents-settings"]')) { box.style.zIndex = "100"; }
  document.body.appendChild(box);
  pageBoxes[name] = box;
  Promise.all([loadRenderer(new URL(files[0], import.meta.url), new URL(files[1], import.meta.url), new URL(files[2], import.meta.url), new URL(files[3], import.meta.url)), loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return Promise.resolve().then(function () {
        if (name === "keys" && document.documentElement.dataset.gpuiRenderer === "webgl") {
          var gl = document.createElement("canvas").getContext("webgl2");
          if (!gl) throw new Error("WebGL2 is unavailable");
          gl.getExtension("WEBGL_lose_context")?.loseContext();
        }
        pageModules[name] = module;
        mountIn(box, function () {
          module.mount(loaded[1], pageWanted[name] || rest);
        });
        if (box.hidden && module.hide) module.hide();
      });
    })
    .catch(function (error) {
      console.error("the page couldn't start", error);
      box.remove();
      delete pageBoxes[name];
      if (name === "keys") location.assign("/settings/" + (isKeysPage(rest) ? rest : "api-keys") + "?ui=html");
      else if (settingsSection()) htmlSettings();
      else pageParam("");
    });
}
window.addEventListener("oa-page-open", function (event) {
  openPage(event.detail || "");
});
window.addEventListener("oa-page-path", function (event) {
  pageParam(event.detail || "");
});
window.addEventListener("oa-page-close", function (event) {
  var name = event.detail || "";
  var box = pageBoxes[name];
  if (box) box.hidden = true;
  pageWanted[name] = null;
  if (pageModules[name] && pageModules[name].hide) Promise.resolve().then(function () {
    if (box && box.hidden) pageModules[name].hide();
  });
  closeVault();
  if (!UNADDRESSED[name]) {
    pageParam("");
  }
  if (document.querySelector('meta[name="openagents-settings"]')) {
    location.assign("/?ui=gpui");
  }
  if (name === "keys") window.dispatchEvent(new Event("oa-settings-close"));
});
function openAddressedPage() {
  var params = new URL(window.location.href).searchParams;
  var path = params.get("page");
  if (path && !UNADDRESSED[path.split("/")[0]]) {
    openPage(path);
  }
  var work = params.get("work");
  if (work) {
    openWork(work);
  }
  return Boolean(PAGES[(path || "").split("/")[0]] || work);
}

window.addEventListener("oa-settings-hide", function () {
  if (settingsBox) {
    settingsBox.hidden = true;
  }
});

// The vault keeps its own module and security policy. Removing this frame
// on close also discards its unlocked session; the GPUI module reads no keys.
var vaultBox = null;
function closeVault() {
  if (vaultBox) { vaultBox.remove(); vaultBox = null; }
}
window.addEventListener("oa-vault-open", function () {
  if (vaultBox) return;
  vaultBox = document.createElement("div");
  vaultBox.setAttribute("role", "dialog");
  vaultBox.setAttribute("aria-label", "Vault");
  vaultBox.style.cssText = "position:fixed;inset:16px;z-index:101;background:white;border-radius:14px;display:flex;flex-direction:column;overflow:hidden;";
  var close = document.createElement("button");
  close.textContent = "Close vault";
  close.addEventListener("click", closeVault);
  vaultBox.appendChild(close);
  var frame = document.createElement("iframe");
  frame.title = "Vault";
  frame.style.cssText = "width:100%;flex:1;border:0;";
  var project = /^\/projects\/[^/]+\/vault$/.test(location.pathname);
  frame.src = (project ? location.pathname : "/settings/vault") + "?ui=html&embed=1";
  vaultBox.appendChild(frame);
  document.body.appendChild(vaultBox);
  // Leave GPUI's click callback before changing its input's focus.
  setTimeout(function () { if (close.isConnected) close.focus(); }, 0);
});
window.addEventListener("keydown", function (event) {
  if (event.key === "Escape") closeVault();
});
window.addEventListener("oa-settings-hide", closeVault);

// Settings routes open the credentials page through the shared page loader.
var settingsShared = {};
function isKeysPage(page) {
  return ["api-keys", "sign-in-keys", "agent-accounts"].indexOf(page) !== -1;
}
function hideKeys() {
  var box = pageBoxes.keys;
  if (box) box.hidden = true;
  pageWanted.keys = null;
  if (pageModules.keys) Promise.resolve().then(function () {
    if (box && box.hidden) pageModules.keys.hide();
  });
}
window.addEventListener("oa-settings-page", function (event) {
  if (isKeysPage(event.detail)) Promise.resolve().then(function () {
    openSettings({detail: JSON.stringify({page: event.detail, shared: settingsShared})});
  });
});
window.addEventListener("oa-settings-hide", hideKeys);
window.addEventListener("pagehide", hideKeys);

start();
