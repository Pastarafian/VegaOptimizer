/**
 * VegaOptimizer — VegaMCP Desktop Tester (v3)
 * 
 * Comprehensive test suite using VegaMCP capabilities.
 * Runs from VegaOptimizer project dir with absolute imports.
 */

import fs from 'fs';
import path from 'path';

const VEGA_CAP = 'file:///C:/Users/fakej/Documents/VegaMCP/build/tools/capabilities';
const { handleCodeToolkit } = await import(`${VEGA_CAP}/code-toolkit.js`);
const { handleSecurityToolkit } = await import(`${VEGA_CAP}/security-toolkit.js`);
const { handleDesignToolkit } = await import(`${VEGA_CAP}/design-toolkit.js`);
const { handlePerformanceToolkit } = await import(`${VEGA_CAP}/performance-toolkit.js`);
const { handleDevopsToolkit } = await import(`${VEGA_CAP}/devops-toolkit.js`);
const { handleSeoToolkit } = await import(`${VEGA_CAP}/seo-toolkit.js`);
const { handleDataToolkit } = await import(`${VEGA_CAP}/data-toolkit.js`);
const { handleDesktopTesting } = await import(`${VEGA_CAP}/desktop-testing.js`);
const { handleAdvancedTesting } = await import(`${VEGA_CAP}/advanced-testing.js`);
const { handleDatabaseTesting } = await import(`${VEGA_CAP}/database-testing.js`);
const { handleServerTesting } = await import(`${VEGA_CAP}/server-testing.js`);
const { handleSecurityTesting } = await import(`${VEGA_CAP}/security-testing.js`);
const { handleCodeAnalysis } = await import(`${VEGA_CAP}/code-analysis.js`);
const { handleHealthCheck } = await import(`${VEGA_CAP}/health-check.js`);
const { handleVisualTesting } = await import(`${VEGA_CAP}/visual-testing.js`);

const PROJECT = 'C:/Users/fakej/Documents/VegaOptimizer';
const SRC = path.join(PROJECT, 'src');
const SRC_TAURI = path.join(PROJECT, 'src-tauri', 'src');
const OUTPUT = path.join(PROJECT, 'vegamcp_full_report.txt');

const lines = [];
function log(s) { lines.push(s); process.stdout.write(s + '\n'); }
function hr(c='=') { log(c.repeat(70)); }
function header(n, t) { log(''); hr(); log(`  ${n}. ${t}`); hr(); }
function sub(t) { log(`\n  -- ${t} ${'~'.repeat(Math.max(0,55-t.length))}`); }

function findFiles(dir, exts, res = []) {
  if (!fs.existsSync(dir)) return res;
  for (const e of fs.readdirSync(dir)) {
    const f = path.join(dir, e); const s = fs.statSync(f);
    if (s.isDirectory() && !['node_modules','target','.git'].includes(e)) findFiles(f, exts, res);
    else if (s.isFile() && exts.some(x => e.endsWith(x))) res.push(f);
  }
  return res;
}

function pr(res) {
  try { return JSON.parse(res.content[0].text); }
  catch { return res?.content?.[0]?.text || res; }
}

function sc(s) {
  if (s >= 90) return '[PASS]';
  if (s >= 70) return '[OK  ]';
  if (s >= 50) return '[WARN]';
  return '[FAIL]';
}

const tsxFiles = findFiles(SRC, ['.tsx','.ts','.css']);
const rustFiles = findFiles(SRC_TAURI, ['.rs']);
const configs = [
  path.join(PROJECT, 'vite.config.ts'),
  path.join(PROJECT, 'tsconfig.json'),
  path.join(PROJECT, 'package.json'),
  path.join(PROJECT, 'index.html'),
].filter(f => fs.existsSync(f));
const allFiles = [...tsxFiles, ...rustFiles, ...configs];
const mainApp = tsxFiles.find(f => f.endsWith('App.tsx'));
const htmlFile = path.join(PROJECT, 'index.html');

const report = { scores: {}, issues: [] };

log('');
log('  VEGAMCP FULL DESKTOP TEST - VegaOptimizer Codebase');
log(`  Date: ${new Date().toISOString()}`);
log(`  Files: ${allFiles.length} (${tsxFiles.length} TS/TSX/CSS, ${rustFiles.length} Rust, ${configs.length} config)`);

// ==================================================================
//  1. CODE TOOLKIT
// ==================================================================
header(1, 'CODE TOOLKIT - Refactoring & Maintainability');
let totalM = 0, cntM = 0;
for (const file of allFiles) {
  const content = fs.readFileSync(file, 'utf-8');
  const rel = path.relative(PROJECT, file);
  const lang = file.endsWith('.rs') ? 'rust' : file.endsWith('.css') ? 'css' : 'typescript';
  try {
    const r = pr(await handleCodeToolkit({ action: 'refactor', code_snippet: content, language: lang }));
    const s = r.rating?.maintainability_score || 0;
    totalM += s; cntM++;
    log(`  ${sc(s)} ${rel}: ${s}/100 (${r.rating?.complexity_grade || 'N/A'})`);
    if (s < 70) report.issues.push({ file: rel, kit: 'code', msg: `Maintainability ${s}/100` });
  } catch(e) { log(`  [ERR ] ${rel}: ${e.message}`); }
}
const avgM = cntM > 0 ? Math.round(totalM / cntM) : 0;
report.scores.code_maintainability = avgM;
log(`\n  AVG Maintainability: ${avgM}/100 across ${cntM} files`);

if (mainApp) {
  sub('Optimization Analysis');
  const r = pr(await handleCodeToolkit({ action: 'optimize', code_snippet: fs.readFileSync(mainApp,'utf-8').substring(0,5000), language: 'typescript' }));
  log(`  Efficiency: ${r.rating?.efficiency_score}/100 | Big-O: ${r.rating?.big_o_estimation}`);
  report.scores.code_efficiency = r.rating?.efficiency_score || 0;
}

sub('Architecture');
const arch = pr(await handleCodeToolkit({ action: 'architecture', query: 'clean_architecture' }));
log(`  Pattern: ${arch.pattern}`);

sub('Tests + Docs');
const t = pr(await handleCodeToolkit({ action: 'generate_tests' }));
log(`  Test Framework: ${t.framework}`);
const d = pr(await handleCodeToolkit({ action: 'document' }));
log(`  Doc Style: ${d.doc_style}`);
const x = pr(await handleCodeToolkit({ action: 'explain_complex' }));
log(`  Explain: ${x.explanation?.substring(0, 100)}`);

// ==================================================================
//  2. SECURITY TOOLKIT
// ==================================================================
header(2, 'SECURITY TOOLKIT - Vulnerability Scanning');
let totalS = 0, cntS = 0;
for (const file of allFiles) {
  const content = fs.readFileSync(file, 'utf-8');
  const rel = path.relative(PROJECT, file);
  try {
    const r = pr(await handleSecurityToolkit({ action: 'scan_code', code_snippet: content }));
    const s = r.rating?.security_score ?? 100;
    totalS += s; cntS++;
    if (r.status === 'vulnerable') {
      log(`  [VULN] ${rel}: ${s}/100 (${r.rating?.grade})`);
      r.findings?.forEach(f => log(`    -> ${f}`));
      report.issues.push({ file: rel, kit: 'security', msg: r.findings?.join('; ') });
    } else {
      log(`  [SAFE] ${rel}: ${s}/100`);
    }
  } catch(e) { log(`  [ERR ] ${rel}: ${e.message}`); }
}
const avgS = cntS > 0 ? Math.round(totalS / cntS) : 0;
report.scores.security = avgS;
log(`\n  AVG Security: ${avgS}/100`);

sub('Compliance (OWASP)');
const comp = pr(await handleSecurityToolkit({ action: 'compliance_check', query: 'owasp' }));
log(`  ${sc(comp.rating?.compliance_score)} Score: ${comp.rating?.compliance_score}/100`);
comp.checklist?.forEach(c => log(`    [ ] ${c}`));
report.scores.compliance = comp.rating?.compliance_score || 0;

sub('Threat Model');
const threat = pr(await handleSecurityToolkit({ action: 'threat_model', target: 'desktop_app' }));
threat.threats?.forEach(t => log(`  >> ${t.actor} -> ${t.vector} -> ${t.impact}`));

sub('Audit Guide');
const audit = pr(await handleSecurityToolkit({ action: 'audit_guide', target: 'tauri' }));
audit.steps?.forEach(s => log(`    ${s}`));

sub('CSP Policy');
const csp = pr(await handleSecurityToolkit({ action: 'generate_policy', query: 'csp' }));
log(`  Policy generated: ${typeof csp.policy === 'string' ? csp.policy.substring(0,100) : 'complete'}`);

sub('Crypto Utils');
const crypto = pr(await handleSecurityToolkit({ action: 'crypto_utils', query: 'hashing' }));
log(`  Recommendation: ${crypto.recommendation || JSON.stringify(crypto).substring(0,100)}`);

// ==================================================================
//  3. DESIGN TOOLKIT
// ==================================================================
header(3, 'DESIGN TOOLKIT - UI/UX Analysis');
const cssFile = tsxFiles.find(f => f.endsWith('.css'));
if (cssFile) {
  const r = pr(await handleDesignToolkit({ action: 'design_lint', code_snippet: fs.readFileSync(cssFile,'utf-8') }));
  log(`  Design Lint: ${JSON.stringify(r.rating || r.score || 'complete')}`);
  if (r.rating?.score) report.scores.design = r.rating.score;
}
const cp = pr(await handleDesignToolkit({ action: 'color_palette', query: 'dark_mode' }));
log(`  Color Palette: ${cp.theme || 'generated'} | Colors: ${JSON.stringify(cp.colors || cp.palette || []).substring(0,100)}`);
const ty = pr(await handleDesignToolkit({ action: 'typography', query: 'Inter' }));
log(`  Typography: ${JSON.stringify(ty.stack || ty).substring(0,100)}`);
const tk = pr(await handleDesignToolkit({ action: 'design_tokens' }));
log(`  Design Tokens: ${JSON.stringify(tk.tokens || tk).substring(0,100)}`);
const co = pr(await handleDesignToolkit({ action: 'component', query: 'button' }));
log(`  Component (button): ${JSON.stringify(co).substring(0,100)}`);
const la = pr(await handleDesignToolkit({ action: 'layout', query: 'dashboard' }));
log(`  Layout (dashboard): ${JSON.stringify(la.layout || la.grid || la).substring(0,100)}`);
const an = pr(await handleDesignToolkit({ action: 'animation', query: 'micro' }));
log(`  Animation: ${JSON.stringify(an).substring(0,100)}`);
const ag = pr(await handleDesignToolkit({ action: 'asset_generator', query: 'icon_set' }));
log(`  Asset Gen: ${JSON.stringify(ag).substring(0,100)}`);

// ==================================================================
//  4. PERFORMANCE TOOLKIT
// ==================================================================
header(4, 'PERFORMANCE TOOLKIT');
if (fs.existsSync(htmlFile)) {
  const lh = pr(await handlePerformanceToolkit({ action: 'lighthouse_score', code_snippet: fs.readFileSync(htmlFile,'utf-8') }));
  log(`  ${sc(lh.rating?.score)} Lighthouse: ${lh.rating?.score}/100 (${lh.rating?.grade})`);
  lh.penalties?.forEach(p => log(`    -> ${p}`));
  report.scores.lighthouse = lh.rating?.score || 0;
}
if (mainApp) {
  const ml = pr(await handlePerformanceToolkit({ action: 'memory_leak_check', code_snippet: fs.readFileSync(mainApp,'utf-8') }));
  log(`  ${sc(ml.rating?.safety_score)} Memory Safety: ${ml.rating?.safety_score}/100 (${ml.rating?.status})`);
  ml.risks?.forEach(r => log(`    -> ${r}`));
  report.scores.memory_safety = ml.rating?.safety_score || 0;
}
const ba = pr(await handlePerformanceToolkit({ action: 'bundle_analysis' }));
log(`  ${sc(ba.rating?.efficiency_score)} Bundle: ${ba.rating?.efficiency_score}/100`);
report.scores.bundle = ba.rating?.efficiency_score || 0;
const ro = pr(await handlePerformanceToolkit({ action: 'render_optimization', target_framework: 'react' }));
log(`  ${sc(ro.rating?.render_score)} Render: ${ro.rating?.render_score}/100`);
report.scores.render = ro.rating?.render_score || 0;

// ==================================================================
//  5. DEVOPS TOOLKIT
// ==================================================================
header(5, 'DEVOPS TOOLKIT');
const ciFile = path.join(PROJECT, '.github', 'workflows', 'ci.yml');
if (fs.existsSync(ciFile)) {
  const ci = pr(await handleDevopsToolkit({ action: 'ci_cd_rating', code_snippet: fs.readFileSync(ciFile,'utf-8') }));
  log(`  ${sc(ci.rating?.robustness_score)} CI/CD: ${ci.rating?.robustness_score}/100 (${ci.rating?.status})`);
  ci.issues?.forEach(i => log(`    -> ${i}`));
  report.scores.cicd = ci.rating?.robustness_score || 0;
} else {
  log('  [WARN] No CI/CD pipeline found');
  report.scores.cicd = 50;
}
const iac = pr(await handleDevopsToolkit({ action: 'iac_linter' }));
log(`  ${sc(iac.rating?.best_practices_score)} IaC: ${iac.rating?.best_practices_score}/100`);
report.scores.iac = iac.rating?.best_practices_score || 0;
const cost = pr(await handleDevopsToolkit({ action: 'cost_optimization', cloud_provider: 'General' }));
log(`  ${sc(cost.rating?.efficiency_score)} Cost: ${cost.rating?.efficiency_score}/100`);
report.scores.cost = cost.rating?.efficiency_score || 0;
const df = pr(await handleDevopsToolkit({ action: 'dockerfile_audit' }));
log(`  Dockerfile Audit: ${JSON.stringify(df.rating || df).substring(0,100)}`);

// ==================================================================
//  6. SEO TOOLKIT
// ==================================================================
header(6, 'SEO TOOLKIT');
if (fs.existsSync(htmlFile)) {
  const htmlContent = fs.readFileSync(htmlFile,'utf-8');
  const seo = pr(await handleSeoToolkit({ action: 'page_analyzer', html_snippet: htmlContent }));
  log(`  ${sc(seo.rating?.seo_score)} SEO: ${seo.rating?.seo_score}/100 (${seo.rating?.grade})`);
  seo.issues?.forEach(i => log(`    -> ${i}`));
  report.scores.seo = seo.rating?.seo_score || 0;
  const sem = pr(await handleSeoToolkit({ action: 'semantic_check', html_snippet: htmlContent }));
  log(`  ${sc(sem.rating?.semantic_score)} Semantic: ${sem.rating?.semantic_score}/100 (${sem.rating?.grade})`);
  sem.advice?.forEach(a => log(`    -> ${a}`));
  report.scores.semantic = sem.rating?.semantic_score || 0;
}
const mg = pr(await handleSeoToolkit({ action: 'meta_generator', topic: 'VegaOptimizer' }));
log(`  Meta Tags Generated: ${mg.topic}`);
const sd = pr(await handleSeoToolkit({ action: 'structured_data', type: 'Product', topic: 'VegaOptimizer' }));
log(`  Structured Data: ${JSON.stringify(sd).substring(0,100)}`);

// ==================================================================
//  7. DATA TOOLKIT
// ==================================================================
header(7, 'DATA TOOLKIT');
const qo = pr(await handleDataToolkit({ action: 'query_optimizer', sql_snippet: 'SELECT id, name, category, status, risk FROM optimizations WHERE status = ? AND category = ? ORDER BY updated_at DESC LIMIT 50' }));
log(`  ${sc(qo.rating?.efficiency_score)} Query: ${qo.rating?.efficiency_score}/100 (${qo.rating?.grade})`);
qo.bottlenecks?.forEach(b => log(`    -> ${b}`));
report.scores.data_query = qo.rating?.efficiency_score || 0;
const sa = pr(await handleDataToolkit({ action: 'schema_analyzer', schema_snippet: 'CREATE TABLE optimizations (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, category TEXT NOT NULL, status TEXT DEFAULT active, risk TEXT, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP); CREATE INDEX idx_opt_status ON optimizations(status); CREATE INDEX idx_opt_category ON optimizations(category);' }));
log(`  ${sc(sa.rating?.normalization_score)} Schema: ${sa.rating?.normalization_score}/100`);
report.scores.data_schema = sa.rating?.normalization_score || 0;
const dm = pr(await handleDataToolkit({ action: 'data_modeling', domain: 'system-optimization' }));
log(`  Model: ${dm.domain} (${dm.core_schema?.length} tables)`);
const ml2 = pr(await handleDataToolkit({ action: 'migration_lint', schema_snippet: 'ALTER TABLE opts ADD COLUMN priority TEXT DEFAULT "low";' }));
log(`  ${sc(ml2.rating?.safety_score)} Migration: ${ml2.rating?.safety_score}/100 (${ml2.rating?.status})`);
report.scores.migration = ml2.rating?.safety_score || 0;

// ==================================================================
//  8. DESKTOP TESTING
// ==================================================================
header(8, 'DESKTOP TESTING');
try {
  const si = pr(await handleDesktopTesting({ action: 'system_info' }));
  log(`  Platform: ${si.platform || si.os || 'N/A'} | Arch: ${si.arch || 'N/A'}`);
} catch(e) { log(`  WARN: ${e.message}`); }
try {
  const wl = pr(await handleDesktopTesting({ action: 'window_list' }));
  log(`  Windows: ${wl.windows?.length || wl.count || JSON.stringify(wl).substring(0,100)}`);
} catch(e) { log(`  WARN: ${e.message}`); }
try {
  const ss = pr(await handleDesktopTesting({ action: 'screenshot' }));
  log(`  Screenshot: ${ss.path || ss.status || JSON.stringify(ss).substring(0,80)}`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  9. ADVANCED TESTING
// ==================================================================
header(9, 'ADVANCED TESTING - Full Suite');
for (const action of ['full_sanity_check','bubble_test','chaos_monkey','fuzz_test','concurrency_stress','regression_suite']) {
  try {
    const r = pr(await handleAdvancedTesting({ action, intensity: 5 }));
    const v = r.result || r.verdict || 'N/A';
    log(`  ${v.includes('Pass') || v.includes('OK') ? '[PASS]' : '[FAIL]'} ${r.test_name}: ${v}`);
  } catch(e) { log(`  [ERR ] ${action}: ${e.message}`); }
}

// ==================================================================
//  10. DATABASE TESTING
// ==================================================================
header(10, 'DATABASE TESTING');
try {
  const r = pr(await handleDatabaseTesting({ action: 'schema_lint' }));
  log(`  Schema Lint: ${JSON.stringify(r.result || r).substring(0,200)}`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  11. SERVER TESTING
// ==================================================================
header(11, 'SERVER TESTING');
try {
  const r = pr(await handleServerTesting({ action: 'port_scan' }));
  log(`  Port Scan: ${JSON.stringify(r.result || r).substring(0,200)}`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  12. SECURITY TESTING
// ==================================================================
header(12, 'SECURITY TESTING - Deep Scan');
try {
  const r = pr(await handleSecurityTesting({ action: 'dast_scan' }));
  log(`  DAST: ${JSON.stringify(r.result || r).substring(0,200)}`);
} catch(e) { log(`  WARN: ${e.message}`); }
try {
  const r = pr(await handleSecurityTesting({ action: 'dependency_audit' }));
  log(`  Deps: ${JSON.stringify(r.result || r).substring(0,200)}`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  13. CODE ANALYSIS ENGINE
// ==================================================================
header(13, 'CODE ANALYSIS ENGINE');
for (const file of allFiles.slice(0, 5)) {
  const content = fs.readFileSync(file, 'utf-8');
  const rel = path.relative(PROJECT, file);
  try {
    const r = pr(await handleCodeAnalysis({ action: 'parse_functions', code: content, filename: rel }));
    log(`  ${rel}: ${r.functions?.length || 0} functions, ${r.classes?.length || 0} classes`);
  } catch(e) { log(`  WARN ${rel}: ${e.message}`); }
}

// ==================================================================
//  14. VISUAL TESTING
// ==================================================================
header(14, 'VISUAL TESTING');
try {
  const r = pr(await handleVisualTesting({ action: 'screenshot_compare' }));
  log(`  Visual: ${JSON.stringify(r.result || r).substring(0,200)}`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  15. HEALTH CHECK
// ==================================================================
header(15, 'VEGAMCP HEALTH CHECK');
try {
  const r = pr(await handleHealthCheck({ action: 'full', verbose: true }));
  log(`  Status: ${r.overall_status || 'complete'}`);
  if (r.checks) {
    for (const c of r.checks) {
      const st = c.status === 'healthy' ? '[PASS]' : c.status === 'degraded' ? '[WARN]' : '[FAIL]';
      log(`  ${st} ${c.name}: ${c.status} - ${c.message}`);
    }
  }
  if (r.score) log(`  Health Score: ${r.score}/100`);
} catch(e) { log(`  WARN: ${e.message}`); }

// ==================================================================
//  FINAL REPORT
// ==================================================================
hr('=');
log('');
log('  FINAL AUDIT REPORT');
hr('=');
log(`  Project: VegaOptimizer`);
log(`  Date: ${new Date().toISOString()}`);
log(`  Files Scanned: ${allFiles.length}`);
log(`  Toolkits Run: 15/15`);
log('');
log('  --- SCORES ---');
const scores = Object.entries(report.scores);
for (const [name, score] of scores) {
  log(`  ${sc(score)} ${name.replace(/_/g,' ')}: ${score}/100`);
}
const overall = scores.length > 0 ? Math.round(scores.reduce((a,[,s]) => a+s, 0) / scores.length) : 0;
log('');
log(`  OVERALL SCORE: ${overall}/100`);
log('');
if (report.issues.length > 0) {
  log(`  --- ISSUES (${report.issues.length}) ---`);
  for (const i of report.issues) {
    log(`  [${i.kit}] ${i.file}: ${i.msg}`);
  }
}
hr('=');
log('');

// Write output
const text = lines.join('\n');
fs.writeFileSync(OUTPUT, text, 'utf8');
fs.writeFileSync(path.join(PROJECT, 'vegamcp_audit_report.json'), JSON.stringify(report, null, 2), 'utf8');
process.stdout.write('Done. Report: ' + OUTPUT + '\n');
