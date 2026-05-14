/* -----------------------------------------------------------------------------
 * dope-panel/static/config.js
 * Configuration page rendering and form mutations.
 * -------------------------------------------------------------------------- */

/* --- Config Rendering ------------------------------------------------------ */

async function renderConfig(app) {
  stopRefresh();
  app.innerHTML = T.configPage;
  const config = await api('/api/config');
  renderConfigForm(app, config);
}

function renderConfigForm(app, config) {
  const container = document.getElementById('config-form');
  if (!container) return;

  container.innerHTML = T.configForm(config.server.port, config.server.pause);

  window._config = config;
  renderScriptRules(config.scripts || []);
  renderResponseRules(config.modify_response || []);
  renderRequestRules(config.modify_request || []);
}

function renderScriptRules(rules) {
  const container = document.getElementById('scripts-list');
  if (rules.length === 0) { container.innerHTML = T.empty('No script rules configured.'); return; }
  container.innerHTML = rules.map((r, i) => T.scriptRuleCard(i, r)).join('');
}

function renderResponseRules(rules) {
  const container = document.getElementById('response-list');
  if (rules.length === 0) { container.innerHTML = T.empty('No response modifiers configured.'); return; }
  container.innerHTML = rules.map((r, i) => T.responseRuleCard(i, r)).join('');
  rules.forEach((r, i) => {
    renderResponseHeaders(i, r.add_headers || {});
    renderResponseRemoveHeaders(i, r.remove_headers || []);
  });
}

function renderResponseRemoveHeaders(idx, headers) {
  const container = document.getElementById(`response-remove-headers-${idx}`);
  if (!container) return;
  container.innerHTML = headers.map((h, hi) => T.removeHeaderPair(idx, hi, h)).join('');
}

function renderResponseHeaders(idx, headers) {
  const container = document.getElementById(`response-headers-${idx}`);
  if (!container) return;
  const entries = Object.entries(headers);
  container.innerHTML = entries.map(([k, v], hi) => T.headerPair(idx, hi, k, v, 'Response')).join('');
}

function renderRequestRules(rules) {
  const container = document.getElementById('request-list');
  if (rules.length === 0) { container.innerHTML = T.empty('No request modifiers configured.'); return; }
  container.innerHTML = rules.map((r, i) => T.requestRuleCard(i, r)).join('');
  rules.forEach((r, i) => {
    renderRequestHeaders(i, r.add_headers || {});
    renderRequestRemoveHeaders(i, r.remove_headers || []);
  });
}

function renderRequestRemoveHeaders(idx, headers) {
  const container = document.getElementById(`request-remove-headers-${idx}`);
  if (!container) return;
  container.innerHTML = headers.map((h, hi) => T.removeHeaderPair(idx, hi, h)).join('');
}

function renderRequestHeaders(idx, headers) {
  const container = document.getElementById(`request-headers-${idx}`);
  if (!container) return;
  const entries = Object.entries(headers);
  container.innerHTML = entries.map(([k, v], hi) => T.headerPair(idx, hi, k, v, 'Request')).join('');
}

/* --- Config mutations ----------------------------------------------------- */

function getConfig() { return window._config || { server: { port: 8080, pause: false }, scripts: [], modify_response: [], modify_request: [] }; }

function addScriptRule() {
  const cfg = getConfig();
  if (!cfg.scripts) cfg.scripts = [];
  cfg.scripts.push({ domain: '', scripts: [] });
  renderScriptRules(cfg.scripts);
}
function removeScriptRule(i) {
  const cfg = getConfig();
  cfg.scripts.splice(i, 1);
  renderScriptRules(cfg.scripts);
}
function updateScriptRule(i, field, val) {
  const cfg = getConfig();
  cfg.scripts[i][field] = val;
}

function addResponseRule() {
  const cfg = getConfig();
  if (!cfg.modify_response) cfg.modify_response = [];
  cfg.modify_response.push({ domain: '', csp: null, remove_headers: null, add_headers: null, inject_at: null });
  renderResponseRules(cfg.modify_response);
}
function removeResponseRule(i) {
  const cfg = getConfig();
  cfg.modify_response.splice(i, 1);
  renderResponseRules(cfg.modify_response);
}
function updateResponseRule(i, field, val) {
  const cfg = getConfig();
  cfg.modify_response[i][field] = val;
}
function addResponseHeader(i) {
  const cfg = getConfig();
  const rule = cfg.modify_response[i];
  if (!rule.add_headers) rule.add_headers = {};
  const base = Object.keys(rule.add_headers).length;
  rule.add_headers[`key${base}`] = 'value';
  renderResponseHeaders(i, rule.add_headers);
}
function removeResponseHeader(i, hi) {
  const cfg = getConfig();
  const headers = cfg.modify_response[i].add_headers || {};
  const keys = Object.keys(headers);
  const k = keys[hi];
  if (k !== undefined) { delete headers[k]; }
  renderResponseHeaders(i, headers);
}
function updateResponseHeader(i, hi, which, val) {
  const cfg = getConfig();
  const headers = cfg.modify_response[i].add_headers || {};
  const keys = Object.keys(headers);
  const k = keys[hi];
  if (k === undefined) return;
  if (which === 'key') {
    headers[val] = headers[k];
    if (val !== k) delete headers[k];
  } else {
    headers[k] = val;
  }
}
function addResponseRemoveHeader(i) {
  const cfg = getConfig();
  const rule = cfg.modify_response[i];
  if (!rule.remove_headers) rule.remove_headers = [];
  rule.remove_headers.push('');
  renderResponseRemoveHeaders(i, rule.remove_headers);
}
function removeResponseRemoveHeader(i, hi) {
  const cfg = getConfig();
  const headers = cfg.modify_response[i].remove_headers || [];
  headers.splice(hi, 1);
  renderResponseRemoveHeaders(i, headers);
}
function updateResponseRemoveHeader(i, hi, val) {
  const cfg = getConfig();
  const headers = cfg.modify_response[i].remove_headers || [];
  if (headers[hi] !== undefined) {
    headers[hi] = val;
  }
}

function addRequestRule() {
  const cfg = getConfig();
  if (!cfg.modify_request) cfg.modify_request = [];
  cfg.modify_request.push({ domain: '', remove_headers: null, add_headers: null });
  renderRequestRules(cfg.modify_request);
}
function removeRequestRule(i) {
  const cfg = getConfig();
  cfg.modify_request.splice(i, 1);
  renderRequestRules(cfg.modify_request);
}
function updateRequestRule(i, field, val) {
  const cfg = getConfig();
  cfg.modify_request[i][field] = val;
}
function addRequestRemoveHeader(i) {
  const cfg = getConfig();
  const rule = cfg.modify_request[i];
  if (!rule.remove_headers) rule.remove_headers = [];
  rule.remove_headers.push('');
  renderRequestRemoveHeaders(i, rule.remove_headers);
}
function removeRequestRemoveHeader(i, hi) {
  const cfg = getConfig();
  const headers = cfg.modify_request[i].remove_headers || [];
  headers.splice(hi, 1);
  renderRequestRemoveHeaders(i, headers);
}
function updateRequestRemoveHeader(i, hi, val) {
  const cfg = getConfig();
  const headers = cfg.modify_request[i].remove_headers || [];
  if (headers[hi] !== undefined) {
    headers[hi] = val;
  }
}
function addRequestHeader(i) {
  const cfg = getConfig();
  const rule = cfg.modify_request[i];
  if (!rule.add_headers) rule.add_headers = {};
  const base = Object.keys(rule.add_headers).length;
  rule.add_headers[`key${base}`] = 'value';
  renderRequestHeaders(i, rule.add_headers);
}
function removeRequestHeader(i, hi) {
  const cfg = getConfig();
  const headers = cfg.modify_request[i].add_headers || {};
  const keys = Object.keys(headers);
  const k = keys[hi];
  if (k !== undefined) { delete headers[k]; }
  renderRequestHeaders(i, headers);
}
function updateRequestHeader(i, hi, which, val) {
  const cfg = getConfig();
  const headers = cfg.modify_request[i].add_headers || {};
  const keys = Object.keys(headers);
  const k = keys[hi];
  if (k === undefined) return;
  if (which === 'key') {
    headers[val] = headers[k];
    if (val !== k) delete headers[k];
  } else {
    headers[k] = val;
  }
}

async function saveConfig() {
  const cfg = getConfig();
  cfg.server.port = parseInt(document.getElementById('cfg-port').value) || 8080;
  cfg.server.pause = document.getElementById('cfg-pause').checked || null;

  try {
    const res = await fetch('/api/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(cfg),
    });
    if (res.ok) { toast('Configuration saved'); }
    else { toast('Failed to save: ' + (await res.text())); }
  } catch (e) {
    toast('Failed to save: ' + e.message);
  }
}