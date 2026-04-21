// ==UserScript==
// @name Example userscript for google.com
// @namespace https://yourname.github.io/userscripts
// @version 0.1.0
// @description Injects a banner
// @author You
// @match https://www.google.com/*
// @grant GM_addStyle
// @run_at document-end
// ==/UserScript==

(function () {
  "use strict";

  GM_addStyle(`
    .injection-banner {
        width: 20vw;
        height: 10vh;
        background: #ab5315ff;
        color: white;
        position: absolute;
        top: 0;
        right: 0;
        display: flex;
        justify-content: center;
        align-items: center;
    `);

  console.log("Google Injection started");
  let banner = document.createElement("div");
  banner.className = "injection-banner"
  banner.innerHTML = "Injection completed"
  document.body.appendChild(banner);
})();
