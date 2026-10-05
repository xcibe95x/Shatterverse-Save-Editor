import bundledStoneCatalog from './data/stones_observed.json';

// Current project credit: xcibe95x.
const REPOSITORY_URL = 'https://github.com/xcibe95x/Shatterverse-Save-Editor';
const invoke = (...args) => window.__TAURI__.core.invoke(...args);

const state = { save: null, page: 'overview', search: '', catalogSet: 'ALL', selectedStone: null, templateTargetIndex: null, modal: null, toast: null, expandedObservation: null, unlocksTab: 'sams', advancedClosed: new Set(), advancedOpen: null, backups: [] };

const CURRENCY_META = {
  CT: { label: 'Catalyst Orbs', hint: 'Spent on Catalyst Boons', icon: 'catalyst-orb.png' },
  GD: { label: 'GD', hint: 'Unknown currency', icon: 'coin', unsafe: true },
  SD: { label: 'Serious Dust', hint: '', icon: 'serious-dust.png' },
  Intel: { icon: 'intel.png' },
};
function currencyIcon(name) {
  const meta = CURRENCY_META[name] ?? { icon: 'coin' };
  return meta.icon.endsWith('.png')
    ? `<img class="currency-game-icon" src="/assets/${meta.icon}" alt="" aria-hidden="true">`
    : icon(meta.icon);
}
const unsafeTag = () => `<span class="unsafe-tag">(Unsafe)</span>`;
const rollLabels = { Sh: 'Shape ID', B: 'Base roll', M: 'Magnitude', F: 'Unknown byte' };
const FLAG_META = {
  bCC: { label: 'Challenge completed', hint: 'Marks a codex challenge as finished' },
  bCl: { label: 'Reward claimed', hint: 'Marks a finished challenge’s reward as collected' },
  bL: { label: 'bL', hint: 'Meaning not confirmed', unsafe: true },
};

const nav = [
  ['overview', 'coin', 'Currencies'], ['stones', 'diamond', 'Trinkets'],
  ['unlocks', 'lock', 'Unlocks'], ['progress', 'clock', 'Progress'],
  ['catalog', 'grid', 'Trinket Workshop'], ['lab', 'flask', 'All fields']
];

const ICONS = {
  home: '<path d="M3 10.5 10 4l7 6.5"/><path d="M5 9v7h10V9"/><path d="M8.3 16v-4h3.4v4"/>',
  diamond: '<path d="M7.5 2.5h5a1 1 0 0 1 1 1V5a3.5 3.5 0 0 1-7 0V3.5a1 1 0 0 1 1-1Z"/><path d="M8.5 8.3v1.7a1.5 1.5 0 0 0 3 0V8.3"/><rect x="5.5" y="10.5" width="9" height="7" rx="1.5"/><path d="M8.5 14h3"/>',
  clock: '<circle cx="10" cy="10" r="7"/><path d="M10 6v4l3 2"/>',
  grid: '<rect x="3" y="3" width="6" height="6"/><rect x="11" y="3" width="6" height="6"/><rect x="3" y="11" width="6" height="6"/><rect x="11" y="11" width="6" height="6"/>',
  flask: '<path d="M8 3h4M8.5 3v4.5L4.5 15a1.5 1.5 0 0 0 1.3 2.3h8.4a1.5 1.5 0 0 0 1.3-2.3l-4-7.5V3"/><path d="M6.3 12.5h7.4"/>',
  coin: '<circle cx="10" cy="10" r="7"/><circle cx="10" cy="10" r="4"/><path d="M10 7.3v5.4M8.4 10h3.2"/>',
  save: '<path d="M4 3h10l3 3v11H3V3z"/><path d="M6 3v5h8V3M6 17v-6h8v6"/>',
  gem: '<path d="M4.5 8 7 4h6l2.5 4L10 17z"/><path d="M4.5 8h11M7 4l1.5 4L10 17M13 4l-1.5 4L10 17"/>',
  shard: '<path d="M10 2 6 9l4 9 4-9z"/><path d="M6 9h8M10 2v16"/>',
  search: '<circle cx="8.5" cy="8.5" r="5.5"/><path d="m16.5 16.5-3.6-3.6"/>',
  star: '<path d="M10 2.6 12.3 7.6 17.8 8.3 13.8 12 14.9 17.5 10 14.7 5.1 17.5 6.2 12 2.2 8.3 7.7 7.6Z"/>',
  bolt: '<path d="M11 2 4 12h5l-1 6 7-10h-5z"/>',
  gear: '<circle cx="10" cy="10" r="2.6"/><path d="M10 3v2M10 15v2M17 10h-2M5 10H3M15 5l-1.4 1.4M6.4 13.6 5 15M15 15l-1.4-1.4M6.4 6.4 5 5"/>',
  warn: '<path d="M10 3 2 17h16Z"/><path d="M10 8.3v3.6M10 14.2v.1"/>',
  lock: '<rect x="4.5" y="9" width="11" height="8" rx="1"/><path d="M6.5 9V6a3.5 3.5 0 0 1 7 0v3"/>',
  unlock: '<rect x="4.5" y="9" width="11" height="8" rx="1"/><path d="M6.5 9V6a3.5 3.5 0 0 1 6.6-1.6"/>',
};
function icon(name, cls = '') { return `<svg class="icon ${cls}" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`; }
function stars(n, max = 6) {
  n = Math.max(0, Math.min(max, Number(n) || 0));
  let out = '';
  for (let i = 1; i <= max; i++) out += `<span class="star-pip ${i <= n ? 'lit' : ''}">${icon('star')}</span>`;
  return `<span class="star-row" title="Rank ${n} of ${max}">${out}</span>`;
}

const RARITY = {
  C: { name: 'Common', cls: 'common' },
  U: { name: 'Uncommon', cls: 'uncommon' },
  R: { name: 'Rare', cls: 'rare' },
  E: { name: 'Epic', cls: 'epic' },
  L: { name: 'Legendary', cls: 'legendary' },
};
function rarityLetterOf(id = '') { return /_([CUREL])$/.exec(id)?.[1] ?? null; }
function rarityOf(id = '') {
  if (id === 'AlienWeapon') return { name: 'Unique', cls: 'unique' };
  const letter = rarityLetterOf(id);
  return letter ? RARITY[letter] : { name: id, cls: 'unknown' };
}
function rarityTag(id) { const r = rarityOf(id); return `<span class="rarity-tag r-${r.cls}">${r.name}</span>`; }

const CATALOG_STORAGE_KEY = 'shatterverse-save-editor.local-stone-observations.v1';
function observationSignature(s) {
  return JSON.stringify([s.ID,s.Set,s.Lvl,s.R,s.Sh,s.B,s.M,s.F??null,Object.entries(s.SecS??{}).sort(([a],[b])=>a.localeCompare(b))]);
}
function loadLocalObservations() {
  try { const data=JSON.parse(localStorage.getItem(CATALOG_STORAGE_KEY)??'[]'); return Array.isArray(data)?data.filter(s=>s&&typeof s.ID==='string'&&typeof s.Set==='string'&&s.SecS&&typeof s.SecS==='object'):[]; }
  catch { return []; }
}
function mergeObservations(current, incoming) {
  const unique=new Map(current.map(s=>[observationSignature(s),s]));
  for(const stone of incoming) unique.set(observationSignature(stone),stone);
  return [...unique.values()];
}
let stoneCatalog = mergeObservations(bundledStoneCatalog, loadLocalObservations());
function persistSupplementalObservations() {
  try {
    const base=new Set(bundledStoneCatalog.map(observationSignature));
    localStorage.setItem(CATALOG_STORAGE_KEY,JSON.stringify(stoneCatalog.filter(s=>!base.has(observationSignature(s)))));
    return true;
  } catch { return false; }
}

function esc(value = '') { return String(value).replace(/[&<>"']/g, c => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[c]); }
function toast(message, error = false) {
  state.toast = { message, error }; render();
  setTimeout(() => { state.toast = null; render(); }, 3600);
}
async function act(command, args = {}, ok = '') {
  try { state.save = await invoke(command, args); state.modal = null; render(); if (ok) toast(ok); }
  catch (e) { toast(String(e), true); }
}
const fmt = n => new Intl.NumberFormat().format(n ?? 0);
const props = () => state.save?.properties ?? [];
const progressProps = () => props().filter(p => p.editable && /(^|\[)(CP|SP|XP|Progress|Level|Lvl|Rank|R)(\]|$)/i.test(p.name) && !p.scope.startsWith('Stone '));
const RARITY_ORDER = ['common', 'uncommon', 'rare', 'epic', 'legendary', 'unknown'];
let stoneChoices = [];
let stoneSets = [];
function refreshStoneCatalogChoices() {
  stoneChoices = stoneCatalog.map((stone,index)=>({...stone,index,rarity:rarityOf(stone.ID)}))
    .sort((a,b)=>RARITY_ORDER.indexOf(a.rarity.cls)-RARITY_ORDER.indexOf(b.rarity.cls)||a.ID.localeCompare(b.ID)||a.Set.localeCompare(b.Set)||b.R-a.R);
  stoneSets = [...new Set(stoneCatalog.map(s=>s.Set))].sort();
}
refreshStoneCatalogChoices();
const STAT_RARITY = { C: 'Common', U: 'Uncommon', R: 'Rare', E: 'Epic', L: 'Legendary' };
function friendlyStatName(key) {
  const match = key.match(/^(.*)_([CUREL])$/);
  const base = (match?.[1] ?? key).replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/Ao E/g, 'AoE');
  return match ? `${base} · ${STAT_RARITY[match[2]]}` : base;
}
function friendlyStatHtml(key) {
  const match=key.match(/^(.*)_([CUREL])$/);
  const base=(match?.[1]??key).replace(/([a-z0-9])([A-Z])/g,'$1 $2').replace(/Ao E/g,'AoE');
  return match ? `${esc(base)} <span class="stat-rarity tier-${match[2]}">${STAT_RARITY[match[2]]}</span>` : esc(base);
}
function stoneNumberField(prop, label, hint, min=-2147483648, max=2147483647) {
  if (!prop) return `<div class="stone-stat-unavailable"><b>${esc(label)}</b><small>Not recognized in this save.</small></div>`;
  return `<label class="stone-edit-field"><span><b>${esc(label)}</b><small>${esc(hint)}</small></span><input aria-label="${esc(label)}" data-value="${prop.value_at}" type="number" min="${min}" max="${max}" step="1" required value="${esc(prop.value)}"></label>`;
}
function unsafeByteField(prop) {
  if (!prop) return '';
  return `<label class="stone-edit-field"><span><b>F ${unsafeTag()}</b><small>Unknown flag (0 or 1). Editing F has broken saves; only experiment on a backup copy.</small></span><input aria-label="F (Unsafe)" data-value="${prop.value_at}" type="number" min="0" max="1" step="1" required value="${esc(prop.value)}"></label>`;
}
function trinketArt(rarity) {
  const tier = ['common','uncommon','rare','epic','legendary'].includes(rarity) ? rarity : 'rare';
  return `<img src="/assets/trinket-${tier}.png" alt="${tier} trinket" aria-hidden="true">`;
}

function captureViewState(root) {
  const pathTo = element => {
    const path=[];
    while(element&&element!==root){
      const parent=element.parentElement;
      if(!parent)return null;
      path.unshift(Array.prototype.indexOf.call(parent.children,element));
      element=parent;
    }
    return element===root?path:null;
  };
  const scroll=[];
  for(const element of [root,...root.querySelectorAll('*')]){
    if(element.scrollHeight>element.clientHeight+1||element.scrollWidth>element.clientWidth+1){
      scroll.push({path:pathTo(element),top:element.scrollTop,left:element.scrollLeft});
    }
  }
  const active=document.activeElement;
  const focus=active&&root.contains(active)&&/^(INPUT|SELECT|TEXTAREA)$/.test(active.tagName)?pathTo(active):null;
  return {x:window.scrollX,y:window.scrollY,scroll,focus};
}
function restoreViewState(root, view) {
  const atPath=path=>{
    let element=root;
    for(const index of path??[]){element=element?.children[index];if(!element)return null;}
    return element;
  };
  for(const item of view.scroll){
    const element=atPath(item.path);
    if(element){element.scrollTop=item.top;element.scrollLeft=item.left;}
  }
  if(view.focus){
    const element=atPath(view.focus);
    if(element&&/^(INPUT|SELECT|TEXTAREA)$/.test(element.tagName))element.focus({preventScroll:true});
  }
  window.scrollTo(view.x,view.y);
}

function render() {
  const root=document.querySelector('#app');
  const view=captureViewState(root);
  const s = state.save;
  root.innerHTML = `
    <div class="shell">
      <aside class="rail">
        <div class="brand"><img class="brand-mark" src="/app-icon.ico" alt="" /><div><b>SHATTERVERSE</b><small>SAVE EDITOR</small></div></div>
        <div class="rail-card"><div class="rail-card-top"><span>${s?'ACTIVE SAVE':'NO SAVE OPEN'}</span></div><p title="${s?esc(s.filename):'Choose a save file'}">${s?esc(s.filename):'Choose a save file'}</p>${s?`<small>${fmt(s.size)} bytes</small>`:`<button class="rail-open" data-action="open">Browse saves <span>↗</span></button>`}</div>
        <div class="rail-label">EDIT SAVE</div>
        <nav>${nav.map(([id, ic, text]) => `<button class="nav-item ${state.page===id?'active':''}" data-page="${id}"><span class="nav-icon">${icon(ic)}</span>${text}${id==='stones'&&s?`<em>${s.stones.length}</em>`:''}</button>`).join('')}</nav>
        <div class="rail-spacer"></div>
        <div class="rail-foot"><span>LOCAL SAVE · V0.4.11 BETA · XCIBE95X</span><a class="repository-link" href="${esc(REPOSITORY_URL)}" target="_blank" rel="noreferrer">GitHub repository ↗</a></div>
      </aside>
      <main class="main">
        <header class="topbar"><div class="crumb"><span class="crumb-root">SAVE</span><b>/</b><strong>${esc(nav.find(x=>x[0]===state.page)?.[2]??'Currencies')}</strong><span class="crumb-file">${s?esc(s.filename):'No save loaded'}</span></div><div class="top-actions">${s?`<span class="save-state ${s.has_unsaved_changes?'pending':''}">${s.has_unsaved_changes?'CHANGES PENDING':'SAVE LOADED'}</span>${s.has_unsaved_changes?'<button class="button button-quiet" data-action="undo">Revert</button>':''}<button class="button button-quiet" data-action="open-backups">Backups</button>`:''}<button class="button button-quiet" data-action="open">Open save</button><button class="button button-save" data-action="save" ${!s?.has_unsaved_changes?'disabled':''}>Save changes <span>↗</span></button></div></header>
        ${s ? pageContent() : welcome()}
        <footer class="statusbar"><span>${s?'Save loaded':'No save open'}</span><span>Your save stays on this device</span></footer>
      </main>
    </div>
    ${state.modal?modalHtml():''}${state.toast?`<div class="toast ${state.toast.error?'error':''}"><span>${state.toast.error?'!':'✓'}</span>${esc(state.toast.message)}</div>`:''}`;
  bind();
  restoreViewState(root,view);
}

function welcome() {
  return `<section class="welcome"><div class="welcome-content"><div class="welcome-kicker">SERIOUS SAM: SHATTERVERSE</div><h1>Open a save file</h1><p>Select a local save to view and edit its currencies, trinkets, and supported progression.</p><button class="button button-primary" data-action="open">Browse save files <span>↗</span></button><div class="welcome-note"><span>Files stay on this device.</span><span>A backup is created before saving.</span></div></div></section>`;
}

function pageContent() {
  if(state.page==='stones') return stonesPage();
  if(state.page==='unlocks') return unlocksPage();
  if(state.page==='progress') return progressPage();
  if(state.page==='catalog') return catalogPage();
  if(state.page==='lab') return labPage();
  return overviewPage();
}
function pageIntro(kicker,title,desc,side='') { return `<div class="page-intro"><div><div class="eyebrow">${kicker}</div><h1>${title}</h1><p>${desc}</p></div>${side}</div>`; }

function overviewPage() {
  const s=state.save;
  return `<div class="page-body currency-page">${pageIntro('Save editor','Currencies','Currency balances must be at least 1 for the game to load this save.')}
      <section class="panel currency-editor"><div class="currency-editor-head"><div><h2>Player balances</h2><p>Values must be at least 1 for the game to load this save.</p></div><span>${s.currencies.length} ${s.currencies.length===1?'currency':'currencies'}</span></div>
      <div class="currency-editor-list">${s.currencies.map(c=>{const meta=CURRENCY_META[c.key]??{label:c.key,hint:'',icon:'coin'};return `<div class="currency-editor-row"><span class="currency-editor-icon">${currencyIcon(c.key)}</span><span class="currency-editor-label"><b>${esc(meta.label)}</b>${meta.hint?`<small>${esc(meta.hint)}</small>`:''}</span><label class="currency-editor-value"><span>Amount</span><input aria-label="${esc(meta.label)} amount" class="currency-tile-input" data-value="${c.offset}" value="${c.value}" type="number" min="1" max="2147483647" required></label></div>`;}).join('')||`<div class="empty-inline">No currency values were recognized in this save.</div>`}</div>
      ${['CT','GD','SD'].filter(k=>!s.currencies.some(c=>c.key===k)).map(k=>{const meta=CURRENCY_META[k];return `<div class="currency-editor-row currency-editor-missing"><span class="currency-editor-icon">${currencyIcon(k)}</span><span class="currency-editor-label"><b>${esc(meta.label)}</b><small>This currency is not stored in this save. Earn it in-game first, then reopen the save to edit it safely.</small></span></div>`;}).join('')}
    </section>
    ${s.warnings.length?`<div class="warning-strip"><span>${icon('warn')}</span><div><b>Save notes</b><small>${esc(s.warnings.join(' · '))}</small></div></div>`:''}</div>`;
}
function stoneCard(stone) { const r=rarityOf(stone.id); return `<button class="stone-card r-${r.cls}" data-stone="${stone.index}"><span class="stone-glyph">${trinketArt(r.cls)}</span><small>Trinket ${stone.index}</small><b>${esc(stone.id)}</b>${rarityTag(stone.id)}${stars(stone.rank)}</button>`; }
function stonesPage() {
  const list=state.save.stones.filter(s=>`${s.id} ${s.stone_set} ${s.index}`.toLowerCase().includes(state.search.toLowerCase()));
  const selected=state.save.stones.find(s=>s.index===state.selectedStone)??state.save.stones[0];
  const rarityCounts=Object.values(RARITY).map(r=>({ ...r, count:state.save.stones.filter(s=>rarityOf(s.id).cls===r.cls).length }));
  return `<div class="page-body trinket-page">${pageIntro('INVENTORY','Trinkets',`${state.save.stones.length} owned · ${list.length} shown`, `<label class="searchbox">${icon('search')}<input id="stone-search" aria-label="Search trinkets" placeholder="Search inventory" value="${esc(state.search)}"></label>`)}
    <div class="stone-workspace"><section class="panel inventory-panel"><div class="inventory-toolbar"><div class="inventory-list-heading"><div><h2>Owned trinkets</h2><p>Select an item to inspect or edit it.</p></div><span>${list.length} ITEMS</span></div><button class="button button-primary" data-action="max-all-trinkets" title="Sets Level and Rank to 6 and raises Magnitude by 7 percentage points for each rank gained. Leaves Base roll and every other field unchanged.">Max All Trinkets</button></div><div class="trinket-grid">${list.map(s=>{const r=rarityOf(s.id);return `<button class="trinket-tile r-${r.cls} ${selected?.index===s.index?'selected':''}" data-stone="${s.index}" aria-pressed="${selected?.index===s.index}"><span class="trinket-art">${trinketArt(r.cls)}</span><span class="trinket-tile-info"><b>${esc(s.id)}</b><small>${esc(s.stone_set)}</small><span class="trinket-tile-meta">${rarityTag(s.id)}<span>Rank ${s.rank}</span></span>${stars(s.rank)}</span></button>`;}).join('')||`<div class="inventory-empty"><b>No Trinkets available</b><p>${state.save.stones.length?'No items match this search.':'This save has no owned trinkets yet. The trinket array is absent, so adding is unavailable. Earn a trinket in-game, then reopen this save.'}</p></div>`}</div><div class="inventory-foot"><div class="rarity-legend">${rarityCounts.map(r=>`<span class="legend-${r.cls}"><i></i>${r.count}</span>`).join('')}</div><b>${state.save.stones.length}/100</b></div></section>
      ${selected?(()=>{const r=rarityOf(selected.id);const stoneScope=`Stone ${selected.index}`;const field=(name)=>props().find(p=>p.scope===stoneScope&&p.name===name&&p.editable&&p.value_at!==null);return `<aside class="panel detail-panel trinket-detail r-${r.cls}"><div class="detail-top"><span>Trinket details</span><div class="detail-top-actions"><span class="record-number">Record ${selected.index}</span><button class="button button-primary max-trinket-button" data-action="max-trinket" data-index="${selected.index}" title="Sets Level and Rank to 6 and raises Magnitude by 7 percentage points for each rank gained. Leaves Base roll and every other field unchanged.">Max Trinket</button></div></div><div class="trinket-detail-heading"><div class="detail-sigil">${trinketArt(r.cls)}</div><div class="detail-title"><h2>${esc(selected.id)}</h2><p>${esc(selected.stone_set)}</p><div class="detail-rarity">${rarityTag(selected.id)}${rarityLetterOf(selected.id)?`<select class="rarity-select" data-rarity-index="${selected.index}" aria-label="Rarity suffix">${Object.entries(RARITY).map(([letter,info])=>`<option value="${letter}" ${letter===rarityLetterOf(selected.id)?'selected':''}>${info.name} (_${letter})</option>`).join('')}</select>`:''}<span>Rank ${selected.rank} of 6</span>${selected.id==='AlienWeapon'?`<small class="rarity-note">Unique item — rarity tier is not stored in its ID.</small>`:''}</div>${stars(selected.rank)}</div></div><div class="detail-fields"><section class="trinket-detail-section"><div class="trinket-section-title">Progression</div><div class="field-row"><div><b>Level</b><small>How developed this trinket is.</small></div><div class="stepper"><button aria-label="Decrease level" data-step="${selected.level_offset}" data-delta="-1">−</button><input aria-label="Trinket level" data-value="${selected.level_offset}" value="${selected.level}" type="number" min="0" max="2147483647" step="1" required><button aria-label="Increase level" data-step="${selected.level_offset}" data-delta="1">＋</button></div></div><div class="field-row"><div><b>Rank</b><small>Higher rank adds more stars.</small></div><div class="stepper"><button aria-label="Decrease rank" data-step="${selected.rank_offset}" data-delta="-1">−</button><input aria-label="Trinket rank" data-value="${selected.rank_offset}" value="${selected.rank}" type="number" min="0" max="6" step="1" required><button aria-label="Increase rank" data-step="${selected.rank_offset}" data-delta="1">＋</button></div></div></section><section class="trinket-detail-section"><div class="trinket-section-title">Primary Stats</div>${stoneNumberField(field('M'),'Magnitude','Main trinket stat value.')}</section></div><div class="secondary-block"><div class="trinket-section-title"><b>Bonus Stats</b><span>${selected.secondary.length} values</span></div>${selected.secondary.length?selected.secondary.map(([k,v])=>{const prop=field(`SecS[${k}]`);return prop?`<label class="secondary-row secondary-edit"><span><b>${friendlyStatHtml(k)}</b><small>${esc(k)}</small></span><input aria-label="${esc(friendlyStatName(k))}" data-value="${prop.value_at}" type="number" min="-2147483648" max="2147483647" step="1" required value="${esc(v)}"></label>`:`<div class="secondary-row"><span>${friendlyStatHtml(k)}<small>${esc(k)}</small></span><b>${fmt(v)}</b></div>`}).join(''):'<div class="empty-inline">No bonus stats were decoded for this trinket.</div>'}</div><section class="trinket-appearance trinket-detail-section"><div class="trinket-section-title">Appearance &amp; Slot</div><p>These saved values can change how the trinket looks or which slot it fits.</p>${stoneNumberField(field('B'),'Base Roll','Can affect the in-game model or icon.')}${stoneNumberField(field('Sh'),'Shape ID','Controls the trinket slot shape. Observed range: 1–6.',1,6)}${selected.foil!==null?unsafeByteField(field('F')):''}</section><div class="detail-actions"><button class="button button-quiet" data-action="export-trinket" data-index="${selected.index}">Export to file</button><button class="button button-danger" data-action="open-remove" data-index="${selected.index}" ${state.save.stones.length>1?'':'disabled'}>Remove trinket</button><p class="safe-edit-note">Use Trinket Workshop to replace this item. Add/remove actions change save structure and require an extra backup confirmation.</p></div></aside>`;})():`<aside class="panel detail-panel detail-empty"><b>No trinket selected</b><span>Choose an item from the collection to view its details.</span></aside>`}</div></div>`;
}
const WEAPON_IDS = new Set(['Raygun','RaptorSniperRifle','MiniGun','XOPFlamethrower','MKGrenadeLauncher','RocketLauncher','SBCCannon','DoubleBarrelShotgun','LaserPistol','Lasergun','AlphaRevolver','AlphaRevolverAlt','PulseShotgun','DimensionBreaker','ReptiloidSpellbook']);
function unlockGroups() {
  const items = props().filter(p => p.scope.startsWith('Unlock: '));
  const byId = new Map();
  for (const p of items) {
    const id = p.scope.slice('Unlock: '.length);
    if (!byId.has(id)) byId.set(id, {});
    byId.get(id)[p.name] = p;
  }
  const sams = [], weapons = [], intels = [];
  for (const [id, fields] of byId) {
    const bucket = /^Sam_\d+$/.test(id) ? sams : WEAPON_IDS.has(id) ? weapons : intels;
    bucket.push({ id, bl: fields.bL, sp: fields.SP });
  }
  const bySort = (a, b) => a.id.localeCompare(b.id);
  return { sams: sams.sort(bySort), weapons: weapons.sort(bySort), intels: intels.sort(bySort) };
}
function codexGroups() {
  // A Codex record also contains ID, ServerId, and the CQP array header.
  // Only these four fields belong to each individual challenge.
  const challengeFields = new Set(['CID', 'CP', 'bCC', 'bCl']);
  const items = props().filter(p => p.scope.startsWith('Codex: ') && challengeFields.has(p.name));
  const byId = new Map();
  for (const p of items) {
    const id = p.scope.slice('Codex: '.length);
    if (!byId.has(id)) byId.set(id, []);
    byId.get(id).push(p);
  }
  const entries = [];
  for (const [id, fields] of byId) {
    fields.sort((a, b) => a.tag_at - b.tag_at);
    const challenges = [];
    let challenge = null;
    for (const field of fields) {
      if (field.name === 'CID') {
        if (challenge?.cid) challenges.push(challenge);
        challenge = { cid: field };
      } else if (challenge) {
        if (field.name === 'CP') challenge.cp = field;
        else if (field.name === 'bCC') challenge.bcc = field;
        else if (field.name === 'bCl') challenge.bcl = field;
      }
    }
    if (challenge?.cid) challenges.push(challenge);
    entries.push({ id, challenges });
  }
  return entries.sort((a, b) => a.id.localeCompare(b.id));
}
function unlockRow(item) {
  // In these unlock records bL is the locked flag: zero means unlocked.
  const unlocked = !item.bl || Number(item.bl.value) === 0;
  return `<div class="unlock-row"><div class="unlock-id">${icon(unlocked?'unlock':'lock', unlocked?'unlocked':'locked')}<b>${esc(item.id)}</b></div>${item.sp?`<div class="unlock-stack"><small>Stack level</small><input class="property-input" data-value="${item.sp.value_at}" value="${esc(item.sp.value)}" type="number" step="1" required></div>`:''}${item.bl?`<button class="toggle ${unlocked?'on':''}" data-bool="${item.bl.value_at}" aria-label="Unlocked: ${esc(item.id)}" aria-pressed="${unlocked}"><i></i></button>`:'<span class="readout">No unlock flag found</span>'}</div>`;
}
function challengeRow(entry) {
  return `<div class="codex-entry"><div class="codex-entry-id">${esc(entry.id)}</div>${entry.challenges.map((c,i)=>`<div class="codex-challenge"><small>Challenge ${i+1}${c.cid?` · CID ${c.cid.value}`:''}</small><div class="codex-challenge-fields">${c.cp?.value_at!=null?`<label>Progress<input class="property-input" data-value="${c.cp.value_at}" value="${esc(c.cp.value ?? 0)}" type="number" min="0" required></label>`:''}${c.bcc?.value_at!=null?`<label>Completed<button class="toggle ${Number(c.bcc.value)!==0?'on':''}" data-bool="${c.bcc.value_at}" aria-label="Completed" aria-pressed="${Number(c.bcc.value)!==0}"><i></i></button></label>`:''}${c.bcl?.value_at!=null?`<label>Claimed<button class="toggle ${Number(c.bcl.value)!==0?'on':''}" data-bool="${c.bcl.value_at}" aria-label="Claimed" aria-pressed="${Number(c.bcl.value)!==0}"><i></i></button></label>`:''}</div></div>`).join('')}</div>`;
}
function advancedEntries(q) {
  const items = props().filter(p => p.scope.startsWith('Unlock: ') || p.scope.startsWith('Codex: '));
  const byId = new Map();
  for (const p of items) {
    const kind = p.scope.startsWith('Codex: ') ? 'Codex' : 'Unlock';
    const id = p.scope.slice(p.scope.indexOf(': ') + 2);
    const key = `${kind}:${id}`;
    if (!byId.has(key)) byId.set(key, { id, kind, fields: [] });
    byId.get(key).fields.push(p);
  }
  for (const e of byId.values()) e.fields.sort((a, b) => a.tag_at - b.tag_at);
  return [...byId.values()].filter(e => e.id.toLowerCase().includes(q)).sort((a, b) => a.id.localeCompare(b.id));
}
function advancedField(p) {
  const input = p.type_name === 'BoolProperty'
    ? `<button class="toggle ${Number(p.value) !== 0 ? 'on' : ''}" data-bool="${p.value_at}"><i></i></button>`
    : p.editable && p.value_at !== null
      ? `<input class="property-input json-value" data-value="${p.value_at}" value="${esc(p.value)}" type="number" required>`
      : `<span class="readout">${esc(labDisplay(p))}</span>`;
  return `<div class="json-row"><span class="json-key">"${esc(p.name)}"</span><span class="json-colon">:</span>${input}<span class="json-type">${esc(p.type_name.replace('Property',''))}</span></div>`;
}
function advancedRow(entry, index) {
  const key = `${entry.kind}:${entry.id}`;
  const open = state.advancedClosed.has(key) ? false : (state.advancedOpen === key || (!state.advancedOpen && index === 0));
  return `<div class="json-entry"><button class="json-entry-head" data-advanced-toggle="${esc(key)}"><span class="json-caret ${open?'open':''}">▸</span><span class="json-entry-kind">${entry.kind}</span><b>${esc(entry.id)}</b><span class="tiny-label">${entry.fields.length} fields</span></button>${open?`<div class="json-body">${entry.fields.map(advancedField).join('')}</div>`:''}</div>`;
}
function unlocksPage() {
  const q = state.search.toLowerCase();
  const { sams, weapons, intels } = unlockGroups();
  const codex = codexGroups().filter(e => e.id.toLowerCase().includes(q));
  const filt = arr => arr.filter(i => i.id.toLowerCase().includes(q));
  const tabs = [
    ['sams', `Sams (${sams.length})`],
    ['weapons', `Weapons (${weapons.length})`],
    ['intels', `Intels (${intels.length})`],
    ['codex', `Codex (${codex.length})`],
    ['advanced', 'Advanced'],
  ];
  const tab = state.unlocksTab;
  let body = '';
  if (tab === 'sams') body = `<section class="panel unlock-panel"><div class="panel-head"><div><span class="section-code">Characters</span><h2>Sams</h2></div><span class="tiny-label">${filt(sams).length} found</span></div><div class="unlock-list">${filt(sams).map(unlockRow).join('')||'<div class="empty-inline">None match.</div>'}</div></section>`;
  else if (tab === 'weapons') body = `<section class="panel unlock-panel"><div class="panel-head"><div><span class="section-code">Armory</span><h2>Weapons</h2></div><span class="tiny-label">${filt(weapons).length} found</span></div><div class="unlock-list">${filt(weapons).map(unlockRow).join('')||'<div class="empty-inline">None match.</div>'}</div></section>`;
  else if (tab === 'intels') body = `<section class="panel unlock-panel"><div class="panel-head"><div class="intel-heading"><img src="/assets/intel.png" alt="" aria-hidden="true"><span><span class="section-code">Codex · Utilities</span><h2>Intels</h2></span></div><span class="tiny-label">${filt(intels).length} found</span></div><div class="unlock-list">${filt(intels).map(unlockRow).join('')||'<div class="empty-inline">None match.</div>'}</div></section>`;
  else if (tab === 'codex') body = `<section class="panel unlock-panel codex-panel"><div class="panel-head"><div><span class="section-code">Codex</span><h2>Bestiary, missions &amp; anomalies</h2></div><span class="tiny-label">${codex.length} found</span></div><div class="unlock-list">${codex.map(challengeRow).join('')||'<div class="empty-inline">None match.</div>'}</div></section>`;
  else { const entries = advancedEntries(q); body = `<section class="panel unlock-panel codex-panel advanced-panel"><div class="panel-head"><div><span class="section-code">Raw data</span><h2>Advanced entries</h2><p>Open one record at a time to inspect its saved fields. Values retain their original names and types.</p></div><span class="tiny-label">${entries.length} entries</span></div><div class="json-tree">${entries.map(advancedRow).join('')||'<div class="empty-inline">No entries match this search.</div>'}</div></section>`; }
  return `<div class="page-body">
    <div class="risk-banner"><span>${icon('warn')}</span><div><b>Edit at your own risk.</b><span>These fields come straight from the save's raw data, not official game text. Field meanings are reverse-engineered and not all confirmed — back up your save before changing anything here.</span></div></div>
    ${pageIntro('Progression','Unlocks','Characters, weapons, and Intels are listed from the shared save unlock data — plus the real Codex (bestiary, bosses, missions, anomalies) and a raw Advanced view below.',`<label class="searchbox">${icon('search')}<input id="unlocks-search" placeholder="Search…" value="${esc(state.search)}"></label>`)}
    <div class="tab-row">${tabs.map(([id,label])=>`<button class="tab-btn ${tab===id?'active':''}" data-unlocks-tab="${id}">${label}</button>`).join('')}</div>
    ${body}
  </div>`;
}
function replacementCompatibility(template, target) {
  if (!target) return {ok:false,reason:'This save has no owned trinket slot to replace.'};
  const bytes = value => new TextEncoder().encode(String(value)).length;
  if (bytes(template.ID) !== bytes(target.id)) return {ok:false,reason:'The template ID is a different length, so it cannot safely overwrite this slot.'};
  if (bytes(template.Set) !== bytes(target.stone_set)) return {ok:false,reason:'The template set name is a different length, so it cannot safely overwrite this slot.'};
  const a = Object.keys(template.SecS).sort(), b = target.secondary.map(([key])=>key).sort();
  if (a.length !== b.length || a.some((key,i)=>key !== b[i])) return {ok:false,reason:'The template bonus-stat keys do not match this slot. Choose a template with the same bonus-stat layout.'};
  return {ok:true,reason:'Compatible: this overwrites fixed-size values in the selected owned slot.'};
}
function catalogPage() {
  const setFilter=state.catalogSet;
  const query=state.search.toLowerCase();
  const rows=stoneCatalog.map((stone,index)=>({stone,index})).filter(({stone})=>
    (setFilter==='ALL'||stone.Set===setFilter) &&
    `${stone.ID} ${stone.Set} ${Object.keys(stone.SecS).join(' ')}`.toLowerCase().includes(query));
  const expanded=state.expandedObservation;
  const stones=state.save.stones;
  const canReplace=stones.length>0;
  const targetIndex=state.templateTargetIndex??state.selectedStone??stones[0]?.index;
  const target=stones.find(s=>s.index===targetIndex);
  const canAdd=stones.length>0;
  return `<div class="page-body workshop-page">${pageIntro('TRINKET WORKSHOP','Replace a trinket','Choose a template and an existing slot. Replacement keeps the save size and slot count unchanged.',`<label class="searchbox workshop-slot"><span>Owned slot</span><select id="template-target" aria-label="Choose owned trinket slot">${stones.map(s=>`<option value="${s.index}" ${s.index===targetIndex?'selected':''}>#${String(s.index).padStart(2,'0')} · ${esc(s.id)} (${esc(s.stone_set)})</option>`).join('')}</select></label>`)}
    <div class="workshop-actions"><div class="workshop-risk-note">${icon('warn')}<span>Create/remove changes save structure and can corrupt the file. A backup and confirmation are required.</span></div><div><button class="button button-quiet" data-action="import-stone-save">Import save data</button><button class="button button-quiet" data-action="import-exported-trinket">Import .trinket</button><button class="button button-primary" data-action="open-add" ${canAdd?'':'disabled'}>Create from data</button><button class="button button-danger" data-action="open-remove" data-index="${targetIndex??''}" ${canAdd&&stones.length>1?'':'disabled'}>Remove selected</button></div></div>
    ${!canAdd?`<div class="warning-strip"><span>${icon('warn')}</span><div><b>No owned trinket slots</b><small>This save has no trinket record to use as a structural template. Earn a trinket in-game first, then reopen this save.</small></div></div>`:''}
    <div class="catalog-summary"><div><b>${stoneCatalog.length}</b><span>OBSERVED RECORDS</span></div><div><b>${new Set(stoneCatalog.map(s=>s.ID)).size}</b><span>UNIQUE STONE IDS</span></div><div><b>${stoneSets.length}</b><span>OBSERVED SETS</span></div><div class="catalog-source">LOCAL CATALOG · ${stoneChoices.length} FULL VALUE RECORDS</div></div>
    <section class="panel catalog-panel"><div class="catalog-toolbar"><label class="searchbox">${icon('search')}<input id="catalog-search" placeholder="Search trinket, set, bonus stat…" value="${esc(state.search)}"></label><select id="catalog-set" class="catalog-filter" aria-label="Filter by set"><option value="ALL">All observed sets</option>${stoneSets.map(set=>`<option value="${esc(set)}" ${setFilter===set?'selected':''}>${esc(set)}</option>`).join('')}</select><span class="catalog-result">${rows.length} MATCHING RECORDS</span></div>
      <div class="catalog-head"><span>OBSERVED TRINKET</span><span>SET</span><span>RANK</span><span>MAGNITUDE · BASE ROLL · SHAPE · F</span><span></span></div>
      <div class="catalog-list">${rows.map(({stone,index})=>{const r=rarityOf(stone.ID);const check=replacementCompatibility(stone,target);return `<div class="catalog-entry r-${r.cls} ${expanded===index?'expanded':''}"><button class="catalog-row" data-observation="${index}"><span class="catalog-identity"><i class="catalog-art">${trinketArt(r.cls)}</i><b>${esc(stone.ID)}</b>${rarityTag(stone.ID)}<small>OBS ${String(index+1).padStart(2,'0')}</small></span><span class="catalog-set">${esc(stone.Set)}</span><span>${stars(stone.R)}</span><span class="catalog-rolls" title="Magnitude (main stat) · Base roll (secondary stat seed) · Shape ID · F is unconfirmed"><b>${rollLabels.M}</b> ${stone.M}<i>·</i><b>${rollLabels.B}</b> ${stone.B}<i>·</i><b>${rollLabels.Sh}</b> ${stone.Sh}<i>·</i><b>${rollLabels.F}</b> ${stone.F}</span><span class="catalog-expand">${expanded===index?'−':'＋'}</span></button>${expanded===index?`<div class="catalog-detail"><div class="catalog-detail-head"><b>BONUS STAT VALUES</b><span>${Object.keys(stone.SecS).length} stats · observed save</span></div><div class="catalog-stats">${Object.entries(stone.SecS).map(([key,value])=>`<div><span>${friendlyStatHtml(key)}</span><b>${fmt(value)}</b></div>`).join('')}</div><div class="replacement-status ${check.ok?'compatible':'incompatible'}">${esc(check.reason)}</div><button class="catalog-use" data-replace-observation="${index}" ${canReplace&&check.ok?'':'disabled'} title="${esc(canReplace?check.reason:'Earn a trinket first; there is no owned slot to replace.')}">Replace slot #${targetIndex??'—'} with this trinket</button></div>`:''}</div>`;}).join('')||'<div class="empty-inline">No observations match this search and set.</div>'}</div>
      <div class="lab-foot">OBSERVED STAT VALUES ARE EXAMPLES FROM REAL SAVES, NOT VERIFIED GAME LIMITS <span>SERVER INSTANCE IDS OMITTED</span></div></section></div>`;
}
function progressPage() {
  const p=progressProps(), challenge=state.save.flags;
  const groups=new Map(); for(const x of p){const g=x.scope||'GENERAL';groups.set(g,[...(groups.get(g)||[]),x]);}
  return `<div class="page-body">${pageIntro('Profile','Progress and challenges','Edit recognized progress counters and challenge flags. Unknown fields keep their saved names.')}<div class="warning-strip"><span>${icon('warn')}</span><div><b>Edit at your own risk</b><small>Progress and challenge values are written into the save as found. Unusual or unsupported values may affect save stability; keep a backup before saving.</small></div></div>
    <div class="progress-grid"><section class="panel progress-main"><div class="panel-head"><div><span class="section-code">Numeric fields</span><h2>Progress counters</h2></div><span class="tiny-label">${p.length} RECOGNIZED</span></div>${p.length?[...groups.entries()].map(([g,items])=>`<div class="property-group"><div class="group-title">${esc(g)} <span>${items.length} FIELDS</span></div>${items.map(propertyRow).join('')}</div>`).join(''):`<div class="empty-activity"><div class="empty-mark">${icon('clock')}</div><b>No named progress fields detected</b><span>Open Field Lab to inspect all recognized numeric fields.</span></div>`}</section>
    <section class="panel challenge-panel"><div class="panel-head"><div><span class="section-code">Challenges</span><h2>Challenge flags</h2></div></div>${challenge.length?challenge.map(f=>{const meta=FLAG_META[f.key]??{label:f.key,hint:''};return `<div class="flag-row"><div><b>${esc(meta.label)}</b><small>${esc(meta.hint)}${meta.hint?' · ':''}${f.count} entries${f.mixed?' · mixed values':''} <code>${esc(f.key)}</code></small></div><button class="toggle ${f.enabled?'on':''}" data-flag="${esc(f.key)}" aria-label="Toggle ${esc(meta.label)}"><i></i></button></div>`;}).join(''):'<div class="empty-inline">No supported challenge flag groups were found.</div>'}<div class="warning-note"><b>FIELD NOTE</b><span>These labels come from save property names. Their in-game meaning can vary by build.</span></div></section></div></div>`;
}
function propertyRow(p) { const value=p.value; const input=p.type_name==='BoolProperty'?`<button class="toggle ${Number(value)!==0?'on':''}" data-bool="${p.value_at}"><i></i></button>`:`<input class="property-input" data-value="${p.value_at}" value="${esc(value)}" type="number" required>`; return `<div class="property-row"><div><b>${esc(p.name)}</b><small>${esc(p.type_name)}${p.scope?' · '+esc(p.scope):''}</small></div>${input}</div>`; }
function labDisplay(p) {
  if (p.value === null || p.value === undefined) return 'No scalar value';
  if (['NameProperty','StrProperty'].includes(p.type_name) && p.value === '') return '(empty string)';
  if (p.type_name === 'ArrayProperty' && typeof p.value === 'object') return `${fmt(p.value.entries ?? 0)} entries · ${fmt(p.value.bytes ?? 0)} bytes`;
  if (['StructProperty','MapProperty'].includes(p.type_name) && typeof p.value === 'object') return `Structured data · ${fmt(p.value.bytes ?? 0)} bytes`;
  if (typeof p.value === 'object') return JSON.stringify(p.value);
  return String(p.value);
}
function labRow(p) {
  const control = ['NameProperty','StrProperty'].includes(p.type_name) ? `<span class="readout text-readout" title="${esc(labDisplay(p))}">${esc(labDisplay(p))}</span>` : p.type_name==='BoolProperty'
    ? `<button class="toggle ${Number(p.value)!==0?'on':''}" data-bool="${p.value_at}"><i></i></button>`
    : p.editable?`<input class="property-input lab-value" data-value="${p.value_at}" value="${esc(p.value)}" type="number" required>`
    :`<span class="readout">${esc(labDisplay(p))}</span>`;
  const editable = p.editable || p.type_name==='BoolProperty';
  const scope = p.scope.startsWith('Stone ') ? `Trinket ${p.scope.slice(6)}` : p.scope.startsWith('Unlock: ') ? `Intel / unlock · ${p.scope.slice(8)}` : p.scope.startsWith('Codex: ') ? `Codex · ${p.scope.slice(7)}` : p.scope || 'Player save';
  const field = p.name.replaceAll('_', ' ');
  return `<div class="lab-row"><div><b title="${esc(p.name)}">${esc(field)}</b><small>${esc(scope)}${p.note?' · '+esc(p.note):''}</small></div><code title="${esc(p.type_name)}">${esc(p.type_name.replace('Property',''))}</code><div>${control}</div><span class="mode ${editable?'mode-edit':'mode-read'}">${editable?'EDITABLE':'READ ONLY'}</span></div>`;
}
function labPage() {
  let list=props().filter(p=>(p.value_at!==null||['ArrayProperty','StructProperty','MapProperty'].includes(p.type_name))&&`${p.name} ${p.type_name} ${p.scope} ${p.note}`.toLowerCase().includes(state.search.toLowerCase()));
  const settings = list.filter(p => /^b[A-Z]/.test(p.name));
  const values = list.filter(p => !/^b[A-Z]/.test(p.name));
  return `<div class="page-body field-lab-page"><div class="risk-banner"><span>${icon('warn')}</span><div><b>Unsafe. Raw dump, not a readable view.</b><span>Every scalar field found anywhere in the save, mixed together by their internal serialized names — not organized for humans. Only touch something here if you already know exactly what that field does.</span></div></div>${pageIntro('Advanced inspection','Field lab','Browse every property the parser recognizes. Every boolean is toggleable; other scalar values with validated offsets can be edited.',`<label class="searchbox lab-search">${icon('search')}<input id="lab-search" placeholder="Search name, type, scope…" value="${esc(state.search)}"></label>`)}
    <section class="panel lab-panel"><div class="lab-section-title"><div><b>Settings and flags</b><small>Boolean and b-prefixed save options</small></div><span>${settings.length} entries · showing up to 300</span></div><div class="lab-table-head"><span>FIELD / SAVE SECTION</span><span>DATA TYPE</span><span>VALUE</span><span>EDIT</span></div><div class="lab-list">${settings.slice(0,300).map(labRow).join('')||'<div class="empty-inline">No matching settings or flags.</div>'}</div></section>
    <section class="panel lab-panel"><div class="lab-section-title"><div><b>Other save fields</b><small>Values grouped by their owning save section</small></div><span>${values.length} entries · showing up to 500</span></div><div class="lab-table-head"><span>FIELD / SAVE SECTION</span><span>DATA TYPE</span><span>VALUE</span><span>EDIT</span></div><div class="lab-list">${values.slice(0,500).map(labRow).join('')||'<div class="empty-inline">No matching fields.</div>'}</div><div class="lab-foot">${Math.min(values.length,500)} OF ${values.length} FIELDS <span>COMPLEX ARRAYS ARE KEPT INTACT</span></div></section></div>`;
}
function modalHtml() {
  if(state.modal?.type==='backups'){
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal backups-modal" role="dialog" aria-modal="true"><div class="modal-kicker">SAFETY</div><h2>Backups</h2><p>A backup is made automatically every time you save. You can also snapshot the file right now, and restore any backup if something goes wrong.</p><div class="backup-toolbar"><button class="button button-primary" data-action="create-backup">Create backup now</button><button class="button button-danger" data-action="clear-older-backups" ${state.backups.length>1?'':'disabled'}>Clear older backups</button></div><div class="backup-list">${state.backups.length?state.backups.map(b=>`<div class="backup-row"><div><b>${esc(b.label)}</b><small>${esc(b.modified_at)} · ${fmt(b.size)} bytes</small></div><div class="backup-actions"><button class="button button-quiet" data-action="restore-backup" data-filename="${esc(b.filename)}">Restore</button><button class="button button-danger" data-action="delete-backup" data-filename="${esc(b.filename)}" aria-label="Delete backup ${esc(b.label)}">Delete</button></div></div>`).join(''):'<div class="empty-inline">No backups yet for this save.</div>'}</div><div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Close</button></div><div class="modal-foot">Restoring first backs up whatever is currently on disk. “Clear older backups” keeps the newest backup and permanently deletes the rest.</div></section></div>`;
  }
  if(state.modal?.type==='edit'){
    const p=props().find(x=>x.value_at===state.modal.offset);
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal" role="dialog" aria-modal="true"><div class="modal-kicker">FIELD VALUE / ${esc(p?.type_name??'PROPERTY')}</div><h2>Edit ${esc(p?.name??'value')}</h2><p>${esc(p?.scope||'Global save field')} · offset 0x${Number(p?.value_at??0).toString(16).toUpperCase()}</p><label>New value<input id="edit-value" type="number" value="${esc(p?.value??'')}" required></label><div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Cancel</button><button class="button button-primary" data-action="confirm-edit">Stage value</button></div><div class="modal-foot">The value stays in memory until you save the edited file.</div></section></div>`;
  }
  if(state.modal?.type==='save-confirm'){
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal risk-modal" role="dialog" aria-modal="true"><div class="modal-kicker">SAVE FILE SAFETY</div><h2>Overwrite this save?</h2><p>This writes your staged changes over <b>${esc(state.save?.filename??'the current save')}</b>. The editor will create a timestamped backup beside the original first, but an invalid edit can still make the game reject the save.</p><div class="risk-banner risk-banner-compact"><span>${icon('warn')}</span><div><b>Check before continuing</b><span>Close the game first. Keep the backup until you have confirmed the edited save loads correctly.</span></div></div><div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Go back</button><button class="button button-danger" data-action="confirm-save">${icon('save')} Update Save</button></div></section></div>`;
  }
  if(state.modal?.type==='risk-add'){
    const obsIndex=state.modal.observation??0;
    const choice=stoneChoices[obsIndex];
    const stones=state.save.stones;
    const statKeys=Object.keys(choice?.SecS??{}).sort();
    const compatible=stones.filter(s=>JSON.stringify(s.secondary.map(([key])=>key).sort())===JSON.stringify(statKeys));
    const cloneDefault=compatible.some(s=>s.index===state.selectedStone)?state.selectedStone:compatible[0]?.index;
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal risk-modal" role="dialog" aria-modal="true"><div class="modal-kicker">HIGH-RISK INVENTORY CHANGE</div><h2>Create a trinket</h2><p>Creating a record changes the save structure and has a high chance of corrupting the save. Continue only if you accept that risk.</p><div class="risk-banner risk-banner-compact"><span>${icon('warn')}</span><div><b>A backup will be made first</b><span>All available observed values are copied. The existing item supplies only the compatible record structure and a new instance ID is generated.</span></div></div><label>Gathered trinket<select id="add-observation" required>${stoneChoices.map((c,i)=>`<option value="${i}" ${i===obsIndex?'selected':''}>${c.rarity.name} · ${esc(c.ID)} · ${esc(c.Set)} · Rank ${c.R}</option>`).join('')}</select></label><label>Compatible structure from<select id="add-clone-template" required ${compatible.length?'':'disabled'}>${compatible.map(s=>`<option value="${s.index}" ${s.index===cloneDefault?'selected':''}>Slot #${String(s.index).padStart(2,'0')} · ${esc(s.id)} (${esc(s.stone_set)})</option>`).join('')}</select></label>${compatible.length?'':'<div class="replacement-status incompatible">No owned trinket has the same bonus-stat key layout. This observation cannot be added safely to this save.</div>'}<div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Cancel</button><button class="button button-danger" data-action="confirm-add" ${compatible.length?'':'disabled'}>Back up and create trinket</button></div><div class="modal-foot">The game’s valid ranges are not fully known. Review the result in-game and keep the backup until it loads successfully.</div></section></div>`;
  }
  if(state.modal?.type==='risk-remove'){
    const stone=state.save?.stones.find(s=>s.index===state.modal.index);
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal risk-modal" role="dialog" aria-modal="true"><div class="modal-kicker">HIGH-RISK INVENTORY CHANGE</div><h2>Remove this trinket?</h2><p><b>${esc(stone?.id??'Selected trinket')}</b> (${esc(stone?.stone_set??'')}) will be removed from the save. Removing a record changes the save structure and can corrupt the save.</p><div class="risk-banner risk-banner-compact"><span>${icon('warn')}</span><div><b>A backup will be made first</b><span>Do not continue unless you accept the risk. Keep the backup until the edited save has loaded successfully in-game.</span></div></div><div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Keep trinket</button><button class="button button-danger" data-action="confirm-remove" data-index="${state.modal.index}">Back up and remove</button></div></section></div>`;
  }
  if(state.modal?.type==='replace-confirm'){
    const stone=state.save?.stones.find(s=>s.index===state.modal.targetIndex);
    const template=stoneCatalog[state.modal.observationIndex];
    return `<div class="modal-backdrop" data-action="dismiss"><section class="modal" role="dialog" aria-modal="true"><div class="modal-kicker">SAME-SIZE REPLACEMENT</div><h2>Replace slot #${state.modal.targetIndex}?</h2><p>${esc(stone?.id??'Current trinket')} will be overwritten with ${esc(template?.ID??'selected trinket')} (${esc(template?.Set??'')}). This changes values in the existing record without changing save size or slot count.</p><div class="modal-actions"><button class="button button-quiet" data-action="cancel-modal">Cancel</button><button class="button button-primary" data-action="confirm-replace">Replace existing item</button></div></section></div>`;
  }
  return '';
}

async function replaceTemplate(observationIndex) {
  const template=stoneCatalog[observationIndex];
  const targetIndex=state.templateTargetIndex??state.selectedStone??state.save.stones[0]?.index;
  const target=state.save.stones.find(s=>s.index===targetIndex);
  const check=replacementCompatibility(template,target);
  if(!check.ok){toast(check.reason,true);return;}
  state.modal={type:'replace-confirm',observationIndex,targetIndex};render();
}

function maxTrinketEdits(stone) {
  const scope=`Stone ${stone.index}`;
  const property=name=>props().find(p=>p.scope===scope&&p.name===name&&p.editable&&p.value_at!==null);
  const edits=[];
  for(const name of ['Lvl','R']){
    const p=property(name);
    if(p)edits.push({offset:p.value_at,value:6});
  }
  // Magnitude is stored in tenths of a percentage point (e.g. 200 = 20.0%).
  // Apply +7 percentage points for each rank gained, yielding +35 points from
  // rank 1 to rank 6. Base roll and all other trinket fields stay untouched.
  const rank=Math.max(1,Math.min(6,Number(stone.rank)||1));
  const magnitude=property('M');
  if(magnitude&&rank<6){
    const current=Number(magnitude.value);
    const boosted=Math.round(current+(6-rank)*70);
    if(Number.isSafeInteger(boosted)&&boosted>=-2147483648&&boosted<=2147483647){
      edits.push({offset:magnitude.value_at,value:boosted});
    }
  }
  return edits;
}

// Raise each trinket's rarity suffix to Legendary (_L). Trinkets without a
// recognized rarity suffix (e.g. AlienWeapon) or already Legendary are skipped.
async function raiseTrinketsToLegendary(stones) {
  let changed=0;
  for(const stone of stones){
    const letter=rarityLetterOf(stone.id);
    if(!letter||letter==='L')continue;
    try{state.save=await invoke('set_trinket_rarity',{index:stone.index,letter:'L'});changed++;}
    catch{/* leave trinkets that can't be converted untouched */}
  }
  if(changed)render();
  return changed;
}

function bind() {
  document.querySelectorAll('[data-page]').forEach(b=>b.onclick=()=>{state.page=b.dataset.page;state.search='';render();});
  document.querySelectorAll('[data-action]').forEach(b=>b.onclick=async e=>{
    if(b.dataset.action==='dismiss'&&e.target!==b)return;
    const a=b.dataset.action;
    if(a==='open') await act('open_save',{},'Save loaded.');
    if(a==='import-stone-save'){
      try{
        const incoming=await invoke('import_stone_catalog');
        const before=stoneCatalog.length;stoneCatalog=mergeObservations(stoneCatalog,incoming);refreshStoneCatalogChoices();
        const persisted=persistSupplementalObservations();render();
        toast(`${stoneCatalog.length-before} new trinket observation(s) imported${persisted?' and saved locally':'; local storage is full'}.`);
      }catch(err){toast(String(err),true);}
    }
    if(a==='import-exported-trinket'){
      try{
        const incoming=await invoke('import_exported_trinket');
        const before=stoneCatalog.length;stoneCatalog=mergeObservations(stoneCatalog,[incoming]);refreshStoneCatalogChoices();
        const persisted=persistSupplementalObservations();render();
        toast(`${stoneCatalog.length-before?'Trinket imported into the local Workshop catalog':'That trinket is already in the catalog'}${persisted?' and saved locally':'; local storage is full'}.`);
      }catch(err){toast(String(err),true);}
    }
    if(a==='save'){await flushStage();state.modal={type:'save-confirm'};render();}
    if(a==='confirm-save'){await flushStage();state.modal=null;await act('save_changes',{},'Saved. Backup created beside the original.');}
    if(a==='undo') await act('undo',{},'Unsaved edits reverted.');
    if(a==='open-backups'){try{state.backups=await invoke('list_backups');}catch(err){toast(String(err),true);state.backups=[];}state.modal={type:'backups'};render();}
    if(a==='create-backup'){try{await invoke('create_backup');state.backups=await invoke('list_backups');toast('Backup created.');render();}catch(err){toast(String(err),true);}}
    if(a==='delete-backup'){const filename=b.dataset.filename;if(!filename||!confirm(`Permanently delete backup "${filename}"?`))return;try{await invoke('delete_backup',{filename});state.backups=await invoke('list_backups');render();toast('Backup deleted.');}catch(err){toast(String(err),true);}}
    if(a==='clear-older-backups'){if(state.backups.length<2)return;if(!confirm(`Permanently delete ${state.backups.length-1} older backup(s)? The newest backup will be kept.`))return;try{const count=await invoke('clear_older_backups');state.backups=await invoke('list_backups');render();toast(`${count} older backup${count===1?'':'s'} deleted. The newest backup was kept.`);}catch(err){toast(String(err),true);}}
    if(a==='restore-backup'){const filename=b.dataset.filename;if(!confirm(`Restore "${filename}"? The current file on disk will be backed up first.`))return;try{state.save=await invoke('restore_backup',{filename});state.modal=null;toast('Backup restored.');render();}catch(err){toast(String(err),true);}}
    if(a==='max-trinket'){
      const stone=state.save?.stones.find(s=>s.index===Number(b.dataset.index));
      if(!stone){toast('Select a trinket first.',true);return;}
      const edits=maxTrinketEdits(stone);
      if(!edits.length){toast('This trinket has no editable level or rank fields.',true);return;}
      stage(edits);await flushStage();
      const raised=await raiseTrinketsToLegendary([stone]);
      toast(`Trinket set to Level 6 and Rank 6; Magnitude increased by 7 percentage points per rank gained.${raised?' Rarity set to Legendary.':''}`);
    }
    if(a==='max-all-trinkets'){
      const stones=state.save?.stones??[];
      if(!stones.length){toast('No owned trinkets to update.',true);return;}
      const edits=stones.flatMap(maxTrinketEdits);
      if(!edits.length){toast('No editable trinket fields were found.',true);return;}
      stage(edits);await flushStage();
      const raised=await raiseTrinketsToLegendary(stones);
      toast(`Set ${stones.length} trinket(s) to Level 6 and Rank 6; Magnitude increased by 7 percentage points per rank gained.${raised?` ${raised} raised to Legendary.`:''} Base roll and all other fields were left unchanged.`);
    }
    if(a==='export-trinket'){try{await invoke('export_trinket',{index:Number(b.dataset.index)});toast('Trinket exported.');}catch(err){toast(String(err),true);}}
    if(a==='replace-observed'){const index=Number(b.dataset.observation);if(Number.isInteger(index))await replaceTemplate(index);}
    if(a==='open-add'){if(!state.save?.stones.length){toast('Earn at least one trinket first; this save has no record to clone.',true);return;}const selectedChoice=stoneChoices.findIndex(c=>c.index===state.expandedObservation);state.modal={type:'risk-add',observation:selectedChoice>=0?selectedChoice:0};render();}
    if(a==='open-remove'){const index=Number(b.dataset.index);if(!Number.isInteger(index)||state.save.stones.length<=1){toast('At least one trinket must remain.',true);return;}state.modal={type:'risk-remove',index};render();}
    if(a==='cancel-modal'||a==='dismiss'){state.modal=null;render();}
    if(a==='confirm-edit'){
      const p=props().find(x=>x.value_at===state.modal?.offset);const input=document.querySelector('#edit-value');const raw=input?.value?.trim()??'';const value=Number(raw);
      if(!p||raw===''||!Number.isSafeInteger(value)){toast('Enter a valid whole number.',true);if(input)input.value=p?.value??'';return;}
      if(p.scope==='Currencies'&&value<1){toast('Currency values must be at least 1.',true);input.value=p.value;return;}
      state.modal=null;await stage([{offset:p.value_at,value}]);
    }
    if(a==='confirm-replace'){
      const {observationIndex,targetIndex}=state.modal??{};const template=stoneCatalog[observationIndex];
      if(!template||!state.save.stones.some(s=>s.index===targetIndex)){toast('The selected trinket slot or template is no longer available.',true);return;}
      try{state.save=await invoke('replace_stone',{index:targetIndex,template});state.selectedStone=targetIndex;state.modal=null;toast('Existing trinket slot replaced in memory.');render();}catch(err){toast(String(err),true);}
    }
    if(a==='confirm-add'){
      const observationIndex=Number(document.querySelector('#add-observation')?.value);const choice=stoneChoices[observationIndex];const templateIndex=Number(document.querySelector('#add-clone-template')?.value);
      if(!choice||!state.save.stones.some(s=>s.index===templateIndex)){toast('Choose a gathered observation and a matching structure template.',true);return;}
      const observation={ID:choice.ID,Set:choice.Set,Lvl:choice.Lvl,R:choice.R,Sh:choice.Sh,B:choice.B,M:choice.M,SecS:choice.SecS,F:choice.F??null};
      try{await invoke('create_backup');state.save=await invoke('add_stone',{templateIndex,observation});state.selectedStone=state.save.stones.at(-1)?.index??null;state.page='stones';state.modal=null;toast('Gathered trinket created in memory. Backup created first.');render();}catch(err){toast(String(err),true);}
    }
    if(a==='confirm-remove'){
      const index=Number(b.dataset.index);
      if(state.save.stones.length<=1){toast('At least one trinket must remain.',true);return;}
      try{await invoke('create_backup');state.save=await invoke('remove_stone',{index});state.selectedStone=state.save.stones[Math.min(index-1,state.save.stones.length-1)]?.index??null;state.page='stones';state.modal=null;toast('Trinket removed in memory. Backup created first.');render();}catch(err){toast(String(err),true);}
    }
  });
  document.querySelectorAll('[data-stone]').forEach(b=>b.onclick=()=>{state.selectedStone=Number(b.dataset.stone);render();});
  document.querySelectorAll('[data-observation]').forEach(b=>b.onclick=()=>{const index=Number(b.dataset.observation);state.expandedObservation=state.expandedObservation===index?null:index;render();});
  document.querySelector('#catalog-set')?.addEventListener('change',e=>{state.catalogSet=e.target.value;render();});
  document.querySelector('#add-observation')?.addEventListener('change',e=>{state.modal.observation=Number(e.target.value);render();});
  document.querySelector('#template-target')?.addEventListener('change',e=>{state.templateTargetIndex=Number(e.target.value);state.selectedStone=state.templateTargetIndex;render();});
  document.querySelectorAll('[data-replace-observation]').forEach(b=>b.onclick=()=>replaceTemplate(Number(b.dataset.replaceObservation)));
  document.querySelectorAll('[data-unlocks-tab]').forEach(b=>b.onclick=()=>{state.unlocksTab=b.dataset.unlocksTab;state.search='';if(state.unlocksTab==='advanced'){state.advancedClosed.clear();state.advancedOpen=null;}render();});
  document.querySelector('[data-rarity-index]')?.addEventListener('change', async e=>{const index=Number(e.target.dataset.rarityIndex),letter=e.target.value;try{state.save=await invoke('set_trinket_rarity',{index,letter});toast('Rarity updated.');render();}catch(err){toast(String(err),true);}});
  document.querySelectorAll('[data-advanced-toggle]').forEach(b=>b.onclick=()=>{const key=b.dataset.advancedToggle;if(state.advancedOpen===key){state.advancedOpen=null;state.advancedClosed.add(key);}else{state.advancedOpen=key;state.advancedClosed.delete(key);}render();});
  document.querySelectorAll('[data-step]').forEach(b=>b.onclick=()=>{const offset=Number(b.dataset.step);const input=document.querySelector(`[data-value="${offset}"]`);if(!input)return;const prop=props().find(p=>p.value_at===offset);const raw=input.value.trim();if(raw===''){input.value=prop?.value??input.defaultValue;toast('A value is required.',true);return;}const current=Number(raw);if(!Number.isSafeInteger(current)){input.value=prop?.value??input.defaultValue;toast('Enter a whole number.',true);return;}const min=input.min===''?Number.MIN_SAFE_INTEGER:Number(input.min);const max=input.max===''?Number.MAX_SAFE_INTEGER:Number(input.max);const value=Math.max(min,Math.min(max,current+Number(b.dataset.delta)));input.value=String(value);stage([{offset,value}]);});
  document.querySelectorAll('[data-value]').forEach(input=>input.onchange=()=>{const offset=Number(input.dataset.value);const prop=props().find(p=>p.value_at===offset);if(!prop)return;const raw=input.value.trim();const value=Number(raw);const min=input.min===''?Number.MIN_SAFE_INTEGER:Number(input.min);const max=input.max===''?Number.MAX_SAFE_INTEGER:Number(input.max);if(raw===''||!Number.isSafeInteger(value)||value<min||value>max){toast('Enter a valid whole number within the allowed range.',true);input.value=prop.value;return;}if(prop.scope==='Currencies'&&value<1){toast('Currency values must be at least 1.',true);input.value=prop.value;return;}stage([{offset,value}]);});
  document.querySelectorAll('[data-bool]').forEach(b=>b.onclick=()=>{const offset=Number(b.dataset.bool);const p=props().find(x=>x.value_at===offset);if(!p)return;const next=Number(p.value)===0;stage([{offset,value:next}]);});
  document.querySelectorAll('[data-flag]').forEach(b=>b.onclick=()=>{const f=state.save.flags.find(x=>x.key===b.dataset.flag);if(!f)return;const edits=f.offsets.map(offset=>({offset,value:!f.enabled}));stage(edits);});
  for(const id of ['stone-search','lab-search','catalog-search','unlocks-search']){const input=document.querySelector(`#${id}`);if(input)input.oninput=()=>{const pos=input.selectionStart;state.search=input.value;render();const again=document.querySelector(`#${id}`);again?.focus();again?.setSelectionRange(pos,pos);};}
  document.querySelector('#edit-value')?.focus();
}
let editBusy=false;
let stageTimer=null;
const pendingEdits=new Map();
const stageWaiters=[];
function stage(edits){
  for(const edit of edits){
    const prop=edit.tag_at!=null&&edit.type_name?edit:props().find(p=>p.value_at===edit.offset&&p.editable&&p.tag_at!=null);
    if(!prop){toast(`Could not match the saved property at offset 0x${Number(edit.offset).toString(16).toUpperCase()}.`,true);continue;}
    pendingEdits.set(edit.offset,{offset:edit.offset,tag_at:prop.tag_at,type_name:prop.type_name,value:edit.value});
  }
  if(!pendingEdits.size)return;
  clearTimeout(stageTimer);
  stageTimer=setTimeout(()=>flushStage(),110);
}
async function flushStage(){
  clearTimeout(stageTimer);stageTimer=null;
  if(editBusy)return new Promise(resolve=>stageWaiters.push(resolve));
  if(!pendingEdits.size)return;
  editBusy=true;
  try{
    while(pendingEdits.size){
      const edits=[...pendingEdits.values()];pendingEdits.clear();
      state.save=await invoke('stage_edits',{edits});
    }
  }catch(e){toast(String(e),true);}
  finally{
    editBusy=false;render();
    while(stageWaiters.length)stageWaiters.shift()();
  }
}
invoke('current_snapshot').then(s=>{state.save=s;render();}).catch(()=>render());

let dropBusy=false;
window.__TAURI__.webview.getCurrentWebview().onDragDropEvent(async event => {
  const p = event.payload;
  document.body.classList.toggle('drag-over', p.type === 'over');
  if (p.type !== 'drop' || dropBusy) return;
  document.body.classList.remove('drag-over');
  const path = p.paths?.[0];
  if (!path) return;
  dropBusy = true;
  try {
    if (path.toLowerCase().endsWith('.sav')) {
      state.save = await invoke('open_save_path', { path });
      toast('Save loaded.'); render();
    } else {
      toast('Drop a .sav save file to open it.', true);
    }
  } catch (e) { toast(String(e), true); }
  finally { dropBusy = false; }
});
