/* -----------------------------------------------------------------------------
 * dope-panel/static/templates.js
 * HTML template literals for the admin panel UI.
 * -------------------------------------------------------------------------- */

const T = {};

T.dashboard = (stats) => `
  <h2>Dashboard</h2>
  <div class="stats" id="stats">${stats}</div>
  <h2>Recent Activity</h2>
  <div id="activity"></div>
`;

T.stats = (total, hosts, errors) => `
  <div class="stat-card"><div class="value">${total}</div><div class="label">Total Entries</div></div>
  <div class="stat-card"><div class="value">${hosts}</div><div class="label">Unique Hosts</div></div>
  <div class="stat-card"><div class="value">${errors}</div><div class="label">Errors</div></div>
`;

T.empty = (msg) => `<div class="empty">${msg}</div>`;

T.activityTable = (rows) => `
  <table>
    <thead><tr><th>Time</th><th>Type</th><th>Details</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>
`;

T.activityRow = (ts, typeBadge, details) =>
  `<tr><td>${ts}</td><td>${typeBadge}</td><td>${details}</td></tr>`;

T.combinedRow = (ts, method, host, status, contentType, duration, detailsExpanded) => `
  <tr class="combined-row" onclick="toggleCombined(this)" style="cursor:pointer">
    <td>${ts}</td>
    <td><span class="badge badge-request">${method}</span></td>
    <td>${host} <span class="arrow">==></span> <span class="badge badge-${Math.floor(status / 100)}xx">${status}</span> <span class="meta">${contentType || ''}</span> <span class="duration">${duration}ms</span></td>
  </tr>
  <tr class="combined-details" style="display:none"><td colspan="3">${detailsExpanded}</td></tr>
`;

T.logsPage = `
  <h2>Logs</h2>
  <div class="filters">
    <input type="text" id="filter-host" placeholder="Host..." oninput="renderLogTable()">
    <select id="filter-type" onchange="renderLogTable()">
      <option value="">All types</option>
      <option value="request">Request</option>
      <option value="response">Response</option>
      <option value="error">Error</option>
    </select>
    <input type="text" id="filter-search" placeholder="Search..." oninput="renderLogTable()" style="flex:1">
    <label class="refresh-toggle active" id="refresh-label">
      <input type="checkbox" checked onchange="toggleRefresh(this.checked)"> Auto-refresh
    </label>
  </div>
  <div id="log-table"></div>
`;

T.logTable = (rows) => `
  <table>
    <thead><tr><th>Time</th><th>Type</th><th>Details</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>
`;

T.logRow = (ts, typeBadge, details, expanded) =>
  `<tr onclick="toggleRow(this)" style="cursor:pointer"><td>${ts}</td><td>${typeBadge}</td><td>${details}</td></tr>
   <tr style="display:none"><td colspan="3">${expanded}</td></tr>`;

T.configPage = `<h2>Configuration</h2><div id="config-form">Loading...</div>`;

T.configForm = (port, pause) => `
  <div class="config-section">
    <h3>Server</h3>
    <div class="config-row">
      <label>Port</label>
      <input type="number" id="cfg-port" value="${port}" min="1" max="65535">
    </div>
    <div class="config-row">
      <label>Pause</label>
      <input type="checkbox" id="cfg-pause" ${pause ? 'checked' : ''}>
    </div>
  </div>
  <div class="config-section">
    <h3>Script Rules</h3>
    <div id="scripts-list"></div>
    <button class="btn btn-primary btn-sm add-row" onclick="addScriptRule()">+ Add Rule</button>
  </div>
  <div class="config-section">
    <h3>Response Modifiers</h3>
    <div id="response-list"></div>
    <button class="btn btn-primary btn-sm add-row" onclick="addResponseRule()">+ Add Rule</button>
  </div>
  <div class="config-section">
    <h3>Request Modifiers</h3>
    <div id="request-list"></div>
    <button class="btn btn-primary btn-sm add-row" onclick="addRequestRule()">+ Add Rule</button>
  </div>
  <button class="btn btn-primary btn-lg" onclick="saveConfig()">Save Configuration</button>
`;

T.ruleCard = (idx, domain, actions) => `
  <div class="rule-card" data-index="${idx}">
    <div class="rule-header">
      <span class="domain-label">${domain || '(empty domain)'}</span>
      ${actions}
    </div>
  </div>
`;

T.scriptRuleCard = (idx, r) => `
  <div class="rule-card" data-index="${idx}">
    <div class="rule-header">
      <span class="domain-label">${r.domain || '(empty domain)'}</span>
      <button class="btn btn-danger btn-sm" onclick="removeScriptRule(${idx})">Remove</button>
    </div>
    <div class="config-row"><label>Domain</label><input type="text" value="${r.domain}" onchange="updateScriptRule(${idx},'domain',this.value)"></div>
    <div class="config-row"><label>Scripts</label><input type="text" value="${(r.scripts||[]).join(', ')}" placeholder="comma-separated" onchange="updateScriptRule(${idx},'scripts',this.value.split(/,\s*/))"></div>
  </div>
`;

T.responseRuleCard = (idx, r) => `
  <div class="rule-card" data-index="${idx}">
    <div class="rule-header">
      <span class="domain-label">${r.domain || '(empty domain)'}</span>
      <button class="btn btn-danger btn-sm" onclick="removeResponseRule(${idx})">Remove</button>
    </div>
    <div class="config-row"><label>Domain</label><input type="text" value="${r.domain}" onchange="updateResponseRule(${idx},'domain',this.value)"></div>
    <div class="config-row">
      <label>CSP</label>
      <select onchange="updateResponseRule(${idx},'csp',this.value)">
        <option value="">(none)</option>
        <option value="remove_nonce" ${r.csp === 'remove_nonce' ? 'selected' : ''}>remove_nonce</option>
        <option value="remove_all" ${r.csp === 'remove_all' ? 'selected' : ''}>remove_all</option>
        <option value="relax_connect_src" ${r.csp === 'relax_connect_src' ? 'selected' : ''}>relax_connect_src</option>
        <option value="keep" ${r.csp === 'keep' ? 'selected' : ''}>keep</option>
      </select>
    </div>
    <div class="config-row"><label>Remove Headers</label></div>
    <div id="response-remove-headers-${idx}"></div>
    <button class="btn btn-sm btn-secondary" onclick="addResponseRemoveHeader(${idx})">+ Add Header to Remove</button>
    <div class="config-row config-row-gap"><label>Inject At</label>
      <select onchange="updateResponseRule(${idx},'inject_at',this.value||null)">
        <option value="">(default)</option>
        <option value="head_end" ${r.inject_at === 'head_end' ? 'selected' : ''}>head_end</option>
        <option value="body_end" ${r.inject_at === 'body_end' ? 'selected' : ''}>body_end</option>
        <option value="html_end" ${r.inject_at === 'html_end' ? 'selected' : ''}>html_end</option>
        <option value="append" ${r.inject_at === 'append' ? 'selected' : ''}>append</option>
      </select>
    </div>
    <div class="config-row config-row-gap"><label>Add Headers</label></div>
    <div id="response-headers-${idx}"></div>
    <button class="btn btn-sm btn-secondary" onclick="addResponseHeader(${idx})">+ Add Header</button>
  </div>
`;

T.requestRuleCard = (idx, r) => `
  <div class="rule-card" data-index="${idx}">
    <div class="rule-header">
      <span class="domain-label">${r.domain || '(empty domain)'}</span>
      <button class="btn btn-danger btn-sm" onclick="removeRequestRule(${idx})">Remove</button>
    </div>
    <div class="config-row"><label>Domain</label><input type="text" value="${r.domain}" onchange="updateRequestRule(${idx},'domain',this.value)"></div>
    <div class="config-row"><label>Remove Headers</label></div>
    <div id="request-remove-headers-${idx}"></div>
    <button class="btn btn-sm btn-secondary" onclick="addRequestRemoveHeader(${idx})">+ Add Header to Remove</button>
    <div class="config-row config-row-gap"><label>Add Headers</label></div>
    <div id="request-headers-${idx}"></div>
    <button class="btn btn-sm btn-secondary" onclick="addRequestHeader(${idx})">+ Add Header</button>
  </div>
`;

T.headerPair = (idx, hi, k, v, type) => `
  <div class="header-pair config-row">
    <input type="text" value="${k}" placeholder="Name" onchange="update${type}Header(${idx},${hi},'key',this.value)">
    <span>=</span>
    <input type="text" value="${v}" placeholder="Value" onchange="update${type}Header(${idx},${hi},'val',this.value)">
    <button class="btn btn-danger btn-sm" onclick="remove${type}Header(${idx},${hi})">x</button>
  </div>
`;

T.removeHeaderPair = (idx, hi, v) => `
  <div class="header-pair config-row">
    <input type="text" value="${v}" placeholder="Header name" onchange="updateRequestRemoveHeader(${idx},${hi},this.value)">
    <button class="btn btn-danger btn-sm" onclick="removeRequestRemoveHeader(${idx},${hi})">x</button>
  </div>
`;