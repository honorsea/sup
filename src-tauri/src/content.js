(function supInject() {
  "use strict";

  var UNREAD_RE = /^\((\d+)\)/;
  var SNAPSHOT_DEBOUNCE_MS = 1200;
  var snapshotTimer = null;

  function canInvoke() {
    return !!(window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);
  }

  function invoke(cmd, payload) {
    if (!canInvoke()) return Promise.resolve(null);
    return window.__TAURI__.core.invoke(cmd, payload || {});
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
    var chatNodes = document.querySelectorAll('[data-testid="cell-frame-container"]');
    var chats = [];
    for (var i = 0; i < chatNodes.length && i < 80; i++) {
      chats.push(chatNodes[i].innerText || "");
    }

    var msgNodes = document.querySelectorAll('[data-testid^="msg-"]');
    var messages = [];
    for (var j = 0; j < msgNodes.length && j < 300; j++) {
      messages.push(msgNodes[j].innerText || "");
    }

    return JSON.stringify({
      capturedAt: new Date().toISOString(),
      title: document.title,
      chats: chats,
      messages: messages,
    });
  }

  function queueSnapshotWrite() {
    if (!canInvoke()) return;
    if (snapshotTimer) clearTimeout(snapshotTimer);
    snapshotTimer = setTimeout(function () {
      invoke("save_snapshot", { snapshot: buildSnapshot() }).catch(function () {});
    }, SNAPSHOT_DEBOUNCE_MS);
  }

  function setupSnapshotObserver() {
    var root = document.body || document.documentElement;
    if (!root) return;

    queueSnapshotWrite();
    new MutationObserver(queueSnapshotWrite).observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
    });
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

  setupSnapshotObserver();
  setupExternalLinkRouting();
})();
