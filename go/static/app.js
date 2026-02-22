/* ─── State ─────────────────────────────────────────────── */
let allCards   = [];
let allTags    = [];
let activeFilters = new Set();
let searchQ    = '';
let searchTimer = null;
let editingId  = null;   // null = creating
let pendingTags = [];    // tags being edited in modal
let pendingFile = null;  // File object for new photo
let removePhotoFlag = false;
let confirmCb  = null;

/* ─── API ────────────────────────────────────────────────── */
async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body instanceof FormData) { opts.body = body; }
  else if (body)                 { opts.body = JSON.stringify(body); opts.headers['Content-Type'] = 'application/json'; }
  const r = await fetch(path, opts);
  if (!r.ok) { const t = await r.text(); throw new Error(t || r.statusText); }
  const ct = r.headers.get('content-type') || '';
  return ct.includes('json') ? r.json() : null;
}

async function loadCards() {
  let url = '/api/cards';
  const params = [];
  if (searchQ)                 params.push('q=' + encodeURIComponent(searchQ));
  if (activeFilters.size > 0)  [...activeFilters].forEach(t => params.push('tag=' + encodeURIComponent(t)));
  if (params.length)           url += '?' + params.join('&');
  return api('GET', url);
}

async function loadTags() {
  return api('GET', '/api/tags');
}

/* ─── Init ───────────────────────────────────────────────── */
async function init() {
  const theme = localStorage.getItem('cv-theme') || 'light';
  setTheme(theme);
  showSkeletons();
  try {
    [allCards, allTags] = await Promise.all([loadCards(), loadTags()]);
    renderTagBar();
    renderGrid();
  } catch(e) {
    toast('Failed to load cards: ' + e.message, 'error');
    document.getElementById('cardGrid').innerHTML = '';
  }
}

/* ─── Theme ──────────────────────────────────────────────── */
function setTheme(t) {
  document.documentElement.setAttribute('data-theme', t);
  document.getElementById('themeToggle').textContent = t === 'dark' ? '☀️' : '🌙';
  localStorage.setItem('cv-theme', t);
}
function toggleTheme() {
  const cur = document.documentElement.getAttribute('data-theme');
  setTheme(cur === 'dark' ? 'light' : 'dark');
}

/* ─── Search ─────────────────────────────────────────────── */
function onSearch(v) {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(async () => {
    searchQ = v.trim();
    await refresh();
  }, 300);
}

/* ─── Tag filter ─────────────────────────────────────────── */
function renderTagBar() {
  const wrap = document.getElementById('tagChips');
  wrap.innerHTML = '';
  if (!allTags.length) { document.getElementById('tagBar').style.display = 'none'; return; }
  document.getElementById('tagBar').style.display = '';
  allTags.forEach(t => {
    const chip = document.createElement('button');
    chip.className = 'tag-chip' + (activeFilters.has(t.name) ? ' active' : '');
    chip.textContent = t.name + (t.count ? ` (${t.count})` : '');
    chip.onclick = () => toggleFilter(t.name);
    wrap.appendChild(chip);
  });
}
async function toggleFilter(tag) {
  if (activeFilters.has(tag)) activeFilters.delete(tag);
  else activeFilters.add(tag);
  await refresh();
}

/* ─── Refresh ────────────────────────────────────────────── */
async function refresh() {
  try {
    [allCards, allTags] = await Promise.all([loadCards(), loadTags()]);
    renderTagBar();
    renderGrid();
  } catch(e) { toast(e.message, 'error'); }
}

/* ─── Skeletons ──────────────────────────────────────────── */
function showSkeletons() {
  const g = document.getElementById('cardGrid');
  g.innerHTML = Array(6).fill(0).map(() => `
    <div class="skeleton">
      <div class="skel-row wide"></div>
      <div class="skel-row mid"></div>
      <div class="skel-row short"></div>
    </div>`).join('');
}

/* ─── Grid render ────────────────────────────────────────── */
function renderGrid() {
  const g = document.getElementById('cardGrid');
  const count = document.getElementById('cardCount');
  count.textContent = allCards.length ? `· ${allCards.length} card${allCards.length !== 1 ? 's' : ''}` : '';
  if (!allCards.length) {
    g.innerHTML = `<div class="empty-state">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="3" y="4" width="18" height="18" rx="2"/><path d="M16 2v4M8 2v4M3 10h18"/>
      </svg>
      <p>No cards found</p>
      <small>${searchQ || activeFilters.size ? 'Try a different search or filter' : 'Click "+ Add Card" to get started'}</small>
    </div>`;
    return;
  }
  g.innerHTML = allCards.map(c => cardHTML(c)).join('');
}

function initials(name) {
  const parts = name.trim().split(/\s+/);
  if (parts.length === 1) return parts[0][0].toUpperCase();
  return (parts[0][0] + parts[parts.length-1][0]).toUpperCase();
}

function avatarColor(name) {
  const palette = ['#4f46e5','#0284c7','#059669','#d97706','#dc2626','#7c3aed','#db2777','#0891b2'];
  let h = 0; for (const c of name) h = (h * 31 + c.charCodeAt(0)) & 0xffffffff;
  return palette[Math.abs(h) % palette.length];
}

function cardHTML(c) {
  const av = c.photo_url
    ? `<div class="avatar"><img src="${c.photo_url}" alt="" loading="lazy" onerror="this.parentNode.style.background='${avatarColor(c.name)}';this.parentNode.innerHTML='${initials(c.name)}'"></div>`
    : `<div class="avatar" style="background:${avatarColor(c.name)}">${initials(c.name)}</div>`;
  const phone = c.phones?.[0] ? `<div class="contact-row"><span>📱</span><span>${esc(c.phones[0].number)}</span></div>` : '';
  const email = c.emails?.[0] ? `<div class="contact-row"><span>📧</span><span>${esc(c.emails[0].address)}</span></div>` : '';
  const web   = c.website    ? `<div class="contact-row"><span>🌐</span><span>${esc(c.website)}</span></div>` : '';
  const tags  = (c.tags||[]).map(t => `<span class="tag-pill">${esc(t)}</span>`).join('');
  return `<div class="biz-card" onclick="openModal(${c.id})">
    <div class="biz-card-top">
      ${av}
      <div class="card-info">
        <div class="card-name">${esc(c.name)}</div>
        ${c.title   ? `<div class="card-title">${esc(c.title)}</div>` : ''}
        ${c.company ? `<div class="card-company">${esc(c.company)}</div>` : ''}
      </div>
    </div>
    ${phone||email||web ? `<div class="biz-card-mid">${phone}${email}${web}</div>` : ''}
    <div class="biz-card-bot">
      ${tags}
      <div class="card-actions" onclick="event.stopPropagation()">
        <button class="btn-sm" onclick="openModal(${c.id})">✏️</button>
        <button class="btn-sm danger" onclick="confirmDelete(${c.id},'${esc(c.name)}')">🗑️</button>
      </div>
    </div>
  </div>`;
}

function esc(s) { return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }

/* ─── Modal ──────────────────────────────────────────────── */
async function openModal(id) {
  editingId = id;
  pendingTags = [];
  pendingFile = null;
  removePhotoFlag = false;
  resetForm();

  if (id !== null) {
    document.getElementById('modalTitle').textContent = 'Edit Card';
    document.getElementById('deleteCardBtn').style.display = '';
    try {
      const c = await api('GET', `/api/cards/${id}`);
      fillForm(c);
    } catch(e) { toast('Failed to load card: ' + e.message, 'error'); return; }
  } else {
    document.getElementById('modalTitle').textContent = 'Add Card';
    document.getElementById('deleteCardBtn').style.display = 'none';
  }

  switchTab('basic');
  document.getElementById('modalOverlay').classList.add('open');
}

function closeModal() { document.getElementById('modalOverlay').classList.remove('open'); }
function overlayClick(e) { if (e.target.id === 'modalOverlay') closeModal(); }

function switchTab(name) {
  document.querySelectorAll('.modal-tab').forEach(b => b.classList.toggle('active', b.dataset.tab === name));
  document.querySelectorAll('.tab-panel').forEach(p => p.classList.toggle('active', p.id === 'tab-' + name));
}

function resetForm() {
  ['f-name','f-title','f-company','f-website','f-notes'].forEach(id => document.getElementById(id).value = '');
  ['f-name'].forEach(id => document.getElementById(id).classList.remove('error'));
  document.getElementById('phoneRows').innerHTML = '';
  document.getElementById('emailRows').innerHTML = '';
  document.getElementById('addressRows').innerHTML = '';
  document.getElementById('photoPreview').innerHTML = '📷';
  document.getElementById('removePhotoBtn').style.display = 'none';
  pendingTags = [];
  renderTagPills();
}

function fillForm(c) {
  document.getElementById('f-name').value    = c.name    || '';
  document.getElementById('f-title').value   = c.title   || '';
  document.getElementById('f-company').value = c.company || '';
  document.getElementById('f-website').value = c.website || '';
  document.getElementById('f-notes').value   = c.notes   || '';
  if (c.photo_url) {
    document.getElementById('photoPreview').innerHTML = `<img src="${c.photo_url}" alt="">`;
    document.getElementById('removePhotoBtn').style.display = '';
  }
  (c.phones||[]).forEach(p => addPhone(p.label, p.number));
  (c.emails||[]).forEach(e => addEmail(e.label, e.address));
  (c.addresses||[]).forEach(a => addAddress(a.label, a.street, a.city, a.country, a.postal));
  pendingTags = [...(c.tags||[])];
  renderTagPills();
}

/* ─── Dynamic rows ───────────────────────────────────────── */
function addPhone(label='mobile', number='') {
  const row = document.createElement('div'); row.className = 'dynamic-row';
  row.innerHTML = `
    <select class="label-select">
      ${['mobile','work','home','fax'].map(l => `<option${l===label?' selected':''}>${l}</option>`).join('')}
    </select>
    <div class="form-group flex1"><input class="form-input" type="tel" placeholder="+65 9123 4567" value="${esc(number)}"></div>
    <button class="remove-row-btn" onclick="this.parentNode.remove()">✕</button>`;
  document.getElementById('phoneRows').appendChild(row);
}
function addEmail(label='work', address='') {
  const row = document.createElement('div'); row.className = 'dynamic-row';
  row.innerHTML = `
    <select class="label-select">
      ${['work','personal'].map(l => `<option${l===label?' selected':''}>${l}</option>`).join('')}
    </select>
    <div class="form-group flex1"><input class="form-input" type="email" placeholder="email@example.com" value="${esc(address)}"></div>
    <button class="remove-row-btn" onclick="this.parentNode.remove()">✕</button>`;
  document.getElementById('emailRows').appendChild(row);
}
function addAddress(label='office', street='', city='', country='', postal='') {
  const row = document.createElement('div'); row.className = 'dynamic-row'; row.style.flexWrap = 'wrap';
  row.innerHTML = `
    <select class="label-select" style="width:90px">
      ${['office','home','other'].map(l => `<option${l===label?' selected':''}>${l}</option>`).join('')}
    </select>
    <div class="form-group flex1"><input class="form-input" placeholder="Street" value="${esc(street)}"></div>
    <div class="form-group" style="width:100%;display:flex;gap:8px;margin-top:4px">
      <input class="form-input" placeholder="City"    value="${esc(city)}">
      <input class="form-input" placeholder="Country" value="${esc(country)}">
      <input class="form-input" style="max-width:90px" placeholder="Postal" value="${esc(postal)}">
    </div>
    <button class="remove-row-btn" onclick="this.parentNode.remove()">✕</button>`;
  document.getElementById('addressRows').appendChild(row);
}

/* ─── Photo ──────────────────────────────────────────────── */
function onPhotoSelect(e) {
  const file = e.target.files[0];
  if (!file) return;
  if (file.size > 5*1024*1024) { toast('Photo must be under 5 MB', 'error'); return; }
  pendingFile = file;
  const reader = new FileReader();
  reader.onload = ev => {
    document.getElementById('photoPreview').innerHTML = `<img src="${ev.target.result}" alt="">`;
    document.getElementById('removePhotoBtn').style.display = '';
  };
  reader.readAsDataURL(file);
}
function removePhoto() {
  pendingFile = null;
  removePhotoFlag = true;
  document.getElementById('photoPreview').innerHTML = '📷';
  document.getElementById('removePhotoBtn').style.display = 'none';
}

// Drag and drop
const dropEl = document.getElementById('photoDrop');
if (dropEl) {
  dropEl.addEventListener('dragover', e => { e.preventDefault(); dropEl.classList.add('dragover'); });
  dropEl.addEventListener('dragleave', () => dropEl.classList.remove('dragover'));
  dropEl.addEventListener('drop', e => {
    e.preventDefault(); dropEl.classList.remove('dragover');
    const file = e.dataTransfer.files[0];
    if (file) { document.getElementById('photoFile').files = e.dataTransfer.files; onPhotoSelect({ target: { files: e.dataTransfer.files } }); }
  });
}

/* ─── Tags ───────────────────────────────────────────────── */
function onTagKey(e) {
  const v = e.target.value.trim();
  if ((e.key === 'Enter' || e.key === ',') && v) {
    e.preventDefault();
    addTag(v.replace(/,/g,'').trim());
    e.target.value = '';
  }
  if (e.key === 'Backspace' && !e.target.value && pendingTags.length) {
    pendingTags.pop(); renderTagPills();
  }
}
function onTagInput(e) {
  const v = e.target.value;
  if (v.endsWith(',')) {
    const t = v.slice(0,-1).trim();
    if (t) addTag(t);
    e.target.value = '';
  }
}
function addTag(t) {
  t = t.toLowerCase().trim();
  if (t && !pendingTags.includes(t)) { pendingTags.push(t); renderTagPills(); }
}
function removeTag(t) { pendingTags = pendingTags.filter(x => x !== t); renderTagPills(); }
function renderTagPills() {
  const el = document.getElementById('tagPills');
  el.innerHTML = pendingTags.map(t =>
    `<span class="tag-pill-removable">${esc(t)}<button onclick="removeTag('${esc(t)}')" type="button">×</button></span>`
  ).join('');
}

/* ─── Save ───────────────────────────────────────────────── */
async function saveCard() {
  const name = document.getElementById('f-name').value.trim();
  if (!name) {
    document.getElementById('f-name').classList.add('error');
    switchTab('basic');
    toast('Name is required', 'error');
    return;
  }
  document.getElementById('f-name').classList.remove('error');

  // Build phones
  const phones = [...document.getElementById('phoneRows').querySelectorAll('.dynamic-row')].map(r => ({
    label: r.querySelector('select').value,
    number: r.querySelector('input').value.trim()
  })).filter(p => p.number);

  // Build emails
  const emails = [...document.getElementById('emailRows').querySelectorAll('.dynamic-row')].map(r => ({
    label: r.querySelector('select').value,
    address: r.querySelector('input').value.trim()
  })).filter(e => e.address);

  // Build addresses
  const addresses = [...document.getElementById('addressRows').querySelectorAll('.dynamic-row')].map(r => {
    const inputs = r.querySelectorAll('input');
    return {
      label:   r.querySelector('select').value,
      street:  inputs[0]?.value.trim() || '',
      city:    inputs[1]?.value.trim() || '',
      country: inputs[2]?.value.trim() || '',
      postal:  inputs[3]?.value.trim() || ''
    };
  }).filter(a => a.street || a.city);

  const fd = new FormData();
  fd.append('name',    name);
  fd.append('title',   document.getElementById('f-title').value.trim());
  fd.append('company', document.getElementById('f-company').value.trim());
  fd.append('website', document.getElementById('f-website').value.trim());
  fd.append('notes',   document.getElementById('f-notes').value.trim());
  fd.append('phones',    JSON.stringify(phones));
  fd.append('emails',    JSON.stringify(emails));
  fd.append('addresses', JSON.stringify(addresses));
  fd.append('tags',      JSON.stringify(pendingTags));
  if (pendingFile) fd.append('photo', pendingFile);

  try {
    if (editingId === null) {
      await api('POST', '/api/cards', fd);
      toast('Card created', 'success');
    } else {
      await api('PUT', `/api/cards/${editingId}`, fd);
      if (removePhotoFlag) {
        try { await api('DELETE', `/api/cards/${editingId}/photo`); } catch(_) {}
      }
      toast('Card updated', 'success');
    }
    closeModal();
    await refresh();
  } catch(e) { toast('Error: ' + e.message, 'error'); }
}

/* ─── Delete ─────────────────────────────────────────────── */
function confirmDelete(id, name) {
  showConfirm(`Delete "${name}"? This cannot be undone.`, async () => {
    // Optimistic remove
    allCards = allCards.filter(c => c.id !== id);
    renderGrid();
    try {
      await api('DELETE', `/api/cards/${id}`);
      await refresh();
      toast('Card deleted', 'success');
    } catch(e) { await refresh(); toast('Delete failed: ' + e.message, 'error'); }
  });
}
function confirmDeleteCurrent() { confirmDelete(editingId, document.getElementById('f-name').value || 'this card'); closeModal(); }

/* ─── Confirm ────────────────────────────────────────────── */
function showConfirm(msg, cb) {
  document.getElementById('confirmMsg').textContent = msg;
  confirmCb = cb;
  document.getElementById('confirmOverlay').classList.add('open');
}
function closeConfirm() { document.getElementById('confirmOverlay').classList.remove('open'); confirmCb = null; }
function doConfirm()    { const cb = confirmCb; closeConfirm(); if (cb) cb(); }

/* ─── Toast ──────────────────────────────────────────────── */
function toast(msg, type='info') {
  const el = document.createElement('div');
  el.className = `toast ${type}`;
  el.textContent = msg;
  document.getElementById('toasts').prepend(el);
  setTimeout(() => el.remove(), 3500);
}

init();
