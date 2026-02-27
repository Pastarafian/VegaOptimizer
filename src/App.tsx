import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./index.css";

// ═══════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════
interface SystemInfo { os_name: string; os_version: string; hostname: string; cpu_name: string; cpu_cores: number; total_memory_mb: number; used_memory_mb: number; available_memory_mb: number; memory_usage_percent: number; total_swap_mb: number; used_swap_mb: number; uptime_seconds: number; }
interface ProcessInfo { pid: number; name: string; memory_mb: number; cpu_percent: number; status: string; parent_pid: number | null; }
interface OptimizationItem { id: string; category: string; name: string; description: string; tooltip: string; risk: string; enabled_by_default: boolean; available: boolean; estimated_savings: string | null; }
interface OptimizationResult { id: string; name: string; success: boolean; message: string; duration_ms: number; memory_freed_mb: number | null; }
interface OptimizationReport { total_duration_ms: number; total_memory_freed_mb: number; items_attempted: number; items_succeeded: number; items_failed: number; results: OptimizationResult[]; memory_before_mb: number; memory_after_mb: number; }
interface LiveMetrics { cpu_usage: number; cpu_per_core: number[]; memory_used_mb: number; memory_total_mb: number; memory_percent: number; swap_used_mb: number; swap_total_mb: number; disk_read_bytes: number; disk_write_bytes: number; net_rx_bytes: number; net_tx_bytes: number; process_count: number; uptime_seconds: number; temperatures: TempReading[]; }
interface TempReading { label: string; temp_c: number; critical: number | null; }
interface HealthScore { overall: number; memory_score: number; cpu_score: number; disk_score: number; startup_score: number; uptime_score: number; details: HealthDetail[]; }
interface HealthDetail { category: string; score: number; label: string; suggestion: string; }
interface HardwareInfo { cpu_name: string; cpu_arch: string; cpu_cores_physical: number; cpu_cores_logical: number; cpu_frequency_mhz: number; ram_total_gb: number; ram_type: string; os_name: string; os_version: string; os_build: string; hostname: string; disks: DiskInfo[]; gpus: string[]; network_adapters: string[]; }
interface DiskInfo { name: string; mount_point: string; fs_type: string; total_gb: number; used_gb: number; free_gb: number; usage_percent: number; is_removable: boolean; }
interface StartupEntry { name: string; command: string; location: string; registry_path: string; enabled: boolean; publisher: string; impact: string; }
interface LargeFile { path: string; size_mb: number; extension: string; category: string; modified: string; }
interface BrowserInfo { name: string; cache_size_mb: number; cache_path: string; installed: boolean; }
interface PrivacyItem { id: string; name: string; description: string; category: string; data_size_mb: number; }
interface DriverInfo { name: string; provider: string; version: string; date: string; device_class: string; signed: boolean; status: string; }

type Page = "dashboard" | "optimizer" | "processes" | "startup" | "disk" | "privacy" | "drivers" | "hardware" | "network" | "debloater" | "benchmark" | "services" | "registry" | "battery" | "duplicates" | "disk_health";

const PROFILES = [
  { id: "gaming", emoji: "🎮", name: "Gaming", desc: "Max performance", ids: ["mem_working_set","mem_standby_list","proc_boost_foreground","proc_lower_idle","cpu_power_high","svc_telemetry","svc_xbox","vis_game_dvr","net_dns_flush"] },
  { id: "productivity", emoji: "💼", name: "Productivity", desc: "Balanced optimization", ids: ["mem_working_set","mem_system_cache","proc_lower_idle","proc_selective_trim","net_dns_flush","disk_temp_files"] },
  { id: "battery", emoji: "🔋", name: "Battery Saver", desc: "Low power mode", ids: ["proc_lower_idle","proc_selective_trim","svc_telemetry","svc_xbox","svc_search","vis_game_dvr","vis_tips"] },
  { id: "deep", emoji: "🧹", name: "Deep Clean", desc: "Everything enabled", ids: ["mem_working_set","mem_system_cache","mem_standby_list","mem_modified_page","mem_combined_page","mem_registry_cache","proc_lower_idle","proc_boost_foreground","proc_selective_trim","proc_handle_detect","svc_telemetry","svc_xbox","net_dns_flush","net_arp_flush","disk_temp_files","disk_thumbnails","disk_shader_cache","disk_error_reports"] },
  { id: "safe", emoji: "🛡️", name: "Safe Mode", desc: "Low-risk only", ids: ["mem_working_set","mem_system_cache","mem_registry_cache","proc_lower_idle","proc_selective_trim","net_dns_flush","disk_temp_files","disk_error_reports"] },
];

// ═══════════════════════════════════════════════════════════════════
// Helper Components
// ═══════════════════════════════════════════════════════════════════
function HealthRing({ score }: { score: number }) {
  const r = 72, c = 2 * Math.PI * r;
  const offset = c - (score / 100) * c;
  const color = score >= 80 ? "var(--success)" : score >= 60 ? "var(--warning)" : "var(--danger)";
  return (
    <div className="health-ring">
      <svg viewBox="0 0 160 160">
        <circle className="bg-ring" cx="80" cy="80" r={r} />
        <circle className="score-ring" cx="80" cy="80" r={r} style={{ stroke: color, strokeDasharray: c, strokeDashoffset: offset }} />
      </svg>
      <div className="score-text">
        <div className="score-number" style={{ color }}>{score}</div>
        <div className="score-label">Health Score</div>
      </div>
    </div>
  );
}

function ProgressBar({ value, color = "var(--accent)", size = "" }: { value: number; color?: string; size?: string }) {
  return (
    <div className={`progress-bar ${size}`}>
      <div className="progress-fill" style={{ width: `${Math.min(100, value)}%`, background: color }} />
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400), h = Math.floor((secs % 86400) / 3600), m = Math.floor((secs % 3600) / 60);
  return d > 0 ? `${d}d ${h}h ${m}m` : `${h}h ${m}m`;
}

// ═══════════════════════════════════════════════════════════════════
// App
// ═══════════════════════════════════════════════════════════════════
export default function App() {
  const [page, setPage] = useState<Page>("dashboard");

  // Dashboard state
  const [health, setHealth] = useState<HealthScore | null>(null);
  const [metrics, setMetrics] = useState<LiveMetrics | null>(null);
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);
  const metricsInterval = useRef<number | null>(null);

  // Optimizer state
  const [catalog, setCatalog] = useState<OptimizationItem[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [optimizing, setOptimizing] = useState(false);
  const [report, setReport] = useState<OptimizationReport | null>(null);
  const [expandedCats, setExpandedCats] = useState<Set<string>>(new Set());

  // Process manager
  const [processes, setProcesses] = useState<ProcessInfo[]>([]);
  const [procSort, setProcSort] = useState<"memory" | "cpu" | "name">("memory");
  const [procSearch, setProcSearch] = useState("");

  // Startup
  const [startupItems, setStartupItems] = useState<StartupEntry[]>([]);

  // Disk
  const [largeFiles, setLargeFiles] = useState<LargeFile[]>([]);
  const [scanning, setScanning] = useState(false);
  const [browsers, setBrowsers] = useState<BrowserInfo[]>([]);

  // Privacy
  const [privacyItems, setPrivacyItems] = useState<PrivacyItem[]>([]);

  // Drivers
  const [drivers, setDrivers] = useState<DriverInfo[]>([]);
  const [driversLoading, setDriversLoading] = useState(false);

  // Hardware
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);

  // ── Load data (staggered to avoid overloading) ──
  useEffect(() => {
    invoke<SystemInfo>("cmd_get_system_info").then(setSysInfo).catch(console.error);
    // Delay heavy calls slightly so they don't all run simultaneously
    setTimeout(() => invoke<HealthScore>("cmd_get_health_score").then(setHealth).catch(console.error), 500);
    setTimeout(() => invoke<OptimizationItem[]>("cmd_get_catalog").then(c => {
      setCatalog(c);
      setSelected(new Set(c.filter(i => i.enabled_by_default).map(i => i.id)));
      setExpandedCats(new Set([...new Set(c.map(i => i.category))]));
    }).catch(console.error), 1200);
  }, []);

  // ── Live metrics polling (5s, with overlap guard) ──
  const pollingRef = useRef(false);
  useEffect(() => {
    if (page === "dashboard") {
      const poll = () => {
        if (pollingRef.current) return; // skip if previous request still running
        pollingRef.current = true;
        invoke<LiveMetrics>("cmd_get_live_metrics")
          .then(setMetrics)
          .catch(console.error)
          .finally(() => { pollingRef.current = false; });
      };
      poll();
      metricsInterval.current = window.setInterval(poll, 5000);
      return () => { if (metricsInterval.current) clearInterval(metricsInterval.current); };
    } else {
      if (metricsInterval.current) clearInterval(metricsInterval.current);
    }
  }, [page]);

  // ── Page data loaders ──
  useEffect(() => {
    if (page === "processes") loadProcesses();
    if (page === "startup") invoke<StartupEntry[]>("cmd_list_startup").then(setStartupItems).catch(console.error);
    if (page === "disk") invoke<BrowserInfo[]>("cmd_detect_browsers").then(setBrowsers).catch(console.error);
    if (page === "privacy") invoke<PrivacyItem[]>("cmd_get_privacy_items").then(setPrivacyItems).catch(console.error);
    if (page === "drivers" && drivers.length === 0) loadDrivers();
    if (page === "hardware" && !hardware) invoke<HardwareInfo>("cmd_get_hardware_info").then(setHardware).catch(console.error);
  }, [page]);

  const loadProcesses = () => invoke<ProcessInfo[]>("cmd_get_processes").then(setProcesses).catch(console.error);

  const loadDrivers = () => {
    setDriversLoading(true);
    invoke<DriverInfo[]>("cmd_list_drivers").then(d => { setDrivers(d); setDriversLoading(false); }).catch(() => setDriversLoading(false));
  };

  // ── Optimizer actions ──
  const toggleItem = (id: string) => {
    setSelected(prev => { const s = new Set(prev); s.has(id) ? s.delete(id) : s.add(id); return s; });
  };

  const applyProfile = (ids: string[]) => setSelected(new Set(ids));

  const runOptimize = async () => {
    setOptimizing(true);
    try {
      const r = await invoke<OptimizationReport>("cmd_optimize", { ids: [...selected] });
      setReport(r);
      invoke<HealthScore>("cmd_get_health_score").then(setHealth).catch(console.error);
      invoke<SystemInfo>("cmd_get_system_info").then(setSysInfo).catch(console.error);
    } catch (e) { console.error(e); }
    setOptimizing(false);
  };

  const killProcess = async (pid: number) => {
    try { await invoke<string>("cmd_kill_process", { pid }); loadProcesses(); } catch (e) { console.error(e); }
  };

  const scanLargeFiles = async () => {
    setScanning(true);
    try { const f = await invoke<LargeFile[]>("cmd_scan_large_files", { minSizeMb: 100 }); setLargeFiles(f); } catch (e) { console.error(e); }
    setScanning(false);
  };

  const cleanBrowser = async (name: string) => {
    try { await invoke<string>("cmd_clean_browser", { name }); invoke<BrowserInfo[]>("cmd_detect_browsers").then(setBrowsers); } catch (e) { console.error(e); }
  };

  const cleanPrivacy = async (id: string) => {
    try { await invoke<string>("cmd_clean_privacy", { id }); invoke<PrivacyItem[]>("cmd_get_privacy_items").then(setPrivacyItems); } catch (e) { console.error(e); }
  };

  // ── Computed ──
  const categories = [...new Set(catalog.map(c => c.category))];
  const sortedProcesses = [...processes].filter(p => !procSearch || p.name.toLowerCase().includes(procSearch.toLowerCase()))
    .sort((a, b) => procSort === "memory" ? b.memory_mb - a.memory_mb : procSort === "cpu" ? b.cpu_percent - a.cpu_percent : a.name.localeCompare(b.name));

  // ═══════════════════════════════════════════════════════════════════
  // Render
  // ═══════════════════════════════════════════════════════════════════
  return (
    <div className="app-layout">
      {/* ── Sidebar ── */}
      <aside className="sidebar">
        <div className="sidebar-brand">
          <h1>◈ VegaOptimizer</h1>
          <div className="version">v3.0.0 — System Toolkit</div>
        </div>
        <nav className="sidebar-nav">
          <div className="sidebar-section">Monitor</div>
          <NavItem icon="📊" label="Dashboard" id="dashboard" active={page} onClick={setPage} />
          <NavItem icon="🌐" label="Network" id="network" active={page} onClick={setPage} />

          <div className="sidebar-section">Optimize</div>
          <NavItem icon="⚡" label="Optimizer" id="optimizer" active={page} onClick={setPage} />
          <NavItem icon="📋" label="Processes" id="processes" active={page} onClick={setPage} badge={metrics ? String(metrics.process_count) : undefined} />
          <NavItem icon="🚀" label="Startup" id="startup" active={page} onClick={setPage} />
          <NavItem icon="⚙️" label="Services" id="services" active={page} onClick={setPage} />

          <div className="sidebar-section">Cleanup</div>
          <NavItem icon="💾" label="Disk Analyzer" id="disk" active={page} onClick={setPage} />
          <NavItem icon="🔒" label="Privacy" id="privacy" active={page} onClick={setPage} />
          <NavItem icon="🗑️" label="Debloater" id="debloater" active={page} onClick={setPage} />
          <NavItem icon="🗂️" label="Registry" id="registry" active={page} onClick={setPage} />
          <NavItem icon="🔍" label="Duplicates" id="duplicates" active={page} onClick={setPage} />

          <div className="sidebar-section">System</div>
          <NavItem icon="📊" label="Benchmark" id="benchmark" active={page} onClick={setPage} />
          <NavItem icon="🩺" label="Disk Health" id="disk_health" active={page} onClick={setPage} />
          <NavItem icon="🔧" label="Drivers" id="drivers" active={page} onClick={setPage} />
          <NavItem icon="🖥️" label="Hardware" id="hardware" active={page} onClick={setPage} />
          <NavItem icon="🔋" label="Battery" id="battery" active={page} onClick={setPage} />
        </nav>
        <div className="sidebar-footer">
          {health && <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: 18, color: health.overall >= 80 ? "var(--success)" : health.overall >= 60 ? "var(--warning)" : "var(--danger)" }}>●</span>
            <span>Health: <strong>{health.overall}/100</strong></span>
          </div>}
        </div>
      </aside>

      {/* ── Main ── */}
      <main className="main-content">
        {page === "dashboard" && <DashboardPage health={health} metrics={metrics} sysInfo={sysInfo} />}
        {page === "optimizer" && (
          <OptimizerPage
            catalog={catalog} categories={categories} selected={selected}
            expandedCats={expandedCats} setExpandedCats={setExpandedCats}
            toggleItem={toggleItem} applyProfile={applyProfile}
            optimizing={optimizing} runOptimize={runOptimize}
            report={report} setReport={setReport}
          />
        )}
        {page === "processes" && <ProcessPage processes={sortedProcesses} sort={procSort} setSort={setProcSort} search={procSearch} setSearch={setProcSearch} refresh={loadProcesses} kill={killProcess} />}
        {page === "startup" && <StartupPage items={startupItems} />}
        {page === "disk" && <DiskPage files={largeFiles} scanning={scanning} scan={scanLargeFiles} browsers={browsers} cleanBrowser={cleanBrowser} />}
        {page === "privacy" && <PrivacyPage items={privacyItems} clean={cleanPrivacy} />}
        {page === "drivers" && <DriverPage drivers={drivers} loading={driversLoading} refresh={loadDrivers} />}
        {page === "hardware" && <HardwarePage info={hardware} />}
        {page === "network" && <NetworkPage />}
        {page === "debloater" && <DebloaterPage />}
        {page === "benchmark" && <BenchmarkPage />}
        {page === "services" && <ServicesPage />}
        {page === "registry" && <RegistryPage />}
        {page === "battery" && <BatteryPage />}
        {page === "duplicates" && <DuplicatesPage />}
        {page === "disk_health" && <DiskHealthPage />}
      </main>

      {/* ── Report Overlay ── */}
      {report && <ReportOverlay report={report} onClose={() => setReport(null)} />}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Nav Item
// ═══════════════════════════════════════════════════════════════════
function NavItem({ icon, label, id, active, onClick, badge }: { icon: string; label: string; id: Page; active: Page; onClick: (p: Page) => void; badge?: string }) {
  return (
    <div className={`nav-item ${active === id ? "active" : ""}`} onClick={() => onClick(id)}>
      <span className="icon">{icon}</span>
      <span>{label}</span>
      {badge && <span className="badge">{badge}</span>}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Dashboard
// ═══════════════════════════════════════════════════════════════════
function DashboardPage({ health, metrics, sysInfo }: { health: HealthScore | null; metrics: LiveMetrics | null; sysInfo: SystemInfo | null }) {
  return (
    <div>
      <div className="page-header">
        <div><h2>System Dashboard</h2><div className="subtitle">Real-time monitoring & health analysis</div></div>
        {sysInfo && <div style={{ textAlign: "right", fontSize: 12, color: "var(--text-muted)" }}>
          <div>{sysInfo.os_name} {sysInfo.os_version}</div>
          <div>{sysInfo.hostname}</div>
        </div>}
      </div>

      {/* Health + quick stats */}
      <div style={{ display: "grid", gridTemplateColumns: "280px 1fr", gap: 16, marginBottom: 16 }}>
        <div className="card" style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center" }}>
          {health ? <HealthRing score={health.overall} /> : <div className="spinner lg" />}
          {health && <div style={{ width: "100%", marginTop: 12 }}>
            {health.details.map(d => (
              <div key={d.category} className="metric-row">
                <span className="label">{d.category}</span>
                <span className="value" style={{ color: d.score >= 80 ? "var(--success)" : d.score >= 60 ? "var(--warning)" : "var(--danger)" }}>{d.score}</span>
              </div>
            ))}
          </div>}
        </div>

        <div className="card-grid card-grid-2" style={{ alignContent: "start" }}>
          <div className="stat-mini">
            <div className="stat-icon" style={{ background: "var(--accent-dim)", color: "var(--accent)" }}>⚡</div>
            <div><div className="stat-value">{metrics ? `${metrics.cpu_usage.toFixed(0)}%` : "..."}</div><div className="stat-label">CPU Usage</div></div>
          </div>
          <div className="stat-mini">
            <div className="stat-icon" style={{ background: "var(--purple-dim)", color: "var(--purple)" }}>🧠</div>
            <div><div className="stat-value">{metrics ? `${metrics.memory_percent.toFixed(0)}%` : "..."}</div><div className="stat-label">Memory ({metrics ? `${(metrics.memory_used_mb / 1024).toFixed(1)}` : "..."} GB)</div></div>
          </div>
          <div className="stat-mini">
            <div className="stat-icon" style={{ background: "var(--success-dim)", color: "var(--success)" }}>📡</div>
            <div><div className="stat-value">{metrics ? formatBytes(metrics.net_rx_bytes) : "..."}</div><div className="stat-label">Network RX</div></div>
          </div>
          <div className="stat-mini">
            <div className="stat-icon" style={{ background: "var(--warning-dim)", color: "var(--warning)" }}>⏱️</div>
            <div><div className="stat-value">{metrics ? formatUptime(metrics.uptime_seconds) : "..."}</div><div className="stat-label">Uptime</div></div>
          </div>
        </div>
      </div>

      {/* Live resource bars */}
      {metrics && (
        <div className="card" style={{ marginBottom: 16 }}>
          <div className="card-header"><h3>System Resources</h3><span style={{ fontSize: 11, color: "var(--text-muted)" }}>Live • Updates every 5s</span></div>
          <div style={{ display: "grid", gap: 14 }}>
            <div>
              <div className="metric-row"><span className="label">CPU</span><span className="value">{metrics.cpu_usage.toFixed(1)}%</span></div>
              <ProgressBar value={metrics.cpu_usage} color={metrics.cpu_usage > 80 ? "var(--danger)" : metrics.cpu_usage > 50 ? "var(--warning)" : "var(--accent)"} />
            </div>
            <div>
              <div className="metric-row"><span className="label">Memory</span><span className="value">{(metrics.memory_used_mb / 1024).toFixed(1)} / {(metrics.memory_total_mb / 1024).toFixed(1)} GB</span></div>
              <ProgressBar value={metrics.memory_percent} color={metrics.memory_percent > 85 ? "var(--danger)" : metrics.memory_percent > 60 ? "var(--warning)" : "var(--purple)"} />
            </div>
            {metrics.swap_total_mb > 0 && <div>
              <div className="metric-row"><span className="label">Swap</span><span className="value">{(metrics.swap_used_mb / 1024).toFixed(1)} / {(metrics.swap_total_mb / 1024).toFixed(1)} GB</span></div>
              <ProgressBar value={(metrics.swap_used_mb / metrics.swap_total_mb) * 100} color="var(--orange)" />
            </div>}
          </div>
          {/* CPU per-core bars */}
          {metrics.cpu_per_core.length > 0 && (
            <div style={{ marginTop: 16 }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>CPU Cores ({metrics.cpu_per_core.length})</div>
              <div style={{ display: "grid", gridTemplateColumns: `repeat(${Math.min(metrics.cpu_per_core.length, 16)}, 1fr)`, gap: 3 }}>
                {metrics.cpu_per_core.map((c, i) => (
                  <div key={i} title={`Core ${i}: ${c.toFixed(0)}%`} style={{ height: 40, background: "var(--bg-primary)", borderRadius: 3, position: "relative", overflow: "hidden" }}>
                    <div style={{ position: "absolute", bottom: 0, width: "100%", height: `${c}%`, background: c > 80 ? "var(--danger)" : c > 50 ? "var(--warning)" : "var(--accent)", borderRadius: 3, transition: "height 1s" }} />
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Temperatures */}
      {metrics && metrics.temperatures.length > 0 && (
        <div className="card">
          <div className="card-header"><h3>Temperatures</h3></div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 8 }}>
            {metrics.temperatures.map((t, i) => (
              <div key={i} className="metric-row">
                <span className="label" style={{ fontSize: 11 }}>{t.label}</span>
                <span className="value" style={{ color: t.temp_c > 80 ? "var(--danger)" : t.temp_c > 60 ? "var(--warning)" : "var(--success)" }}>{t.temp_c.toFixed(0)}°C</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Optimizer
// ═══════════════════════════════════════════════════════════════════
function OptimizerPage({ catalog, categories, selected, expandedCats, setExpandedCats, toggleItem, applyProfile, optimizing, runOptimize, report, setReport }: any) {
  return (
    <div>
      <div className="page-header"><div><h2>System Optimizer</h2><div className="subtitle">Select optimizations and clean your system</div></div></div>

      {/* Profiles */}
      <div style={{ marginBottom: 16 }}>
        <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-muted)", marginBottom: 8, textTransform: "uppercase", letterSpacing: 1 }}>Quick Profiles</div>
        <div className="profile-cards">
          {PROFILES.map(p => (
            <div key={p.id} className="profile-card" onClick={() => applyProfile(p.ids)}>
              <div className="emoji">{p.emoji}</div>
              <div className="name">{p.name}</div>
              <div className="desc">{p.desc}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Categories */}
      <div style={{ marginBottom: 16 }}>
        {categories.map((cat: string) => {
          const items = catalog.filter((i: OptimizationItem) => i.category === cat);
          const selectedCount = items.filter((i: OptimizationItem) => selected.has(i.id)).length;
          const isExpanded = expandedCats.has(cat);
          return (
            <div key={cat} style={{ marginBottom: 4 }}>
              <div className="category-header" onClick={() => setExpandedCats((prev: Set<string>) => { const s = new Set(prev); s.has(cat) ? s.delete(cat) : s.add(cat); return s; })}>
                <h3>{cat}</h3>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span className="count">{selectedCount}/{items.length}</span>
                  <span style={{ color: "var(--text-muted)" }}>{isExpanded ? "▼" : "▶"}</span>
                </div>
              </div>
              {isExpanded && (
                <div className="category-items">
                  {items.map((item: OptimizationItem) => (
                    <div key={item.id} className="checkbox-row" onClick={() => toggleItem(item.id)}>
                      <div className={`checkbox-custom ${selected.has(item.id) ? "checked" : ""}`} />
                      <div className="checkbox-info" style={{ flex: 1 }}>
                        <h4>{item.name} <span className={`badge badge-${item.risk}`} style={{ marginLeft: 6 }}>{item.risk}</span></h4>
                        <p>{item.description}</p>
                      </div>
                      {item.estimated_savings && <span style={{ fontSize: 11, color: "var(--success)", fontFamily: "'JetBrains Mono', monospace", whiteSpace: "nowrap" }}>~{item.estimated_savings}</span>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Optimize Button */}
      <button className="optimize-btn" disabled={optimizing || selected.size === 0} onClick={runOptimize}>
        {optimizing ? <><div className="spinner" style={{ display: "inline-block", marginRight: 8, borderTopColor: "white" }} /> OPTIMIZING...</> : <>⚡ OPTIMIZE NOW</>}
        <div className="sub">{selected.size} items selected</div>
      </button>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Process Manager (Enhanced)
// ═══════════════════════════════════════════════════════════════════
interface ProcessSuggestion { pid: number; name: string; memory_mb: number; cpu_percent: number; estimated_savings_mb: number; reason: string; severity: string; category: string; safe_to_optimize: boolean; }
interface ProcessOptResult { pid: number; name: string; memory_before_mb: number; memory_after_mb: number; freed_mb: number; success: boolean; message: string; }
interface ProcessOptReport { total_freed_mb: number; processes_trimmed: number; results: ProcessOptResult[]; }

const CATEGORY_LABELS: Record<string, { emoji: string; label: string; color: string }> = {
  bloated: { emoji: "🔴", label: "Bloated", color: "var(--danger)" },
  idle_hog: { emoji: "🟠", label: "Idle Hog", color: "var(--warning)" },
  duplicate: { emoji: "🟡", label: "Duplicate", color: "var(--orange)" },
  background: { emoji: "🔵", label: "Background", color: "var(--accent)" },
};

function ProcessPage({ processes, sort, setSort, search, setSearch, refresh, kill }: any) {
  const [suggestions, setSuggestions] = useState<ProcessSuggestion[]>([]);
  const [selectedPids, setSelectedPids] = useState<Set<number>>(new Set());
  const [loadingSuggestions, setLoadingSuggestions] = useState(false);
  const [optimizingProcs, setOptimizingProcs] = useState(false);
  const [procReport, setProcReport] = useState<ProcessOptReport | null>(null);
  const [view, setView] = useState<"suggestions" | "all">("suggestions");

  const loadSuggestions = useCallback(() => {
    setLoadingSuggestions(true);
    invoke<ProcessSuggestion[]>("cmd_get_process_suggestions").then(s => {
      setSuggestions(s);
      // Auto-select all suggestions
      setSelectedPids(new Set(s.map(sg => sg.pid)));
      setLoadingSuggestions(false);
    }).catch(() => setLoadingSuggestions(false));
  }, []);

  useEffect(() => { loadSuggestions(); }, []);

  const togglePid = (pid: number) => {
    setSelectedPids(prev => { const s = new Set(prev); s.has(pid) ? s.delete(pid) : s.add(pid); return s; });
  };

  const selectAll = () => setSelectedPids(new Set(suggestions.map(s => s.pid)));
  const selectNone = () => setSelectedPids(new Set());

  const totalEstimated = suggestions.filter(s => selectedPids.has(s.pid)).reduce((acc, s) => acc + s.estimated_savings_mb, 0);

  const runProcessOptimize = async () => {
    setOptimizingProcs(true);
    try {
      const r = await invoke<ProcessOptReport>("cmd_optimize_processes", { pids: [...selectedPids] });
      setProcReport(r);
      // Refresh both lists
      loadSuggestions();
      refresh();
    } catch (e) { console.error(e); }
    setOptimizingProcs(false);
  };

  return (
    <div>
      <div className="page-header">
        <div><h2>Process Manager</h2><div className="subtitle">{processes.length} processes • {suggestions.length} optimization suggestions</div></div>
        <div style={{ display: "flex", gap: 8 }}>
          <input type="text" placeholder="Search..." value={search} onChange={e => setSearch(e.target.value)}
            style={{ padding: "8px 12px", background: "var(--bg-input)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text-primary)", fontSize: 13, width: 200, outline: "none", fontFamily: "inherit" }} />
          <button className="btn btn-ghost btn-sm" onClick={() => { refresh(); loadSuggestions(); }}>↻ Refresh</button>
        </div>
      </div>

      <div className="tab-bar" style={{ marginBottom: 12 }}>
        <button className={`tab-btn ${view === "suggestions" ? "active" : ""}`} onClick={() => setView("suggestions")}>🎯 Suggestions ({suggestions.length})</button>
        <button className={`tab-btn ${view === "all" ? "active" : ""}`} onClick={() => setView("all")}>📋 All Processes</button>
        <button className={`tab-btn ${sort === "memory" ? "active" : ""}`} onClick={() => { setView("all"); setSort("memory"); }}>By Memory</button>
        <button className={`tab-btn ${sort === "cpu" ? "active" : ""}`} onClick={() => { setView("all"); setSort("cpu"); }}>By CPU</button>
      </div>

      {/* ── Suggestions View ── */}
      {view === "suggestions" && (
        <div>
          {/* Summary bar */}
          {suggestions.length > 0 && (
            <div className="card" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12, padding: "12px 20px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
                <div style={{ display: "flex", gap: 6 }}>
                  <button className="btn btn-ghost btn-sm" onClick={selectAll}>Select All</button>
                  <button className="btn btn-ghost btn-sm" onClick={selectNone}>Deselect</button>
                </div>
                <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
                  <strong style={{ color: "var(--text-primary)" }}>{selectedPids.size}</strong> of {suggestions.length} processes selected
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
                <div style={{ textAlign: "right" }}>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: 1 }}>Est. Savings</div>
                  <div style={{ fontSize: 18, fontWeight: 700, color: "var(--success)", fontFamily: "'JetBrains Mono', monospace" }}>
                    {totalEstimated >= 1024 ? `${(totalEstimated / 1024).toFixed(1)} GB` : `${totalEstimated.toFixed(0)} MB`}
                  </div>
                </div>
                <button
                  className="optimize-btn"
                  disabled={optimizingProcs || selectedPids.size === 0}
                  onClick={runProcessOptimize}
                  style={{ padding: "10px 24px", fontSize: 13, minWidth: 180 }}
                >
                  {optimizingProcs ? <><div className="spinner" style={{ display: "inline-block", marginRight: 8, borderTopColor: "white", width: 14, height: 14 }} /> OPTIMIZING...</> : <>⚡ OPTIMIZE SELECTED</>}
                </button>
              </div>
            </div>
          )}

          {loadingSuggestions ? (
            <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Analyzing processes...</p></div>
          ) : suggestions.length === 0 ? (
            <div className="empty-state"><div className="icon">✅</div><p>All processes look healthy — no optimization needed!</p></div>
          ) : (
            <div style={{ display: "grid", gap: 6, maxHeight: "calc(100vh - 340px)", overflow: "auto" }}>
              {suggestions.map(s => {
                const cat = CATEGORY_LABELS[s.category] || CATEGORY_LABELS.background;
                const isSelected = selectedPids.has(s.pid);
                return (
                  <div
                    key={s.pid}
                    className="card"
                    onClick={() => togglePid(s.pid)}
                    style={{
                      display: "grid", gridTemplateColumns: "32px 1fr auto", gap: 14, alignItems: "center",
                      cursor: "pointer", padding: "12px 16px",
                      borderLeft: `3px solid ${cat.color}`,
                      opacity: isSelected ? 1 : 0.55,
                      transition: "all 0.15s",
                    }}
                  >
                    {/* Checkbox */}
                    <div className={`checkbox-custom ${isSelected ? "checked" : ""}`} />

                    {/* Info */}
                    <div>
                      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                        <span style={{ fontWeight: 600, fontSize: 14, color: "var(--text-primary)" }}>{s.name}</span>
                        <span className={`badge badge-${s.severity}`}>{s.severity}</span>
                        <span style={{ fontSize: 11, color: cat.color, fontWeight: 600 }}>{cat.emoji} {cat.label}</span>
                        <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>PID {s.pid}</span>
                      </div>
                      <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.4 }}>{s.reason}</div>
                      <div style={{ display: "flex", gap: 16, marginTop: 6, fontSize: 11, fontFamily: "'JetBrains Mono', monospace" }}>
                        <span style={{ color: "var(--text-secondary)" }}>Memory: <strong style={{ color: s.memory_mb > 500 ? "var(--danger)" : s.memory_mb > 200 ? "var(--warning)" : "var(--text-primary)" }}>{s.memory_mb.toFixed(0)} MB</strong></span>
                        <span style={{ color: "var(--text-secondary)" }}>CPU: {s.cpu_percent.toFixed(1)}%</span>
                      </div>
                    </div>

                    {/* Savings estimate */}
                    <div style={{ textAlign: "right", minWidth: 90 }}>
                      <div style={{ fontSize: 16, fontWeight: 700, color: "var(--success)", fontFamily: "'JetBrains Mono', monospace" }}>
                        ~{s.estimated_savings_mb >= 1024 ? `${(s.estimated_savings_mb / 1024).toFixed(1)} GB` : `${s.estimated_savings_mb.toFixed(0)} MB`}
                      </div>
                      <div style={{ fontSize: 10, color: "var(--text-muted)", textTransform: "uppercase" }}>est. savings</div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* ── All Processes View ── */}
      {view === "all" && (
        <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 240px)" }}>
          <table className="data-table">
            <thead><tr><th>Name</th><th>PID</th><th>Memory</th><th>CPU</th><th>Status</th><th></th></tr></thead>
            <tbody>
              {processes.slice(0, 100).map((p: ProcessInfo) => (
                <tr key={p.pid}>
                  <td style={{ fontWeight: 500, color: "var(--text-primary)" }}>{p.name}</td>
                  <td className="mono">{p.pid}</td>
                  <td className="mono">{p.memory_mb.toFixed(1)} MB</td>
                  <td className="mono" style={{ color: p.cpu_percent > 50 ? "var(--danger)" : p.cpu_percent > 10 ? "var(--warning)" : "var(--text-secondary)" }}>{p.cpu_percent.toFixed(1)}%</td>
                  <td><span className={`badge ${p.status === "Run" ? "badge-low" : "badge-medium"}`}>{p.status}</span></td>
                  <td><button className="btn-icon" onClick={() => kill(p.pid)} title="Kill process">✕</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* ── Process Optimization Report Overlay ── */}
      {procReport && (
        <div className="overlay" onClick={() => setProcReport(null)}>
          <div className="overlay-panel" onClick={e => e.stopPropagation()}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
              <h2 style={{ fontSize: 20, fontWeight: 700 }}>⚡ Process Optimization Complete</h2>
              <button className="btn-icon" onClick={() => setProcReport(null)}>✕</button>
            </div>

            <div className="card-grid card-grid-3" style={{ marginBottom: 20 }}>
              <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--success)", fontSize: 18 }}>{procReport.processes_trimmed}</div><div className="stat-label">Trimmed</div></div></div>
              <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--danger)", fontSize: 18 }}>{procReport.results.filter(r => !r.success).length}</div><div className="stat-label">Failed</div></div></div>
              <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--accent)", fontSize: 18 }}>{procReport.total_freed_mb.toFixed(0)} MB</div><div className="stat-label">Total Freed</div></div></div>
            </div>

            <div style={{ maxHeight: 350, overflow: "auto" }}>
              {procReport.results.map((r, i) => (
                <div key={i} style={{ display: "grid", gridTemplateColumns: "20px 1fr auto auto", gap: 10, alignItems: "center", padding: "10px 0", borderBottom: "1px solid var(--border)", fontSize: 13 }}>
                  <span style={{ color: r.success ? "var(--success)" : "var(--danger)" }}>{r.success ? "✓" : "✗"}</span>
                  <div>
                    <span style={{ fontWeight: 500 }}>{r.name}</span>
                    <span style={{ fontSize: 11, color: "var(--text-muted)", marginLeft: 8 }}>PID {r.pid}</span>
                  </div>
                  {r.success ? (
                    <span style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace", color: "var(--text-secondary)" }}>
                      {r.memory_before_mb.toFixed(0)} → {r.memory_after_mb.toFixed(0)} MB
                    </span>
                  ) : <span />}
                  <span style={{ fontSize: 12, fontWeight: 600, fontFamily: "'JetBrains Mono', monospace", color: r.success ? "var(--success)" : "var(--danger)", minWidth: 70, textAlign: "right" }}>
                    {r.success ? `−${r.freed_mb.toFixed(1)} MB` : r.message}
                  </span>
                </div>
              ))}
            </div>

            <button className="btn btn-accent" onClick={() => setProcReport(null)} style={{ width: "100%", marginTop: 16 }}>Close Report</button>
          </div>
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Startup Manager
// ═══════════════════════════════════════════════════════════════════
function StartupPage({ items }: { items: StartupEntry[] }) {
  const [toggling, setToggling] = useState<string | null>(null);
  const [localItems, setLocalItems] = useState<StartupEntry[]>(items);

  useEffect(() => { setLocalItems(items); }, [items]);

  const toggleStartup = async (item: StartupEntry) => {
    setToggling(item.name);
    try {
      await invoke<string>("cmd_toggle_startup", { name: item.name, registryPath: item.registry_path, enable: !item.enabled });
      setLocalItems(prev => prev.map(s => s.name === item.name ? { ...s, enabled: !s.enabled } : s));
    } catch (e) { console.error(e); }
    setToggling(null);
  };

  const enabled = localItems.filter(s => s.enabled).length;
  const disabled = localItems.length - enabled;

  return (
    <div>
      <div className="page-header">
        <div><h2>🚀 Startup Manager</h2><div className="subtitle">{localItems.length} entries • {enabled} enabled • {disabled} disabled</div></div>
      </div>
      <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 180px)" }}>
        <table className="data-table">
          <thead><tr><th>Name</th><th>Status</th><th>Location</th><th>Impact</th><th>Command</th><th></th></tr></thead>
          <tbody>
            {localItems.map((s, i) => (
              <tr key={i} style={{ opacity: s.enabled ? 1 : 0.6 }}>
                <td style={{ fontWeight: 500, color: "var(--text-primary)" }}>{s.name}</td>
                <td><span className={`badge ${s.enabled ? "badge-low" : "badge-medium"}`}>{s.enabled ? "Enabled" : "Disabled"}</span></td>
                <td><span className="badge badge-info">{s.location}</span></td>
                <td><span className={`badge badge-${s.impact === "High" ? "high" : s.impact === "Medium" ? "medium" : "low"}`}>{s.impact}</span></td>
                <td style={{ fontSize: 11, color: "var(--text-muted)", maxWidth: 250, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={s.command}>{s.command}</td>
                <td>
                  <button
                    className={`btn btn-sm ${s.enabled ? "btn-warning" : "btn-success"}`}
                    disabled={toggling === s.name}
                    onClick={() => toggleStartup(s)}
                    style={{ minWidth: 80 }}
                  >
                    {toggling === s.name ? "..." : s.enabled ? "Disable" : "Enable"}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {localItems.length === 0 && <div className="empty-state"><div className="icon">🚀</div><p>Loading startup entries...</p></div>}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Disk Analyzer
// ═══════════════════════════════════════════════════════════════════
function DiskPage({ files, scanning, scan, browsers, cleanBrowser }: any) {
  const [cleaningAll, setCleaningAll] = useState(false);
  const [localFiles, setLocalFiles] = useState<LargeFile[]>(files);
  const [localBrowsers, setLocalBrowsers] = useState<BrowserInfo[]>(browsers);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [deletingFile, setDeletingFile] = useState<string | null>(null);

  useEffect(() => { setLocalFiles(files); setSelectedFiles(new Set()); }, [files]);
  useEffect(() => { setLocalBrowsers(browsers); }, [browsers]);

  const deleteFile = async (path: string) => {
    if (!confirm(`Delete this file?\n${path}`)) return;
    setDeletingFile(path);
    try {
      await invoke<string>("cmd_delete_file", { path });
      setLocalFiles(prev => prev.filter(f => f.path !== path));
      setSelectedFiles(prev => { const s = new Set(prev); s.delete(path); return s; });
    } catch (e) { alert(String(e)); }
    setDeletingFile(null);
  };

  const deleteSelectedFiles = async () => {
    if (selectedFiles.size === 0) return;
    if (!confirm(`Delete ${selectedFiles.size} selected files?\n\nThis cannot be undone!`)) return;
    setDeletingFile("batch");
    let deleted = 0;
    for (const path of selectedFiles) {
      try {
        await invoke<string>("cmd_delete_file", { path });
        deleted++;
      } catch (e) { console.error(e); }
    }
    setLocalFiles(prev => prev.filter(f => !selectedFiles.has(f.path)));
    setSelectedFiles(new Set());
    setDeletingFile(null);
    alert(`Deleted ${deleted} files.`);
  };

  const cleanAllCaches = async () => {
    if (!confirm("Clean all browser caches? This will clear cached data from all detected browsers.")) return;
    setCleaningAll(true);
    const installed = localBrowsers.filter((b: BrowserInfo) => b.installed);
    for (const b of installed) {
      try { await invoke<string>("cmd_clean_browser", { name: b.name }); } catch (e) { console.error(e); }
    }
    // Refresh browser data to show updated cache sizes
    try { const updated = await invoke<BrowserInfo[]>("cmd_detect_browsers"); setLocalBrowsers(updated); } catch (e) { console.error(e); }
    setCleaningAll(false);
  };

  const totalCache = localBrowsers.filter((b: BrowserInfo) => b.installed).reduce((a: number, b: BrowserInfo) => a + b.cache_size_mb, 0);

  return (
    <div>
      <div className="page-header"><div><h2>Disk Analyzer</h2><div className="subtitle">Find large files and clean browser caches</div></div></div>

      {/* Browser caches */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h3>Browser Caches</h3>
          {localBrowsers.filter((b: BrowserInfo) => b.installed).length > 0 && (
            <button className="optimize-btn" onClick={cleanAllCaches} disabled={cleaningAll} style={{ padding: "8px 18px", fontSize: 12 }}>
              {cleaningAll ? <>⏳ Cleaning...</> : <>🧹 Clean All Caches ({totalCache.toFixed(0)} MB)</>}
            </button>
          )}
        </div>
        <div className="card-grid card-grid-3">
          {localBrowsers.filter((b: BrowserInfo) => b.installed).map((b: BrowserInfo) => (
            <div key={b.name} className="stat-mini" style={{ justifyContent: "space-between" }}>
              <div>
                <div style={{ fontWeight: 600, fontSize: 13 }}>{b.name}</div>
                <div style={{ fontSize: 12, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{b.cache_size_mb.toFixed(1)} MB</div>
              </div>
              <button className="btn btn-ghost btn-sm" onClick={() => cleanBrowser(b.name)}>Clean</button>
            </div>
          ))}
          {localBrowsers.filter((b: BrowserInfo) => b.installed).length === 0 && <div className="empty-state"><p>Detecting browsers...</p></div>}
        </div>
      </div>

      {/* Large file scanner */}
      <div className="card">
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h3>Large Files ({">"}100 MB) {localFiles.length > 0 && <span style={{ fontWeight: 400, fontSize: 12, color: "var(--text-muted)" }}> — {localFiles.length} files, {localFiles.reduce((a: number, f: LargeFile) => a + f.size_mb, 0).toFixed(0)} MB total</span>}</h3>
          <div style={{ display: "flex", gap: 8 }}>
            {selectedFiles.size > 0 && (
              <button className="optimize-btn" onClick={deleteSelectedFiles} disabled={deletingFile !== null} style={{ padding: "8px 18px", fontSize: 12 }}>
                🗑️ Delete Selected ({selectedFiles.size})
              </button>
            )}
            <button className="btn btn-accent btn-sm" onClick={scan} disabled={scanning}>
              {scanning ? <><div className="spinner" style={{ width: 14, height: 14 }} /> Scanning...</> : "🔍 Scan Now"}
            </button>
          </div>
        </div>
        {localFiles.length > 0 ? (
          <div style={{ overflow: "auto", maxHeight: 400 }}>
            <table className="data-table">
              <thead><tr>
                <th style={{ width: 30 }}><input type="checkbox" checked={selectedFiles.size === localFiles.length && localFiles.length > 0} onChange={e => { if (e.target.checked) setSelectedFiles(new Set(localFiles.map((f: LargeFile) => f.path))); else setSelectedFiles(new Set()); }} /></th>
                <th>File</th><th>Size</th><th>Type</th><th>Modified</th><th></th>
              </tr></thead>
              <tbody>
                {localFiles.map((f: LargeFile, i: number) => (
                  <tr key={i}>
                    <td><input type="checkbox" checked={selectedFiles.has(f.path)} onChange={() => { const s = new Set(selectedFiles); if (s.has(f.path)) s.delete(f.path); else s.add(f.path); setSelectedFiles(s); }} /></td>
                    <td style={{ maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={f.path}>{f.path.split("\\").pop()}</td>
                    <td className="mono" style={{ color: f.size_mb > 1000 ? "var(--danger)" : "var(--warning)" }}>{f.size_mb.toFixed(0)} MB</td>
                    <td><span className="badge badge-purple">{f.category}</span></td>
                    <td style={{ fontSize: 11, color: "var(--text-muted)" }}>{f.modified}</td>
                    <td>
                      <button className="btn btn-ghost btn-sm" onClick={() => deleteFile(f.path)} disabled={deletingFile === f.path} style={{ color: "var(--danger)", fontSize: 11 }}>
                        {deletingFile === f.path ? "..." : "Delete"}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="empty-state"><div className="icon">💾</div><p>{scanning ? "Scanning your drives..." : "Click 'Scan Now' to find large files"}</p></div>
        )}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Privacy
// ═══════════════════════════════════════════════════════════════════
function PrivacyPage({ items, clean }: { items: PrivacyItem[]; clean: (id: string) => void }) {
  const [cleaningAll, setCleaningAll] = useState(false);
  const [localItems, setLocalItems] = useState<PrivacyItem[]>(items);

  useEffect(() => { setLocalItems(items); }, [items]);

  const cleanAll = async () => {
    if (!confirm(`Clean all ${localItems.length} privacy items? This will remove all tracked data.`)) return;
    setCleaningAll(true);
    for (const item of localItems) {
      try { await invoke<string>("cmd_clean_privacy", { id: item.id }); } catch (e) { console.error(e); }
    }
    // Refresh the privacy items list to reflect cleaned state
    try { const updated = await invoke<PrivacyItem[]>("cmd_get_privacy_items"); setLocalItems(updated); } catch (e) { console.error(e); }
    setCleaningAll(false);
  };

  const totalSize = localItems.reduce((a, i) => a + i.data_size_mb, 0);

  return (
    <div>
      <div className="page-header">
        <div><h2>🔒 Privacy Cleanup</h2><div className="subtitle">Remove tracking data and clear traces • {totalSize.toFixed(0)} MB total</div></div>
        {localItems.length > 0 && (
          <button className="optimize-btn" onClick={cleanAll} disabled={cleaningAll} style={{ padding: "10px 24px" }}>
            {cleaningAll ? <>{"\u23f3"} Cleaning...</> : <>{"\ud83e\uddf9"} CLEAN ALL ({localItems.length} items)</>}
          </button>
        )}
      </div>
      <div className="card-grid card-grid-2">
        {localItems.map(item => (
          <div key={item.id} className="card" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontWeight: 600, fontSize: 14, marginBottom: 4 }}>{item.name}</div>
              <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{item.description}</div>
              {item.data_size_mb > 0 && <div style={{ fontSize: 11, color: "var(--warning)", marginTop: 4, fontFamily: "'JetBrains Mono', monospace" }}>{item.data_size_mb.toFixed(1)} MB</div>}
            </div>
            <button className="btn btn-ghost btn-sm" onClick={() => clean(item.id)}>Clean</button>
          </div>
        ))}
        {localItems.length === 0 && <div className="empty-state" style={{ gridColumn: "1 / -1" }}><div className="icon">{"\ud83d\udd12"}</div><p>Loading privacy items...</p></div>}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Drivers
// ═══════════════════════════════════════════════════════════════════
function DriverPage({ drivers, loading, refresh }: { drivers: DriverInfo[]; loading: boolean; refresh: () => void }) {
  return (
    <div>
      <div className="page-header">
        <div><h2>Driver Manager</h2><div className="subtitle">{drivers.length} drivers detected</div></div>
        <button className="btn btn-ghost btn-sm" onClick={refresh} disabled={loading}>{loading ? "Scanning..." : "↻ Rescan"}</button>
      </div>
      {loading ? (
        <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Scanning drivers (this may take a moment)...</p></div>
      ) : (
        <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 180px)" }}>
          <table className="data-table">
            <thead><tr><th>Device</th><th>Provider</th><th>Version</th><th>Date</th><th>Status</th><th>Signed</th></tr></thead>
            <tbody>
              {drivers.map((d, i) => (
                <tr key={i}>
                  <td style={{ fontWeight: 500, color: "var(--text-primary)", maxWidth: 250, overflow: "hidden", textOverflow: "ellipsis" }}>{d.name}</td>
                  <td style={{ fontSize: 12 }}>{d.provider}</td>
                  <td className="mono">{d.version || "—"}</td>
                  <td style={{ fontSize: 11, color: "var(--text-muted)" }}>{d.date || "—"}</td>
                  <td><span className={`badge ${d.status === "OK" || d.status === "Running" ? "badge-low" : "badge-medium"}`}>{d.status}</span></td>
                  <td>{d.signed ? <span style={{ color: "var(--success)" }}>✓</span> : <span style={{ color: "var(--danger)" }}>✗</span>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Hardware
// ═══════════════════════════════════════════════════════════════════
function HardwarePage({ info }: { info: HardwareInfo | null }) {
  if (!info) return <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Detecting hardware...</p></div>;
  return (
    <div>
      <div className="page-header"><div><h2>Hardware Info</h2><div className="subtitle">{info.hostname} — {info.os_build}</div></div></div>

      <div className="card-grid card-grid-2" style={{ marginBottom: 16 }}>
        <div className="card">
          <div className="card-header"><h3>⚡ Processor</h3></div>
          <div className="card-value" style={{ fontSize: 18 }}>{info.cpu_name}</div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, marginTop: 12 }}>
            <div><div className="card-label">Architecture</div><div style={{ fontWeight: 600 }}>{info.cpu_arch}</div></div>
            <div><div className="card-label">Frequency</div><div style={{ fontWeight: 600 }}>{info.cpu_frequency_mhz} MHz</div></div>
            <div><div className="card-label">Physical Cores</div><div style={{ fontWeight: 600 }}>{info.cpu_cores_physical}</div></div>
            <div><div className="card-label">Logical Cores</div><div style={{ fontWeight: 600 }}>{info.cpu_cores_logical}</div></div>
          </div>
        </div>

        <div className="card">
          <div className="card-header"><h3>🧠 Memory</h3></div>
          <div className="card-value" style={{ fontSize: 18 }}>{info.ram_total_gb.toFixed(1)} GB</div>
          <div style={{ marginTop: 8 }}><div className="card-label">Type</div><div style={{ fontWeight: 600 }}>{info.ram_type}</div></div>
        </div>
      </div>

      {/* GPU */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-header"><h3>🎮 Graphics</h3></div>
        {info.gpus.map((g, i) => <div key={i} style={{ padding: "8px 0", borderBottom: i < info.gpus.length - 1 ? "1px solid var(--border)" : "none", fontWeight: 500 }}>{g}</div>)}
      </div>

      {/* Disks */}
      <div className="card" style={{ marginBottom: 16 }}>
        <div className="card-header"><h3>💾 Storage</h3></div>
        <div className="card-grid card-grid-2">
          {info.disks.map((d, i) => (
            <div key={i} style={{ padding: 12, background: "var(--bg-secondary)", borderRadius: 8 }}>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 6 }}>
                <span style={{ fontWeight: 600 }}>{d.mount_point}</span>
                <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{d.fs_type}</span>
              </div>
              <ProgressBar value={d.usage_percent} color={d.usage_percent > 90 ? "var(--danger)" : d.usage_percent > 75 ? "var(--warning)" : "var(--accent)"} />
              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 11, color: "var(--text-muted)" }}>
                <span>{d.used_gb.toFixed(1)} GB used</span>
                <span>{d.free_gb.toFixed(1)} GB free / {d.total_gb.toFixed(1)} GB</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Network */}
      <div className="card">
        <div className="card-header"><h3>🌐 Network Adapters</h3></div>
        {info.network_adapters.map((n, i) => <div key={i} style={{ padding: "6px 0", fontSize: 13, borderBottom: i < info.network_adapters.length - 1 ? "1px solid var(--border)" : "none" }}>{n}</div>)}
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Report Overlay
// ═══════════════════════════════════════════════════════════════════
function ReportOverlay({ report, onClose }: { report: OptimizationReport; onClose: () => void }) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="overlay-panel" onClick={e => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h2 style={{ fontSize: 20, fontWeight: 700 }}>✅ Optimization Complete</h2>
          <button className="btn-icon" onClick={onClose}>✕</button>
        </div>

        <div className="card-grid card-grid-3" style={{ marginBottom: 20 }}>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--success)", fontSize: 18 }}>{report.items_succeeded}</div><div className="stat-label">Succeeded</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--danger)", fontSize: 18 }}>{report.items_failed}</div><div className="stat-label">Failed</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--accent)", fontSize: 18 }}>{report.total_memory_freed_mb.toFixed(0)} MB</div><div className="stat-label">Freed</div></div></div>
        </div>

        <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
          <span>Memory: {report.memory_before_mb} MB → {report.memory_after_mb} MB</span>
          <span>Duration: {(report.total_duration_ms / 1000).toFixed(1)}s</span>
        </div>

        <div style={{ maxHeight: 300, overflow: "auto" }}>
          {report.results.map((r, i) => (
            <div key={i} style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 0", borderBottom: "1px solid var(--border)", fontSize: 13 }}>
              <span style={{ color: r.success ? "var(--success)" : "var(--danger)" }}>{r.success ? "✓" : "✗"}</span>
              <span style={{ flex: 1, fontWeight: 500 }}>{r.name}</span>
              <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{r.message}</span>
            </div>
          ))}
        </div>

        <button className="btn btn-accent" onClick={onClose} style={{ width: "100%", marginTop: 16 }}>Close Report</button>
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Network Monitor
// ═══════════════════════════════════════════════════════════════════
interface NetOverview { total_connections: number; tcp_established: number; tcp_listening: number; udp_active: number; processes_with_network: number; top_talkers: { pid: number; name: string; connections: number; status: string }[]; connections: { protocol: string; local_addr: string; remote_addr: string; state: string; pid: number; process_name: string }[]; }

function NetworkPage() {
  const [overview, setOverview] = useState<NetOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [ping, setPing] = useState<number | null>(null);
  const [view, setView] = useState<"talkers" | "connections">("talkers");

  const load = useCallback(() => {
    setLoading(true);
    invoke<NetOverview>("cmd_get_network_overview").then(o => { setOverview(o); setLoading(false); }).catch(() => setLoading(false));
  }, []);
  useEffect(() => { load(); }, []);

  const runPing = () => { invoke<number>("cmd_ping_test", { host: "8.8.8.8" }).then(setPing); };

  return (
    <div>
      <div className="page-header">
        <div><h2>🌐 Network Monitor</h2><div className="subtitle">Per-process connections & bandwidth</div></div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-ghost btn-sm" onClick={runPing}>🏓 Ping Test</button>
          <button className="btn btn-ghost btn-sm" onClick={load}>{loading ? "⏳" : "↻"} Refresh</button>
        </div>
      </div>

      {ping !== null && <div className="card" style={{ padding: "10px 16px", marginBottom: 12, display: "flex", gap: 16, alignItems: "center" }}>
        <span style={{ fontWeight: 600 }}>Ping to 8.8.8.8:</span>
        <span className="mono" style={{ fontSize: 18, fontWeight: 700, color: ping < 50 ? "var(--success)" : ping < 100 ? "var(--warning)" : "var(--danger)" }}>{ping.toFixed(0)} ms</span>
      </div>}

      {overview && (
        <div className="card-grid card-grid-4" style={{ marginBottom: 12 }}>
          <div className="stat-mini"><div><div className="stat-value">{overview.total_connections}</div><div className="stat-label">Total Connections</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--success)" }}>{overview.tcp_established}</div><div className="stat-label">Established</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--accent)" }}>{overview.tcp_listening}</div><div className="stat-label">Listening</div></div></div>
          <div className="stat-mini"><div><div className="stat-value">{overview.processes_with_network}</div><div className="stat-label">Processes</div></div></div>
        </div>
      )}

      <div className="tab-bar" style={{ marginBottom: 12 }}>
        <button className={`tab-btn ${view === "talkers" ? "active" : ""}`} onClick={() => setView("talkers")}>📊 Top Talkers</button>
        <button className={`tab-btn ${view === "connections" ? "active" : ""}`} onClick={() => setView("connections")}>🔗 All Connections</button>
      </div>

      {loading ? <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Scanning network...</p></div> :
        overview && view === "talkers" ? (
          <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 340px)" }}>
            <table className="data-table"><thead><tr><th>Process</th><th>PID</th><th>Connections</th><th>Status</th></tr></thead>
              <tbody>{overview.top_talkers.map((t, i) => (
                <tr key={i}><td style={{ fontWeight: 500 }}>{t.name}</td><td className="mono">{t.pid}</td><td className="mono">{t.connections}</td>
                  <td><span className={`badge ${t.status === "Heavy" ? "badge-high" : t.status === "Active" ? "badge-medium" : "badge-low"}`}>{t.status}</span></td></tr>
              ))}</tbody></table>
          </div>
        ) : overview ? (
          <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 340px)" }}>
            <table className="data-table"><thead><tr><th>Protocol</th><th>Local</th><th>Remote</th><th>State</th><th>Process</th></tr></thead>
              <tbody>{overview.connections.slice(0, 100).map((c, i) => (
                <tr key={i}><td><span className={`badge ${c.protocol === "TCP" ? "badge-low" : "badge-medium"}`}>{c.protocol}</span></td>
                  <td className="mono" style={{ fontSize: 11 }}>{c.local_addr}</td><td className="mono" style={{ fontSize: 11 }}>{c.remote_addr}</td>
                  <td><span className={`badge ${c.state === "Established" ? "badge-low" : c.state === "Listen" ? "badge-medium" : ""}`}>{c.state}</span></td>
                  <td>{c.process_name}</td></tr>
              ))}</tbody></table>
          </div>
        ) : null}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Windows Debloater
// ═══════════════════════════════════════════════════════════════════
interface AppxPkg { name: string; display_name: string; publisher: string; version: string; size_mb: number; category: string; safe_to_remove: boolean; description: string; }

function DebloaterPage() {
  const [packages, setPackages] = useState<AppxPkg[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<"all" | "bloatware" | "game" | "media" | "utility">("all");

  const load = useCallback(() => {
    setLoading(true);
    invoke<AppxPkg[]>("cmd_list_appx").then(p => { setPackages(p); setLoading(false); }).catch(() => setLoading(false));
  }, []);
  useEffect(() => { load(); }, []);

  const remove = (name: string) => {
    if (!confirm("Remove this app? This cannot be undone.")) return;
    invoke<string>("cmd_remove_appx", { name }).then(() => load()).catch(e => alert(String(e)));
  };

  const removeAll = () => {
    if (!confirm("Remove ALL bloatware? This will uninstall all safe-to-remove preinstalled apps.")) return;
    invoke<[string, boolean, string][]>("cmd_remove_all_bloatware").then(() => load());
  };

  const filtered = filter === "all" ? packages : packages.filter(p => p.category === filter);
  const bloatCount = packages.filter(p => p.safe_to_remove).length;
  const totalSize = packages.filter(p => p.safe_to_remove).reduce((a, p) => a + p.size_mb, 0);

  const CAT_COLORS: Record<string, string> = { bloatware: "var(--danger)", game: "var(--warning)", media: "var(--accent)", utility: "var(--success)", system: "var(--text-muted)" };

  return (
    <div>
      <div className="page-header">
        <div><h2>🗑️ Windows Debloater</h2><div className="subtitle">{packages.length} apps • {bloatCount} removable ({totalSize.toFixed(0)} MB)</div></div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-ghost btn-sm" onClick={load}>↻ Refresh</button>
          {bloatCount > 0 && <button className="optimize-btn" onClick={removeAll} style={{ padding: "8px 16px" }}>🗑️ Remove All Bloatware ({bloatCount})</button>}
        </div>
      </div>

      <div className="tab-bar" style={{ marginBottom: 12 }}>
        {["all", "bloatware", "game", "media", "utility"].map(f => (
          <button key={f} className={`tab-btn ${filter === f ? "active" : ""}`} onClick={() => setFilter(f as any)}>{f === "all" ? `All (${packages.length})` : `${f} (${packages.filter(p => p.category === f).length})`}</button>
        ))}
      </div>

      {loading ? <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Scanning installed apps...</p></div> : (
        <div style={{ display: "grid", gap: 6, maxHeight: "calc(100vh - 300px)", overflow: "auto" }}>
          {filtered.map((p, i) => (
            <div key={i} className="card" style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 12, padding: "12px 16px", borderLeft: `3px solid ${CAT_COLORS[p.category] || "var(--border)"}`, opacity: p.safe_to_remove ? 1 : 0.6 }}>
              <div>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                  <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{p.display_name}</span>
                  <span className={`badge ${p.safe_to_remove ? "badge-high" : "badge-low"}`}>{p.category}</span>
                  {p.size_mb > 0 && <span className="mono" style={{ fontSize: 11, color: "var(--text-muted)" }}>{p.size_mb.toFixed(1)} MB</span>}
                </div>
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{p.description || p.name}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>Publisher: {p.publisher} • v{p.version}</div>
              </div>
              {p.safe_to_remove && <button className="btn btn-ghost btn-sm" onClick={() => remove(p.name)} style={{ color: "var(--danger)", alignSelf: "center" }}>✕ Remove</button>}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// System Benchmark
// ═══════════════════════════════════════════════════════════════════
interface BenchResult { cpu_single_score: number; cpu_multi_score: number; cpu_cores_used: number; ram_read_mbps: number; ram_write_mbps: number; ram_latency_ns: number; disk_seq_read_mbps: number; disk_seq_write_mbps: number; disk_random_iops: number; total_score: number; duration_ms: number; }

function BenchmarkPage() {
  const [result, setResult] = useState<BenchResult | null>(null);
  const [running, setRunning] = useState(false);

  const run = () => {
    setRunning(true);
    invoke<BenchResult>("cmd_run_benchmark").then(r => { setResult(r); setRunning(false); }).catch(() => setRunning(false));
  };

  const scoreColor = (score: number, max: number) => {
    const pct = score / max;
    return pct > 0.7 ? "var(--success)" : pct > 0.4 ? "var(--warning)" : "var(--danger)";
  };

  return (
    <div>
      <div className="page-header">
        <div><h2>📊 System Benchmark</h2><div className="subtitle">CPU • RAM • Disk performance test</div></div>
        <button className="optimize-btn" onClick={run} disabled={running} style={{ padding: "10px 24px" }}>
          {running ? <><div className="spinner" style={{ display: "inline-block", marginRight: 8, borderTopColor: "white", width: 14, height: 14 }} /> RUNNING...</> : <>🚀 RUN BENCHMARK</>}
        </button>
      </div>

      {running && <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Running benchmark... this takes ~15 seconds</p></div>}

      {result && !running && (
        <div>
          <div className="card" style={{ textAlign: "center", padding: 24, marginBottom: 16 }}>
            <div style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: 2, color: "var(--text-muted)" }}>Overall Score</div>
            <div style={{ fontSize: 48, fontWeight: 800, color: scoreColor(result.total_score, 3000), fontFamily: "'JetBrains Mono', monospace" }}>{result.total_score}</div>
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>Completed in {(result.duration_ms / 1000).toFixed(1)}s using {result.cpu_cores_used} cores</div>
          </div>

          <div className="card-grid card-grid-3" style={{ marginBottom: 16 }}>
            <div className="card" style={{ padding: 16 }}>
              <h3 style={{ fontSize: 14, marginBottom: 12 }}>🧠 CPU</h3>
              <div style={{ display: "grid", gap: 8 }}>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Single Core</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.cpu_single_score, 2000) }}>{result.cpu_single_score.toFixed(0)}</div></div>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Multi Core ({result.cpu_cores_used})</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.cpu_multi_score, 8000) }}>{result.cpu_multi_score.toFixed(0)}</div></div>
              </div>
            </div>
            <div className="card" style={{ padding: 16 }}>
              <h3 style={{ fontSize: 14, marginBottom: 12 }}>💾 RAM</h3>
              <div style={{ display: "grid", gap: 8 }}>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Read</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.ram_read_mbps, 50000) }}>{(result.ram_read_mbps / 1000).toFixed(1)} GB/s</div></div>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Write</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.ram_write_mbps, 40000) }}>{(result.ram_write_mbps / 1000).toFixed(1)} GB/s</div></div>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Latency</div><div className="mono" style={{ fontSize: 14, color: "var(--text-secondary)" }}>{result.ram_latency_ns.toFixed(1)} ns</div></div>
              </div>
            </div>
            <div className="card" style={{ padding: 16 }}>
              <h3 style={{ fontSize: 14, marginBottom: 12 }}>💿 Disk</h3>
              <div style={{ display: "grid", gap: 8 }}>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Seq. Read</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.disk_seq_read_mbps, 3000) }}>{result.disk_seq_read_mbps.toFixed(0)} MB/s</div></div>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Seq. Write</div><div className="mono" style={{ fontSize: 20, fontWeight: 700, color: scoreColor(result.disk_seq_write_mbps, 2000) }}>{result.disk_seq_write_mbps.toFixed(0)} MB/s</div></div>
                <div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>Random 4K IOPS</div><div className="mono" style={{ fontSize: 14, color: "var(--text-secondary)" }}>{result.disk_random_iops.toFixed(0)}</div></div>
              </div>
            </div>
          </div>
        </div>
      )}

      {!result && !running && (
        <div className="empty-state"><div className="icon" style={{ fontSize: 48 }}>📊</div><p>Click Run Benchmark to test your system performance</p><p style={{ fontSize: 12, color: "var(--text-muted)" }}>Tests CPU, RAM, and Disk speed</p></div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Services Manager
// ═══════════════════════════════════════════════════════════════════
interface SvcInfo { name: string; display_name: string; status: string; start_type: string; memory_mb: number; pid: number; description: string; category: string; safe_to_disable: boolean; recommendation: string; }

function ServicesPage() {
  const [svcs, setSvcs] = useState<SvcInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<string>("all");
  const [search, setSearch] = useState("");

  const load = useCallback(() => {
    setLoading(true);
    invoke<SvcInfo[]>("cmd_list_services").then(s => { setSvcs(s); setLoading(false); }).catch(() => setLoading(false));
  }, []);
  useEffect(() => { load(); }, []);

  const stopSvc = (name: string) => invoke<string>("cmd_stop_service", { name }).then(() => load()).catch(e => alert(String(e)));
  const startSvc = (name: string) => invoke<string>("cmd_start_service", { name }).then(() => load()).catch(e => alert(String(e)));

  const CAT_COLORS: Record<string, string> = { essential: "var(--success)", optional: "var(--accent)", telemetry: "var(--danger)", gaming: "var(--warning)", media: "var(--orange)", unknown: "var(--text-muted)" };
  const filtered = svcs.filter(s => (filter === "all" || s.category === filter) && (!search || s.display_name.toLowerCase().includes(search.toLowerCase()) || s.name.toLowerCase().includes(search.toLowerCase())));
  const running = svcs.filter(s => s.status === "Running").length;

  return (
    <div>
      <div className="page-header">
        <div><h2>⚙️ Services Manager</h2><div className="subtitle">{svcs.length} services • {running} running</div></div>
        <div style={{ display: "flex", gap: 8 }}>
          <input type="text" placeholder="Search..." value={search} onChange={e => setSearch(e.target.value)}
            style={{ padding: "8px 12px", background: "var(--bg-input)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text-primary)", fontSize: 13, width: 200, outline: "none", fontFamily: "inherit" }} />
          <button className="btn btn-ghost btn-sm" onClick={load}>↻ Refresh</button>
        </div>
      </div>

      <div className="tab-bar" style={{ marginBottom: 12 }}>
        {["all", "essential", "optional", "telemetry", "gaming"].map(f => (
          <button key={f} className={`tab-btn ${filter === f ? "active" : ""}`} onClick={() => setFilter(f)}>{f} ({f === "all" ? svcs.length : svcs.filter(s => s.category === f).length})</button>
        ))}
      </div>

      {loading ? <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /></div> : (
        <div className="card" style={{ overflow: "auto", maxHeight: "calc(100vh - 280px)" }}>
          <table className="data-table"><thead><tr><th>Service</th><th>Status</th><th>Start</th><th>Memory</th><th>Category</th><th></th></tr></thead>
            <tbody>{filtered.slice(0, 100).map((s, i) => (
              <tr key={i}>
                <td><div style={{ fontWeight: 500, color: "var(--text-primary)" }}>{s.display_name}</div><div style={{ fontSize: 11, color: "var(--text-muted)" }}>{s.name}</div>{s.recommendation && <div style={{ fontSize: 10, color: "var(--text-muted)", fontStyle: "italic" }}>{s.recommendation}</div>}</td>
                <td><span className={`badge ${s.status === "Running" ? "badge-low" : "badge-medium"}`}>{s.status}</span></td>
                <td style={{ fontSize: 12 }}>{s.start_type}</td>
                <td className="mono" style={{ fontSize: 12 }}>{s.memory_mb > 0 ? `${s.memory_mb.toFixed(1)} MB` : "—"}</td>
                <td><span style={{ fontSize: 11, fontWeight: 600, color: CAT_COLORS[s.category] || "var(--text-muted)" }}>{s.category}</span></td>
                <td>
                  {s.status === "Running" && s.safe_to_disable && <button className="btn-icon" onClick={() => stopSvc(s.name)} title="Stop">⏹</button>}
                  {s.status !== "Running" && <button className="btn-icon" onClick={() => startSvc(s.name)} title="Start">▶</button>}
                </td>
              </tr>
            ))}</tbody></table>
        </div>
      )}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Registry Cleaner
// ═══════════════════════════════════════════════════════════════════
interface RegIssue { key_path: string; value_name: string; issue_type: string; description: string; severity: string; safe_to_fix: boolean; }
interface RegScan { issues: RegIssue[]; total_issues: number; by_type: [string, number][]; duration_ms: number; }

function RegistryPage() {
  const [scan, setScan] = useState<RegScan | null>(null);
  const [scanning, setScanning] = useState(false);
  const [fixingAll, setFixingAll] = useState(false);

  const runScan = () => {
    setScanning(true);
    invoke<RegScan>("cmd_scan_registry").then(r => { setScan(r); setScanning(false); }).catch(() => setScanning(false));
  };

  const fix = (issue: RegIssue) => {
    invoke<string>("cmd_fix_registry_issue", { keyPath: issue.key_path, valueName: issue.value_name, issueType: issue.issue_type })
      .then(() => {
        setScan(prev => prev ? { ...prev, issues: prev.issues.filter(i => i !== issue), total_issues: prev.total_issues - 1 } : null);
      })
      .catch(e => alert(String(e)));
  };

  const fixAll = async () => {
    if (!scan) return;
    const safeIssues = scan.issues.filter(i => i.safe_to_fix);
    if (safeIssues.length === 0) return;
    if (!confirm(`Fix all ${safeIssues.length} safe registry issues?`)) return;
    setFixingAll(true);
    let fixed = 0;
    for (const issue of safeIssues) {
      try {
        await invoke<string>("cmd_fix_registry_issue", { keyPath: issue.key_path, valueName: issue.value_name, issueType: issue.issue_type });
        fixed++;
      } catch (e) { console.error(e); }
    }
    // Re-scan to get fresh state
    try {
      const r = await invoke<RegScan>("cmd_scan_registry");
      setScan(r);
    } catch (e) { console.error(e); }
    setFixingAll(false);
    alert(`Fixed ${fixed} of ${safeIssues.length} issues.`);
  };

  const TYPE_COLORS: Record<string, string> = { orphaned_software: "var(--warning)", broken_shortcut: "var(--danger)", invalid_path: "var(--accent)", obsolete_clsid: "var(--text-muted)" };
  const safeCount = scan ? scan.issues.filter(i => i.safe_to_fix).length : 0;

  return (
    <div>
      <div className="page-header">
        <div><h2>🗂️ Registry Cleaner</h2><div className="subtitle">{scan ? `${scan.total_issues} issues found in ${(scan.duration_ms / 1000).toFixed(1)}s` : "Scan for broken registry entries"}</div></div>
        <div style={{ display: "flex", gap: 8 }}>
          {scan && safeCount > 0 && (
            <button className="optimize-btn" onClick={fixAll} disabled={fixingAll || scanning} style={{ padding: "10px 24px" }}>
              {fixingAll ? <>⏳ Fixing...</> : <>🔧 FIX ALL SAFE ({safeCount})</>}
            </button>
          )}
          <button className={scan && safeCount > 0 ? "btn btn-ghost" : "optimize-btn"} onClick={runScan} disabled={scanning} style={{ padding: "10px 24px" }}>
            {scanning ? <><div className="spinner" style={{ display: "inline-block", marginRight: 8, borderTopColor: "white", width: 14, height: 14 }} /> SCANNING...</> : <>🔍 SCAN REGISTRY</>}
          </button>
        </div>
      </div>

      {scanning && <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Scanning registry... this may take a moment</p></div>}

      {scan && !scanning && (
        <div>
          {scan.by_type.length > 0 && (
            <div className="card-grid card-grid-4" style={{ marginBottom: 12 }}>
              {scan.by_type.map(([type, count], i) => (
                <div key={i} className="stat-mini"><div><div className="stat-value" style={{ color: TYPE_COLORS[type] || "var(--text-primary)" }}>{count}</div><div className="stat-label">{type.replace(/_/g, " ")}</div></div></div>
              ))}
            </div>
          )}

          <div style={{ display: "grid", gap: 6, maxHeight: "calc(100vh - 320px)", overflow: "auto" }}>
            {scan.issues.map((issue, i) => (
              <div key={i} className="card" style={{ display: "grid", gridTemplateColumns: "1fr auto", padding: "12px 16px", borderLeft: `3px solid ${TYPE_COLORS[issue.issue_type] || "var(--border)"}` }}>
                <div>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                    <span className={`badge badge-${issue.severity}`}>{issue.severity}</span>
                    <span style={{ fontSize: 11, fontWeight: 600, color: TYPE_COLORS[issue.issue_type] || "var(--text-muted)" }}>{issue.issue_type.replace(/_/g, " ")}</span>
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.5 }}>{issue.description}</div>
                </div>
                {issue.safe_to_fix && <button className="btn btn-ghost btn-sm" onClick={() => fix(issue)} style={{ alignSelf: "center" }}>Fix</button>}
              </div>
            ))}
          </div>
        </div>
      )}

      {!scan && !scanning && <div className="empty-state"><div className="icon" style={{ fontSize: 48 }}>🗂️</div><p>Click Scan Registry to find broken entries</p></div>}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Battery Health
// ═══════════════════════════════════════════════════════════════════
interface BatteryInfo { present: boolean; status: string; charge_percent: number; design_capacity_mwh: number; full_charge_capacity_mwh: number; current_capacity_mwh: number; health_pct: number; wear_pct: number; voltage_mv: number; charge_rate_mw: number; estimated_runtime_min: number | null; cycle_count: number | null; chemistry: string; manufacturer: string; serial: string; }

function BatteryPage() {
  const [battery, setBattery] = useState<BatteryInfo | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    invoke<BatteryInfo>("cmd_get_battery_health").then(b => { setBattery(b); setLoading(false); }).catch(() => setLoading(false));
  }, []);

  if (loading) return <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /></div>;
  if (!battery || !battery.present) return <div className="empty-state"><div className="icon" style={{ fontSize: 48 }}>🔌</div><p>No battery detected — this device is running on AC power</p></div>;

  const healthColor = battery.health_pct >= 80 ? "var(--success)" : battery.health_pct >= 50 ? "var(--warning)" : "var(--danger)";
  const statusEmoji = battery.status === "Charging" ? "⚡" : battery.status === "Discharging" ? "🔋" : battery.status === "Full" ? "✅" : "🔌";

  return (
    <div>
      <div className="page-header">
        <div><h2>🔋 Battery Health</h2><div className="subtitle">{statusEmoji} {battery.status} — {battery.charge_percent}%</div></div>
      </div>

      <div className="card" style={{ textAlign: "center", padding: 24, marginBottom: 16 }}>
        <div style={{ fontSize: 11, textTransform: "uppercase", letterSpacing: 2, color: "var(--text-muted)" }}>Battery Health</div>
        <div style={{ fontSize: 48, fontWeight: 800, color: healthColor, fontFamily: "'JetBrains Mono', monospace" }}>{battery.health_pct}%</div>
        <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{battery.wear_pct.toFixed(1)}% wear</div>
        <ProgressBar value={battery.health_pct} color={healthColor} />
      </div>

      <div className="card-grid card-grid-3" style={{ marginBottom: 16 }}>
        <div className="card" style={{ padding: 16 }}>
          <h3 style={{ fontSize: 14, marginBottom: 12 }}>⚡ Current State</h3>
          <div style={{ display: "grid", gap: 8, fontSize: 13 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Charge</span><span className="mono" style={{ fontWeight: 600 }}>{battery.charge_percent}%</span></div>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Status</span><span>{statusEmoji} {battery.status}</span></div>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Voltage</span><span className="mono">{(battery.voltage_mv / 1000).toFixed(2)} V</span></div>
            {battery.charge_rate_mw !== 0 && <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Power</span><span className="mono">{(Math.abs(battery.charge_rate_mw) / 1000).toFixed(1)} W</span></div>}
            {battery.estimated_runtime_min && <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Runtime</span><span className="mono">{Math.floor(battery.estimated_runtime_min / 60)}h {battery.estimated_runtime_min % 60}m</span></div>}
          </div>
        </div>

        <div className="card" style={{ padding: 16 }}>
          <h3 style={{ fontSize: 14, marginBottom: 12 }}>📊 Capacity</h3>
          <div style={{ display: "grid", gap: 8, fontSize: 13 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Design</span><span className="mono">{(battery.design_capacity_mwh / 1000).toFixed(1)} Wh</span></div>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Full Charge</span><span className="mono">{(battery.full_charge_capacity_mwh / 1000).toFixed(1)} Wh</span></div>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Current</span><span className="mono">{(battery.current_capacity_mwh / 1000).toFixed(1)} Wh</span></div>
            <div style={{ display: "flex", justifyContent: "space-between" }}>
              <span style={{ color: "var(--text-muted)" }}>Lost Capacity</span>
              <span className="mono" style={{ color: "var(--danger)" }}>{((battery.design_capacity_mwh - battery.full_charge_capacity_mwh) / 1000).toFixed(1)} Wh</span>
            </div>
          </div>
        </div>

        <div className="card" style={{ padding: 16 }}>
          <h3 style={{ fontSize: 14, marginBottom: 12 }}>🔧 Details</h3>
          <div style={{ display: "grid", gap: 8, fontSize: 13 }}>
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Chemistry</span><span>{battery.chemistry}</span></div>
            {battery.cycle_count && <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Cycles</span><span className="mono">{battery.cycle_count}</span></div>}
            <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Manufacturer</span><span>{battery.manufacturer || "Unknown"}</span></div>
            {battery.serial && <div style={{ display: "flex", justifyContent: "space-between" }}><span style={{ color: "var(--text-muted)" }}>Serial</span><span className="mono" style={{ fontSize: 11 }}>{battery.serial}</span></div>}
          </div>
        </div>
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Duplicate File Finder
// ═══════════════════════════════════════════════════════════════════
interface DuplicateGroup { hash: string; size_mb: number; files: string[]; }
interface DupScanResult { groups: DuplicateGroup[]; total_duplicates: number; total_wasted_mb: number; files_scanned: number; duration_ms: number; }

function DuplicatesPage() {
  const [scan, setScan] = useState<DupScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [minSize, setMinSize] = useState(1);

  const runScan = () => {
    setScanning(true);
    invoke<DupScanResult>("cmd_scan_duplicates", { minSizeMb: minSize })
      .then(r => { setScan(r); setScanning(false); })
      .catch(() => setScanning(false));
  };

  const deleteDup = (path: string) => {
    if (!confirm(`Delete this file?\n${path}`)) return;
    invoke<string>("cmd_delete_duplicate", { path })
      .then(() => {
        // Remove from UI
        setScan(prev => {
          if (!prev) return null;
          const groups = prev.groups.map(g => ({
            ...g,
            files: g.files.filter(f => f !== path),
          })).filter(g => g.files.length > 1);
          return { ...prev, groups, total_duplicates: prev.total_duplicates - 1 };
        });
      })
      .catch(e => alert(String(e)));
  };

  const [deletingAll, setDeletingAll] = useState(false);

  const deleteAllDuplicates = async () => {
    if (!scan || scan.groups.length === 0) return;
    const totalDups = scan.groups.reduce((a, g) => a + g.files.length - 1, 0);
    if (!confirm(`Delete ${totalDups} duplicate files? This keeps the first copy of each group and removes the rest.\n\nThis cannot be undone!`)) return;
    setDeletingAll(true);
    let deleted = 0;
    for (const g of scan.groups) {
      // Skip the first file (keep it)
      for (let fi = 1; fi < g.files.length; fi++) {
        try {
          await invoke<string>("cmd_delete_duplicate", { path: g.files[fi] });
          deleted++;
        } catch (e) { console.error(e); }
      }
    }
    setDeletingAll(false);
    setScan(null);
    alert(`Deleted ${deleted} duplicate files.`);
  };

  return (
    <div>
      <div className="page-header">
        <div><h2>🔍 Duplicate File Finder</h2><div className="subtitle">{scan ? `${scan.total_duplicates} duplicates in ${scan.groups.length} groups — ${scan.total_wasted_mb.toFixed(0)} MB wasted` : "Find and remove duplicate files"}</div></div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ fontSize: 12, color: "var(--text-muted)" }}>Min size:</label>
          <select value={minSize} onChange={e => setMinSize(Number(e.target.value))}
            style={{ padding: "6px 10px", background: "var(--bg-input)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text-primary)", fontSize: 13, fontFamily: "inherit" }}>
            <option value={0.1}>100 KB</option>
            <option value={1}>1 MB</option>
            <option value={10}>10 MB</option>
            <option value={50}>50 MB</option>
          </select>
          {scan && scan.groups.length > 0 && (
            <button className="optimize-btn" onClick={deleteAllDuplicates} disabled={deletingAll || scanning} style={{ padding: "10px 24px" }}>
              {deletingAll ? <>⏳ Deleting...</> : <>🗑️ DELETE ALL DUPLICATES ({scan.total_wasted_mb.toFixed(0)} MB)</>}
            </button>
          )}
          <button className={scan && scan.groups.length > 0 ? "btn btn-ghost" : "optimize-btn"} onClick={runScan} disabled={scanning || deletingAll} style={{ padding: "10px 24px" }}>
            {scanning ? <><div className="spinner" style={{ display: "inline-block", marginRight: 8, borderTopColor: "white", width: 14, height: 14 }} /> SCANNING...</> : <>🔍 SCAN</>}
          </button>
        </div>
      </div>

      {scanning && <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Scanning files... this may take a while</p></div>}

      {scan && !scanning && scan.groups.length > 0 && (
        <div className="card-grid card-grid-3" style={{ marginBottom: 12 }}>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--warning)" }}>{scan.groups.length}</div><div className="stat-label">Duplicate Groups</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--danger)" }}>{scan.total_duplicates}</div><div className="stat-label">Total Duplicates</div></div></div>
          <div className="stat-mini"><div><div className="stat-value" style={{ color: "var(--accent)" }}>{scan.total_wasted_mb.toFixed(0)} MB</div><div className="stat-label">Wasted Space</div></div></div>
        </div>
      )}

      {scan && !scanning && (
        <div style={{ display: "grid", gap: 10, maxHeight: "calc(100vh - 340px)", overflow: "auto" }}>
          {scan.groups.map((g, gi) => (
            <div key={gi} className="card" style={{ padding: "14px 16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <span className="badge badge-medium">{g.files.length} copies</span>
                  <span className="mono" style={{ fontSize: 12, color: "var(--warning)" }}>{g.size_mb.toFixed(1)} MB each</span>
                </div>
                <span className="mono" style={{ fontSize: 11, color: "var(--text-muted)" }}>{g.hash.slice(0, 16)}...</span>
              </div>
              {g.files.map((f, fi) => (
                <div key={fi} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "6px 0", borderTop: fi > 0 ? "1px solid var(--border)" : "none", fontSize: 12 }}>
                  <span style={{ color: fi === 0 ? "var(--success)" : "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "80%" }} title={f}>
                    {fi === 0 ? "✓ " : ""}{f.split("\\").pop()}
                  </span>
                  {fi > 0 && (
                    <button className="btn btn-ghost btn-sm" onClick={() => deleteDup(f)} style={{ color: "var(--danger)", fontSize: 11 }}>Delete</button>
                  )}
                </div>
              ))}
            </div>
          ))}
        </div>
      )}

      {scan && !scanning && scan.groups.length === 0 && <div className="empty-state"><div className="icon">✅</div><p>No duplicate files found!</p></div>}
      {!scan && !scanning && <div className="empty-state"><div className="icon" style={{ fontSize: 48 }}>🔍</div><p>Click Scan to find duplicate files on your system</p><p style={{ fontSize: 12, color: "var(--text-muted)" }}>Uses content hashing for accurate detection</p></div>}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
// Disk Health (S.M.A.R.T.)
// ═══════════════════════════════════════════════════════════════════
interface DiskHealthInfo { name: string; model: string; serial: string; media_type: string; status: string; size_gb: number; temperature_c: number | null; health_pct: number; smart_attributes: { id: number; name: string; value: number; worst: number; threshold: number; raw_value: string; status: string }[]; }

function DiskHealthPage() {
  const [disks, setDisks] = useState<DiskHealthInfo[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    invoke<DiskHealthInfo[]>("cmd_get_disk_health")
      .then(d => { setDisks(d); setLoading(false); })
      .catch(() => setLoading(false));
  }, []);

  if (loading) return <div className="empty-state"><div className="spinner lg" style={{ margin: "0 auto" }} /><p style={{ marginTop: 12 }}>Reading disk health data...</p></div>;

  return (
    <div>
      <div className="page-header">
        <div><h2>🩺 Disk Health</h2><div className="subtitle">{disks.length} drives detected</div></div>
        <button className="btn btn-ghost btn-sm" onClick={() => {
          setLoading(true);
          invoke<DiskHealthInfo[]>("cmd_get_disk_health").then(d => { setDisks(d); setLoading(false); }).catch(() => setLoading(false));
        }}>↻ Refresh</button>
      </div>

      {disks.length === 0 && !loading && <div className="empty-state"><div className="icon">💿</div><p>No S.M.A.R.T. data available</p><p style={{ fontSize: 12, color: "var(--text-muted)" }}>Some drives (USB, virtual) don't support S.M.A.R.T.</p></div>}

      <div style={{ display: "grid", gap: 16 }}>
        {disks.map((disk, di) => {
          const statusColor = disk.status === "OK" || disk.status === "Healthy" ? "var(--success)" : disk.status === "Warning" ? "var(--warning)" : "var(--danger)";
          return (
            <div key={di} className="card" style={{ padding: 20 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
                <div>
                  <h3 style={{ fontSize: 16, fontWeight: 700, color: "var(--text-primary)" }}>{disk.model || disk.name}</h3>
                  <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>{disk.media_type} • {disk.size_gb.toFixed(0)} GB{disk.serial ? ` • S/N: ${disk.serial}` : ""}</div>
                </div>
                <div style={{ textAlign: "right" }}>
                  <span className={`badge ${disk.status === "OK" || disk.status === "Healthy" ? "badge-low" : "badge-high"}`} style={{ fontSize: 13, padding: "4px 12px" }}>{disk.status}</span>
                  {disk.temperature_c != null && <div className="mono" style={{ fontSize: 12, marginTop: 4, color: disk.temperature_c > 50 ? "var(--danger)" : "var(--text-secondary)" }}>{disk.temperature_c}°C</div>}
                </div>
              </div>

              {disk.health_pct > 0 && (
                <div style={{ marginBottom: 16 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, marginBottom: 4 }}>
                    <span style={{ color: "var(--text-muted)" }}>Health</span>
                    <span className="mono" style={{ color: statusColor, fontWeight: 700 }}>{disk.health_pct}%</span>
                  </div>
                  <ProgressBar value={disk.health_pct} color={statusColor} />
                </div>
              )}

              {disk.smart_attributes && disk.smart_attributes.length > 0 && (
                <div style={{ overflow: "auto", maxHeight: 250 }}>
                  <table className="data-table">
                    <thead><tr><th>ID</th><th>Attribute</th><th>Value</th><th>Worst</th><th>Threshold</th><th>Raw</th><th>Status</th></tr></thead>
                    <tbody>
                      {disk.smart_attributes.map((attr, ai) => (
                        <tr key={ai}>
                          <td className="mono" style={{ fontSize: 11 }}>{attr.id}</td>
                          <td style={{ fontSize: 12 }}>{attr.name}</td>
                          <td className="mono" style={{ fontSize: 12 }}>{attr.value}</td>
                          <td className="mono" style={{ fontSize: 12, color: "var(--text-muted)" }}>{attr.worst}</td>
                          <td className="mono" style={{ fontSize: 12, color: "var(--text-muted)" }}>{attr.threshold}</td>
                          <td className="mono" style={{ fontSize: 11, color: "var(--text-muted)" }}>{attr.raw_value}</td>
                          <td><span className={`badge ${attr.status === "OK" || attr.status === "Ok" ? "badge-low" : "badge-high"}`}>{attr.status}</span></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
