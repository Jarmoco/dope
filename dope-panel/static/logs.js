/* -----------------------------------------------------------------------------
 * dope-panel/static/logs.js
 * Logs page rendering and activity table for dashboard.
 * -------------------------------------------------------------------------- */

/* --- Logs State ------------------------------------------------------------ */

window.dope.refreshInterval = null;
window.dope.autoRefresh = true;

/* --- Logs API ------------------------------------------------------------- */

async function renderLogs(app) {
  stopRefresh();
  app.innerHTML = T.logsPage;
  const entries = await api('/api/logs?limit=500');
  window.dope.logsCache = entries;
  renderLogTable();
  if (window.dope.autoRefresh) startRefresh(refreshLogs);
}

async function refreshLogs() {
  if (window.dope.logsCache.length > 0) {
    const lastTs = window.dope.logsCache[window.dope.logsCache.length - 1].ts;
    const newEntries = await api(`/api/logs?since=${lastTs + 1}`);
    if (newEntries.length > 0) {
      window.dope.logsCache = window.dope.logsCache.concat(newEntries);
      renderLogTable();
    }
  }
}

function toggleRefresh(on) {
  window.dope.autoRefresh = on;
  if (on) startRefresh(refreshLogs);
  else stopRefresh();
}

function startRefresh(fn) {
  stopRefresh();
  window.dope.refreshInterval = setInterval(fn, 3000);
}

function stopRefresh() {
  if (window.dope.refreshInterval) { clearInterval(window.dope.refreshInterval); window.dope.refreshInterval = null; }
}

/* --- Grouping ------------------------------------------------------------- */

function groupByReqId(entries) {
  const groups = {};
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    const id = e.req_id || `orphan-${i}`;
    if (!groups[id]) {
      groups[id] = { request: null, response: null, error: null, ts: e.ts };
    }
    if (e.type === 'request') groups[id].request = e;
    else if (e.type === 'response') groups[id].response = e;
    else if (e.type === 'error') groups[id].error = e;
    if (e.ts < groups[id].ts) groups[id].ts = e.ts;
  }
  return Object.values(groups).sort((a, b) => b.ts - a.ts);
}

/* --- Log Table Rendering -------------------------------------------------- */

function renderLogTable() {
  const search = (document.getElementById('filter-search')?.value || '').toLowerCase();
  const container = document.getElementById('log-table');
  if (!container) return;

  let entries = window.dope.logsCache;
  if (search) {
    entries = entries.filter(e => JSON.stringify(e).toLowerCase().includes(search));
  }

  if (entries.length === 0) { container.innerHTML = T.empty('No matching entries.'); return; }

  const groups = groupByReqId(entries);
  const filtered = search ? groups.filter(g => {
    const q = search;
    const req = g.request;
    const resp = g.response;
    const err = g.error;
    const host = req ? req.host.toLowerCase() : (err ? err.client_addr.toLowerCase() : '');
    const method = req ? req.method.toLowerCase() : '';
    const status = resp ? String(resp.status) : '';
    const ct = resp ? (resp.content_type || '').toLowerCase() : '';
    return host.includes(q) || method.includes(q) || status.includes(q) || ct.includes(q);
  }) : groups;
  const rows = filtered.map(g => {
    const ts = new Date(g.ts).toLocaleTimeString();
    const req = g.request;
    const resp = g.response;
    const err = g.error;

    let method = req ? req.method : (err ? 'ERR' : '???');
    let hostVal = req ? req.host : (err ? err.client_addr : '-');
    let status = resp ? resp.status : (err ? 'ERR' : '-');
    let contentType = resp ? resp.content_type : '';
    let duration = resp && req ? Math.max(1, resp.ts - req.ts) : '-';

    let respDetails = '';
    if (contentType) respDetails += `<span class="meta">${contentType}</span> `;
    if (duration !== '-') respDetails += `<span class="duration">${duration}ms</span>`;

    let detailsExpanded = '';
    if (req) {
      detailsExpanded += `<div class="detail-section"><h4>Request</h4><pre>${JSON.stringify({method:req.method, uri:req.uri, host:req.host, user_agent:req.user_agent, accept:req.accept}, null, 2)}</pre></div>`;
    }
    if (resp) {
      detailsExpanded += `<div class="detail-section"><h4>Response</h4><pre>${JSON.stringify({status:resp.status, content_type:resp.content_type, body_preview:resp.body_preview}, null, 2)}</pre></div>`;
    }
    if (err) {
      detailsExpanded += `<div class="detail-section"><h4>Error</h4><pre>${JSON.stringify({client_addr:err.client_addr, error:err.error}, null, 2)}</pre></div>`;
    }

    return T.combinedRow(ts, method, hostVal, status, respDetails, detailsExpanded);
  }).join('');
  container.innerHTML = T.logTable(rows);
}

function toggleRow(tr) {
  const next = tr.nextElementSibling;
  if (next && next.style.display === 'none') next.style.display = 'table-row';
  else if (next) next.style.display = 'none';
}

function toggleCombined(tr) {
  const next = tr.nextElementSibling;
  if (next && next.style.display === 'none') next.style.display = 'table-row';
  else if (next) next.style.display = 'none';
}

/* --- Dashboard Activity ---------------------------------------------------- */

async function renderDashboardActivity() {
  const entries = await api('/api/logs?limit=20');
  const container = document.getElementById('activity');
  if (container) renderActivityTable(container, entries);
}

function renderActivityTable(container, entries) {
  if (entries.length === 0) { container.innerHTML = T.empty('No entries yet.'); return; }

  const groups = groupByReqId(entries).slice(0, 20);
  const rows = groups.map(g => {
    const ts = new Date(g.ts).toLocaleTimeString();
    const req = g.request;
    const resp = g.response;
    const err = g.error;

    let method = req ? req.method : (err ? 'ERR' : '???');
    let hostVal = req ? req.host : (err ? err.client_addr : '-');
    let status = resp ? resp.status : (err ? 'ERR' : '-');
    let contentType = resp ? resp.content_type : '';
    let duration = resp && req ? Math.max(1, resp.ts - req.ts) : '-';

    let respDetails = '';
    if (contentType) respDetails += `<span class="meta">${contentType}</span> `;
    if (duration !== '-') respDetails += `<span class="duration">${duration}ms</span>`;

    return T.combinedRow(ts, method, hostVal, status, respDetails, '');
  }).join('');
  container.innerHTML = T.activityTable(rows);
}