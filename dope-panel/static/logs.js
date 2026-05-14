/* -----------------------------------------------------------------------------
 * dope-panel/static/logs.js
 * Logs page rendering and activity table for dashboard.
 * -------------------------------------------------------------------------- */

/* --- Logs State ------------------------------------------------------------ */

let logsCache = [];
let refreshInterval = null;
let autoRefresh = true;

/* --- Logs API ------------------------------------------------------------- */

async function renderLogs(app) {
  stopRefresh();
  app.innerHTML = T.logsPage;
  const entries = await api('/api/logs?limit=500');
  logsCache = entries;
  renderLogTable();
  if (autoRefresh) startRefresh(refreshLogs);
}

async function refreshLogs() {
  if (logsCache.length > 0) {
    const lastTs = logsCache[logsCache.length - 1].ts;
    const newEntries = await api(`/api/logs?since=${lastTs + 1}`);
    if (newEntries.length > 0) {
      logsCache = logsCache.concat(newEntries);
      renderLogTable();
    }
  }
}

function toggleRefresh(on) {
  autoRefresh = on;
  if (on) startRefresh(refreshLogs);
  else stopRefresh();
}

function startRefresh(fn) {
  stopRefresh();
  refreshInterval = setInterval(fn, 3000);
}

function stopRefresh() {
  if (refreshInterval) { clearInterval(refreshInterval); refreshInterval = null; }
}

/* --- Log Table Rendering -------------------------------------------------- */

function renderLogTable() {
  const host = (document.getElementById('filter-host')?.value || '').toLowerCase();
  const type = document.getElementById('filter-type')?.value || '';
  const search = (document.getElementById('filter-search')?.value || '').toLowerCase();
  const container = document.getElementById('log-table');
  if (!container) return;

  let entries = logsCache;
  if (host) entries = entries.filter(e => (e.host || '').toLowerCase().includes(host));
  if (type) entries = entries.filter(e => e.type === type);
  if (search) entries = entries.filter(e => JSON.stringify(e).toLowerCase().includes(search));

  if (entries.length === 0) { container.innerHTML = T.empty('No matching entries.'); return; }

  const rows = entries.slice().reverse().map(e => {
    const ts = new Date(e.ts).toLocaleTimeString();
    let typeBadge, details, expanded;
    if (e.type === 'request') {
      typeBadge = '<span class="badge badge-request">req</span>';
      details = `<strong>${e.method}</strong> ${e.host} <span style="color:#888">${e.uri.slice(0, 80)}</span>`;
      expanded = `<pre>${JSON.stringify({method:e.method, uri:e.uri, host:e.host, user_agent:e.user_agent, accept:e.accept}, null, 2)}</pre>`;
    } else if (e.type === 'response') {
      const cls = `badge-${Math.floor(e.status / 100)}xx`;
      typeBadge = `<span class="badge ${cls}">${e.status}</span>`;
      details = `<span style="color:#888">${e.content_type}</span>`;
      expanded = `<pre>${JSON.stringify({status:e.status, content_type:e.content_type, body_preview:e.body_preview}, null, 2)}</pre>`;
    } else {
      typeBadge = '<span class="badge badge-error">err</span>';
      details = `<span style="color:#e94560">${e.error.slice(0, 80)}</span>`;
      expanded = `<pre>${JSON.stringify({client_addr:e.client_addr, error:e.error}, null, 2)}</pre>`;
    }
    return T.logRow(ts, typeBadge, details, expanded);
  }).join('');
  container.innerHTML = T.logTable(rows);
}

function toggleRow(tr) {
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
  const rows = entries.slice().reverse().map(e => {
    const ts = new Date(e.ts).toLocaleTimeString();
    let typeBadge, details;
    if (e.type === 'request') {
      typeBadge = '<span class="badge badge-request">req</span>';
      details = `${e.method} ${e.host}`;
    } else if (e.type === 'response') {
      const cls = `badge-${Math.floor(e.status / 100)}xx`;
      typeBadge = `<span class="badge ${cls}">${e.status}</span>`;
      details = `${e.content_type}`;
    } else {
      typeBadge = '<span class="badge badge-error">err</span>';
      details = e.error.slice(0, 60);
    }
    return T.activityRow(ts, typeBadge, details);
  }).join('');
  container.innerHTML = T.activityTable(rows);
}