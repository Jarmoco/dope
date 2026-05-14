/* -----------------------------------------------------------------------------
 * dope-panel/static/app.js
 * Main entry point: shared state, API helpers, router, and dashboard.
 * -------------------------------------------------------------------------- */

/* --- Shared State ---------------------------------------------------------- */

window.dope = {};
window.dope.logsCache = [];

/* --- API Helpers ---------------------------------------------------------- */

async function api(url, opts) {
  const res = await fetch(url, opts);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

function toast(msg) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.classList.add('show');
  setTimeout(() => el.classList.remove('show'), 2500);
}

/* --- Router --------------------------------------------------------------- */

function navigate() {
  const hash = location.hash.slice(1) || '/';
  document.querySelectorAll('.nav-link').forEach(a => {
    a.classList.toggle('active', a.getAttribute('href') === location.hash);
  });
  const app = document.getElementById('app');
  switch (hash.split('?')[0]) {
    case '/': renderDashboard(app); break;
    case '/logs': renderLogs(app); break;
    case '/config': renderConfig(app); break;
    default: app.innerHTML = '<h2>Not found</h2>';
  }
}

window.addEventListener('hashchange', navigate);
window.addEventListener('DOMContentLoaded', navigate);

/* --- Dashboard ------------------------------------------------------------ */

async function renderDashboard(app) {
  stopRefresh();
  const entries = await api('/api/logs?limit=200');
  window.dope.logsCache = entries;

  const total = entries.length;
  const hosts = new Set(entries.filter(e => e.type === 'request').map(e => e.host)).size;
  const errors = entries.filter(e => e.type === 'error' || (e.type === 'response' && e.status >= 500)).length;

  app.innerHTML = T.dashboard(T.stats(total, hosts, errors));
  renderActivityTable(document.getElementById('activity'), entries.slice(-20));
  startRefresh(renderDashboardActivity);
}