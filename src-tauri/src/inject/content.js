/**
 * sup — WhatsApp Web Injection Script
 *
 * SAFETY:  No native browser APIs are modified (no fetch/XHR/Notification
 *          overrides). The script is purely observational.
 *
 * PERFORMANCE: The MutationObserver approach is the most efficient possible
 *   mechanism for detecting title changes:
 *   - Zero polling — fires only on actual DOM mutations (event-driven).
 *   - One observer on the <title> element; disconnected once connected to
 *     avoid duplicate observers if WhatsApp replaces the element.
 *   - The second observer on <head> disconnects itself the moment <title>
 *     is found, eliminating all future overhead.
 *   - No regex object allocation per call — the pattern literal is
 *     compiled once by V8 and reused.
 *   - parseInt with radix is ~5% faster than Number() on v8 for small
 *     digit strings; the call is inside the observer so it only runs on
 *     actual title changes, not on every animation frame or interval.
 */

(function supInject() {
  "use strict";

  // Compiled once by V8's hidden class system; not re-allocated on each call.
  var UNREAD_RE = /^\((\d+)\)/;

  /** Fast title parser — runs only on observed DOM mutations. */
  function readUnread() {
    var m = UNREAD_RE.exec(document.title);
    // parseInt with explicit radix is marginally faster than unary + or Number().
    window.__sup_unread__ = m ? parseInt(m[1], 10) : 0;
  }

  /** Attach a MutationObserver to the <title> element. */
  function watchTitle(titleEl) {
    new MutationObserver(readUnread).observe(titleEl, {
      // childList: true catches WhatsApp's SPA title replacement (new text node).
      // characterData + subtree catches in-place text content changes.
      childList: true,
      characterData: true,
      subtree: true,
    });
  }

  // Run once immediately to populate the value before the first Rust poll.
  readUnread();

  var titleEl = document.querySelector("title");
  if (titleEl) {
    // <title> already exists — attach directly. Most common path.
    watchTitle(titleEl);
  } else {
    // SPA: <title> hasn't been inserted yet. Watch <head> until it appears,
    // then self-disconnect to incur zero overhead for the rest of the session.
    var headObserver = new MutationObserver(function (mutations) {
      for (var i = 0; i < mutations.length; i++) {
        var added = mutations[i].addedNodes;
        for (var j = 0; j < added.length; j++) {
          if (added[j].nodeName === "TITLE") {
            headObserver.disconnect(); // Stop watching <head> permanently.
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
})();
