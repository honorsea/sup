(function supInject() {
  "use strict";

  var UNREAD_RE = /^\((\d+)\)/;
  var SNAPSHOT_DEBOUNCE_MS = 1200;
  var snapshotTimer = null;

  function resolveInvoke() {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke;
    }
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      return window.__TAURI_INTERNALS__.invoke;
    }
    return null;
  }

  function invoke(cmd, payload) {
    var inv = resolveInvoke();
    if (!inv) return Promise.resolve(null);
    return inv(cmd, payload || {});
  }

  function readUnread() {
    var m = UNREAD_RE.exec(document.title);
    window.__sup_unread__ = m ? parseInt(m[1], 10) : 0;
  }

  function watchTitle(titleEl) {
    new MutationObserver(readUnread).observe(titleEl, {
      childList: true,
      characterData: true,
      subtree: true,
    });
  }

  function buildSnapshot() {
    var chatNodes = document.querySelectorAll(
      '[data-testid="cell-frame-container"], [role="listitem"], #pane-side [tabindex]'
    );
    var chats = [];
    for (var i = 0; i < chatNodes.length && i < 80; i++) {
      var chatText = (chatNodes[i].innerText || "").trim();
      if (chatText) chats.push(chatText);
    }

    var msgNodes = document.querySelectorAll(
      '[data-testid^="msg-"], [data-pre-plain-text], #main [role="row"], #main [tabindex]'
    );
    var messages = [];
    for (var j = 0; j < msgNodes.length && j < 300; j++) {
      var msgText = (msgNodes[j].innerText || "").trim();
      if (msgText) messages.push(msgText);
    }

    return JSON.stringify({
      capturedAt: new Date().toISOString(),
      title: document.title,
      chats: chats,
      messages: messages,
    });
  }

  function queueSnapshotWrite() {
    if (!resolveInvoke()) return;
    if (snapshotTimer) clearTimeout(snapshotTimer);
    snapshotTimer = setTimeout(function () {
      invoke("save_snapshot", { snapshot: buildSnapshot() }).catch(function () {});
    }, SNAPSHOT_DEBOUNCE_MS);
  }

  function setupSnapshotObserver() {
    var root = document.body || document.documentElement;
    if (!root) return false;

    queueSnapshotWrite();
    new MutationObserver(queueSnapshotWrite).observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    return true;
  }

  function setupExternalLinkRouting() {
    document.addEventListener(
      "click",
      function (event) {
        var el = event.target;
        while (el && el.tagName !== "A") el = el.parentElement;
        if (!el) return;

        var href = el.getAttribute("href") || "";
        if (!href) return;

        var isHttp = /^https?:\/\//i.test(href);
        var isMail = /^mailto:/i.test(href);
        var hasDownload = el.hasAttribute("download");

        if (isHttp || isMail || hasDownload) {
          event.preventDefault();
          event.stopPropagation();
          invoke("open_external", { url: href }).catch(function () {});
        }
      },
      true
    );
  }

  readUnread();

  var titleEl = document.querySelector("title");
  if (titleEl) {
    watchTitle(titleEl);
  } else {
    var headObserver = new MutationObserver(function (mutations) {
      for (var i = 0; i < mutations.length; i++) {
        var added = mutations[i].addedNodes;
        for (var j = 0; j < added.length; j++) {
          if (added[j].nodeName === "TITLE") {
            headObserver.disconnect();
            watchTitle(added[j]);
            readUnread();
            return;
          }
        }
      }
    });
    headObserver.observe(document.head || document.documentElement, {
      childList: true,
    });
  }

  if (!setupSnapshotObserver()) {
    document.addEventListener(
      "DOMContentLoaded",
      function () {
        setupSnapshotObserver();
      },
      { once: true }
    );
    setTimeout(setupSnapshotObserver, 1200);
  }
  setupExternalLinkRouting();
})();
