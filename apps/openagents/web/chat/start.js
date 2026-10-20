// Starts the GPUI web chat (crates/openagents-web/src/pages/gpui_chat.rs).
// It needs WebGPU; without it, the page goes to the regular
// chat. Otherwise the module and the fonts download together,
// and the canvas replaces the page's outline once the app has started.
import init, { launch, gpuiRecordedFrame } from "./openagents_web_chat.js";

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

// The module draws with WebGPU only: GPUI's renderer needs storage
// buffers, which WebGL2 doesn't have (#12237).
// A browser can name WebGPU and still have no adapter for it (a headless
// or blocklisted GPU), so the check asks for one.
function supported() {
  if (!navigator.gpu) {
    return Promise.resolve(false);
  }
  return navigator.gpu.requestAdapter().then(function (adapter) {
    return !!adapter;
  }, function () {
    return false;
  });
}

// The fonts the chat fetched, which the file panel draws with too.
var loadedFonts = null;

function say(text) {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.textContent = text;
  }
}

// Without WebGPU the HTML chat takes over: the page goes to
// its address (the note's link), and shows the note if it can't.
function unsupported() {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.remove();
  }
  var note = document.getElementById("gpui-unsupported");
  if (note) {
    note.hidden = false;
    var link = note.querySelector("a");
    if (link) {
      location.replace(link.href);
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
    var shell = document.getElementById("gpui-shell");
    if (shell) {
      shell.style.display = "none";
    }
    // Measure the actual first frame, including renderer initialization.
    window.__gpuiReadyAt = performance.now();
    window.__gpuiReadyMs = window.__gpuiReadyAt - started;
    document.documentElement.dataset.gpui = "ready";
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
  openAddressedPage();
  Promise.all([init({ module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_chat_bg.wasm", import.meta.url))) }), fonts])
    .then(function (loaded) {
      launch(loaded[1]);
    })
    .catch(function (error) {
      say("The chat couldn't start. Reload the page to try again.");
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
var host = "";
var files = null;
var filesBox = null;
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
  var glue = new URL("./openagents_web_files.js", import.meta.url);
  Promise.all([import(glue.href), loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_files_bg.wasm", import.meta.url))),
      }).then(function () {
        files = module;
        var parts = hostParts();
        mountIn(filesBox, function () {
          module.mount(loaded[1], parts[0], parts[1]);
        });
      });
    })
    .catch(function (error) {
      console.error("the file panel couldn't start", error);
      filesBox.remove();
      filesBox = null;
    });
}
window.addEventListener("oa-files-toggle", openFiles);
window.addEventListener("oa-files-close", function () {
  showFiles(false);
});
window.addEventListener("oa-host", function (event) {
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
window.addEventListener("oa-room-open", function (event) {
  roomChat = event.detail || "";
  loadRoom()
    .then(function (module) {
      if (roomChat) {
        module.open(roomChat);
      }
    })
    .catch(function (error) {
      console.error("the chat's transcript couldn't load", error);
      roomLoading = null;
      window.dispatchEvent(new CustomEvent("oa-room", {
        detail: JSON.stringify({ chat: roomChat, entries: [], caught_up: false, unavailable: true }),
      }));
    });
});
window.addEventListener("oa-room-close", function () {
  roomChat = "";
  if (room) {
    room.close();
  }
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
function openWork() {
  if (workBox) {
    workBox.hidden = !workBox.hidden;
    return;
  }
  workBox = document.createElement("div");
  workBox.id = workBox.dataset.box = "gpui-work";
  workBox.setAttribute("aria-label", "Work");
  workBox.style.cssText = "position:fixed;top:8px;right:8px;bottom:8px;width:min(560px,46vw);z-index:6;border-radius:14px;overflow:hidden;";
  document.body.appendChild(workBox);
  var glue = new URL("./openagents_web_work.js", import.meta.url);
  Promise.all([import(glue.href), loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_work_bg.wasm", import.meta.url))),
      }).then(function () {
        mountIn(workBox, function () {
          module.mount(loaded[1]);
        });
      });
    })
    .catch(function (error) {
      console.error("the work panel couldn't start", error);
      workBox.remove();
      workBox = null;
    });
}
window.addEventListener("oa-work-toggle", openWork);
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
function openSettings(event) {
  var asked = {};
  try {
    asked = JSON.parse(event.detail || "{}");
  } catch (error) {
    asked = {};
  }
  var page = asked.page || "general";
  var shared = JSON.stringify(asked.shared || {});
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
  document.body.appendChild(settingsBox);
  var glue = new URL("./openagents_web_settings.js", import.meta.url);
  Promise.all([import(glue.href), loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_settings_bg.wasm", import.meta.url))),
      }).then(function () {
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
    });
}
window.addEventListener("oa-settings-open", openSettings);

// The account's pages (#12329, #12330, #12331): Traces, Agents, and
// Projects, each its own module, loaded the first time the address asks for
// it (`?page=traces/ID`) or something opens it (`oa-page-open`, with the
// path). Its box covers the chat; its Back to chat (`oa-page-close`) hides
// the box, and moving within the page (`oa-page-path`) keeps the address.
var PAGES = {
  traces: ["./openagents_web_traces.js", "./openagents_web_traces_bg.wasm"],
  agents: ["./openagents_web_agents.js", "./openagents_web_agents_bg.wasm"],
  projects: ["./openagents_web_projects.js", "./openagents_web_projects_bg.wasm"],
};
var pageBoxes = {};
var pageModules = {};
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
  pageParam(path);
  Object.keys(pageBoxes).forEach(function (other) {
    if (other !== name && pageBoxes[other]) {
      pageBoxes[other].hidden = true;
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
  document.body.appendChild(box);
  pageBoxes[name] = box;
  Promise.all([import(new URL(files[0], import.meta.url).href), loadedFonts])
    .then(function (loaded) {
      var module = loaded[0];
      return module.default({
        module_or_path: WebAssembly.compileStreaming(fetch(new URL(files[1], import.meta.url))),
      }).then(function () {
        pageModules[name] = module;
        mountIn(box, function () {
          module.mount(loaded[1], rest);
        });
      });
    })
    .catch(function (error) {
      console.error("the page couldn't start", error);
      box.remove();
      delete pageBoxes[name];
      pageParam("");
    });
}
window.addEventListener("oa-page-open", function (event) {
  openPage(event.detail || "");
});
window.addEventListener("oa-page-path", function (event) {
  pageParam(event.detail || "");
});
window.addEventListener("oa-page-close", function (event) {
  var box = pageBoxes[event.detail || ""];
  if (box) {
    box.hidden = true;
  }
  pageParam("");
});
function openAddressedPage() {
  var path = new URL(window.location.href).searchParams.get("page");
  if (path) {
    openPage(path);
  }
}

window.addEventListener("oa-settings-hide", function () {
  if (settingsBox) {
    settingsBox.hidden = true;
  }
});

supported().then(function (ok) {
  if (ok) {
    start();
  } else {
    unsupported();
  }
});
