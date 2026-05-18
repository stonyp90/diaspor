/**
 * MetricsPage - Cutting-Edge Performance Dashboard
 * Professional monitoring UI with real-time visualizations
 */
import { useState, useEffect, useCallback } from 'react';
import { isTauriAvailable } from '../../hooks';
import { getStoredPollingInterval } from '../SettingsPage/SettingsPage';
import './MetricsPage.css';

const invokeTauri = async <T,>(command: string): Promise<T> => {
  if (!isTauriAvailable()) {
    throw new Error('Metrics are only available in the desktop app');
  }
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command);
};

// Types
interface GpuInfo {
  id: number;
  name: string;
  vendor: string;
  memory_total_mb: number;
}

interface GpuMetrics {
  gpu_utilization: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_utilization: number;
  temperature_celsius: number | null;
  power_usage_watts: number | null;
  power_limit_watts: number | null;
  fan_speed_percent: number | null;
  clock_speed_mhz: number | null;
  memory_clock_mhz: number | null;
  pcie_throughput_tx_mbps: number | null;
  pcie_throughput_rx_mbps: number | null;
  encoder_utilization: number | null;
  decoder_utilization: number | null;
}

interface SystemMetrics {
  cpu_usage: number;
  per_core_usage: number[];
  memory_used_mb: number;
  memory_total_mb: number;
  memory_usage_percent: number;
  swap_used_mb: number;
  swap_total_mb: number;
  disk_read_bytes_sec: number;
  disk_write_bytes_sec: number;
  network_rx_bytes_sec: number;
  network_tx_bytes_sec: number;
  load_average: [number, number, number];
  uptime_seconds: number;
}

interface GpuWithMetrics {
  info: GpuInfo;
  current: GpuMetrics;
}

interface AllMetrics {
  gpus: GpuWithMetrics[];
  system: SystemMetrics;
}

interface ThresholdConfig {
  cpu: number;
  memory: number;
  gpu: number;
  temperature: number;
}

const DEFAULT_THRESHOLDS: ThresholdConfig = {
  cpu: 90,
  memory: 85,
  gpu: 90,
  temperature: 85,
};

const loadThresholds = (): ThresholdConfig => {
  try {
    const saved = localStorage.getItem('diaspor-metric-thresholds');
    if (saved) return { ...DEFAULT_THRESHOLDS, ...JSON.parse(saved) };
  } catch {
    /* ignore */
  }
  return DEFAULT_THRESHOLDS;
};

const saveThresholds = (t: ThresholdConfig) => {
  try {
    localStorage.setItem('diaspor-metric-thresholds', JSON.stringify(t));
  } catch {
    /* ignore */
  }
};

// Helpers
const formatBytes = (b: number) => {
  if (b < 1024) return `${b.toFixed(0)} B`;
  if (b < 1024 ** 2) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 ** 3) return `${(b / 1024 ** 2).toFixed(1)} MB`;
  return `${(b / 1024 ** 3).toFixed(2)} GB`;
};

const formatThroughput = (b: number) => `${formatBytes(b)}/s`;

const formatUptime = (s: number) => {
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
};

// Brand-aligned per-metric colors. Default base is primary (indigo);
// callers pass cyan/violet for CPU/GPU. We only swap to a warning hue
// when the value approaches the threshold — the inline alert indicator
// + header badge handle the loud "this is broken" signal, so the chart
// itself stays calm and brand-on.
const BRAND_CYAN = 'var(--vfs-accent, #22d3ee)';
const BRAND_INDIGO = 'var(--vfs-primary, #4338ca)';
const BRAND_VIOLET = 'var(--vfs-secondary, #6366f1)';

const getColor = (v: number, t = 90, base: string = BRAND_INDIGO) => {
  if (v >= t * 0.9) return 'var(--vfs-warning, #f59e0b)';
  return base;
};

// Circular Progress Ring (unused - kept for future use)
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const Ring = ({
  value,
  max = 100,
  size = 120,
  strokeWidth = 10,
  label,
  subLabel,
  color,
}: {
  value: number;
  max?: number;
  size?: number;
  strokeWidth?: number;
  label: string;
  subLabel?: string;
  color: string;
}) => {
  const pct = Math.min((value / max) * 100, 100);
  const r = (size - strokeWidth) / 2;
  const c = r * 2 * Math.PI;
  const offset = c - (pct / 100) * c;

  return (
    <div className="ring-container" style={{ width: size, height: size }}>
      <svg viewBox={`0 0 ${size} ${size}`}>
        <circle
          className="ring-bg"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={strokeWidth}
        />
        <circle
          className="ring-value"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={strokeWidth}
          strokeDasharray={c}
          strokeDashoffset={offset}
          style={{ stroke: color }}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
      </svg>
      <div className="ring-content">
        <span className="ring-value-text">
          {value.toFixed(0)}
          <span className="ring-unit">%</span>
        </span>
        <div className="ring-label-row">
          <span className="ring-indicator" style={{ background: color }} />
          <span className="ring-label">{label}</span>
        </div>
        {subLabel && <span className="ring-sub">{subLabel}</span>}
      </div>
    </div>
  );
};

// Live Chart
const LiveChart = ({
  data,
  color,
  height = 60,
  showGrid = true,
}: {
  data: number[];
  color: string;
  height?: number;
  showGrid?: boolean;
}) => {
  if (data.length < 2)
    return <div className="chart-empty" style={{ height }} />;

  const max = Math.max(...data, 1);
  const pts = data
    .map(
      (v, i) => `${(i / (data.length - 1)) * 100},${100 - (v / max) * 90 - 5}`,
    )
    .join(' ');
  const area = `0,100 ${pts} 100,100`;

  return (
    <div className="chart-container" style={{ height }}>
      {showGrid && (
        <div className="chart-grid">
          {[0, 25, 50, 75, 100].map((p) => (
            <div key={p} className="grid-line" style={{ bottom: `${p}%` }} />
          ))}
        </div>
      )}
      <svg
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        className="chart-svg"
      >
        <defs>
          <linearGradient
            id={`grad-${color.replace(/[^a-zA-Z0-9]/g, '-')}`}
            x1="0"
            y1="0"
            x2="0"
            y2="1"
          >
            <stop offset="0%" stopColor={color} stopOpacity="0.4" />
            <stop offset="100%" stopColor={color} stopOpacity="0.05" />
          </linearGradient>
        </defs>
        <polygon
          points={area}
          fill={`url(#grad-${color.replace(/[^a-zA-Z0-9]/g, '-')})`}
          className="chart-area"
        />
        <polyline
          points={pts}
          fill="none"
          stroke={color}
          strokeWidth="2"
          strokeLinecap="round"
          className="chart-line"
        />
      </svg>
      <div className="chart-value" style={{ color }}>
        {data[data.length - 1]?.toFixed(1)}
      </div>
    </div>
  );
};

// Core Bar (for per-core CPU)
const CoreBar = ({ value, index }: { value: number; index: number }) => {
  const color = getColor(value);
  return (
    <div className="core-bar">
      <div
        className="core-fill"
        style={{ height: `${value}%`, background: color }}
      />
      <span className="core-label">{index}</span>
    </div>
  );
};

// Quick Stat - Compact horizontal stat with progress bar
const QuickStat = ({
  label,
  value,
  detail,
  color,
  showAsValue = false,
  maxValue = 100,
}: {
  label: string;
  value: number;
  detail?: string;
  color: string;
  showAsValue?: boolean;
  maxValue?: number;
}) => {
  // Cap the progress bar width at maxValue, but show actual value
  const progressWidth = Math.min((value / maxValue) * 100, 100);
  return (
    <div className="quick-stat">
      <div className="quick-stat-header">
        <span className="quick-stat-label">{label}</span>
        <span className="quick-stat-value" style={{ color }}>
          {showAsValue ? value.toFixed(0) : `${value.toFixed(0)}%`}
        </span>
      </div>
      <div className="quick-stat-bar">
        <div
          className="quick-stat-fill"
          style={{
            width: `${progressWidth}%`,
            background: color,
          }}
        />
      </div>
      {detail && <span className="quick-stat-detail">{detail}</span>}
    </div>
  );
};

// Metric Card - Clean design with small status indicator
const MetricCard = ({
  icon,
  title,
  value,
  unit,
  subtitle,
  statusColor,
  children,
  status,
  alerting = false,
  alertId,
}: {
  icon: React.ReactNode;
  title: string;
  value: string | number;
  unit?: string;
  subtitle?: string;
  statusColor?: string;
  children?: React.ReactNode;
  status?: 'good' | 'warn' | 'crit';
  alerting?: boolean;
  alertId?: string;
}) => (
  <div
    className={`metric-card ${status ? `status-${status}` : ''}`}
    data-alert={alerting ? 'true' : undefined}
    data-alert-id={alertId}
  >
    <div className="card-header">
      <div className="card-icon">{icon}</div>
      <div className="card-title">
        <span>{title}</span>
        {subtitle && <span className="card-subtitle">{subtitle}</span>}
      </div>
      <div className="card-value-wrapper">
        {alerting && (
          <span className="card-alert-indicator" aria-label="Threshold breached">
            ⚠
          </span>
        )}
        {/* Small color indicator dot */}
        {statusColor && (
          <span className="status-dot" style={{ background: statusColor }} />
        )}
        <span className="card-value">
          {value}
          {unit && <span className="card-unit">{unit}</span>}
        </span>
      </div>
    </div>
    {children && <div className="card-body">{children}</div>}
  </div>
);

// I/O Stat
const IOStat = ({
  label,
  value,
  icon,
  color,
}: {
  label: string;
  value: string;
  icon: React.ReactNode;
  color: string;
}) => (
  <div className="io-stat" style={{ borderColor: color }}>
    <div className="io-icon" style={{ color }}>
      {icon}
    </div>
    <div className="io-data">
      <span className="io-label">{label}</span>
      <span className="io-value">{value}</span>
    </div>
  </div>
);

// Settings Modal
const SettingsModal = ({
  isOpen,
  onClose,
  thresholds,
  onSave,
}: {
  isOpen: boolean;
  onClose: () => void;
  thresholds: ThresholdConfig;
  onSave: (t: ThresholdConfig) => void;
}) => {
  const [local, setLocal] = useState(thresholds);

  useEffect(() => {
    setLocal(thresholds);
  }, [thresholds, isOpen]);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) onClose();
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-box" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Alert Thresholds</h3>
          <button onClick={onClose} className="modal-close">
            ×
          </button>
        </div>
        <div className="modal-body">
          {(
            [
              ['cpu', 'CPU'],
              ['memory', 'Memory'],
              ['gpu', 'GPU'],
              ['temperature', 'Temperature'],
            ] as const
          ).map(([key, label]) => (
            <div key={key} className="threshold-row">
              <label>{label}</label>
              <input
                type="range"
                min={50}
                max={100}
                value={local[key]}
                onChange={(e) => setLocal({ ...local, [key]: +e.target.value })}
              />
              <span>{local[key]}%</span>
            </div>
          ))}
        </div>
        <div className="modal-footer">
          <button onClick={() => setLocal(DEFAULT_THRESHOLDS)}>Reset</button>
          <button
            className="primary"
            onClick={() => {
              onSave(local);
              onClose();
            }}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
};

// Icons
const CpuIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M9 1v3M15 1v3M9 20v3M15 20v3M1 9h3M1 15h3M20 9h3M20 15h3" />
  </svg>
);
const MemIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="2" y="6" width="20" height="12" rx="2" />
    <path d="M6 10v4M10 10v4M14 10v4M18 10v4" />
  </svg>
);
const GpuIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="2" y="4" width="20" height="16" rx="2" />
    <path d="M6 8h4M14 8h4M6 12h4M14 12h4" />
  </svg>
);
const DiskIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <circle cx="12" cy="12" r="3" />
  </svg>
);
const NetIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M5 12.55a11 11 0 0114.08 0M1.42 9a16 16 0 0121.16 0M8.53 16.11a6 6 0 016.95 0" />
    <circle cx="12" cy="20" r="1" />
  </svg>
);
const TempIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M14 14.76V3.5a2.5 2.5 0 00-5 0v11.26a4.5 4.5 0 105 0z" />
  </svg>
);
const UpIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="17 11 12 6 7 11" />
    <line x1="12" y1="6" x2="12" y2="18" />
  </svg>
);
const DownIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="7 13 12 18 17 13" />
    <line x1="12" y1="6" x2="12" y2="18" />
  </svg>
);
const SettingsIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
  </svg>
);
const LoadIcon = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
  </svg>
);

// Main Component
export function MetricsPage() {
  const [metrics, setMetrics] = useState<AllMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [thresholds, setThresholds] = useState<ThresholdConfig>(loadThresholds);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeAlerts, setActiveAlerts] = useState<Set<string>>(new Set());
  const [alertsPanelOpen, setAlertsPanelOpen] = useState(false);

  // Ensure dialog is closed by default when component mounts and on unmount
  useEffect(() => {
    setSettingsOpen(false);
    return () => {
      setSettingsOpen(false);
    };
  }, []);

  const [history, setHistory] = useState<{
    cpu: number[];
    mem: number[];
    gpu: number[];
    diskR: number[];
    diskW: number[];
    netRx: number[];
    netTx: number[];
  }>({
    cpu: [],
    mem: [],
    gpu: [],
    diskR: [],
    diskW: [],
    netRx: [],
    netTx: [],
  });

  // Silent state collector: maintains the active-alert set with hysteresis.
  // Cards inspect this set via `data-alert` and the header badge shows the count.
  // No toasts — breaches surface on-demand instead of interrupting the user.
  const checkAlerts = useCallback(
    (m: AllMetrics) => {
      setActiveAlerts((prev) => {
        const next = new Set(prev);

        // CPU
        if (m.system.cpu_usage >= thresholds.cpu) next.add('cpu');
        else if (m.system.cpu_usage < thresholds.cpu * 0.9) next.delete('cpu');

        // Memory
        if (m.system.memory_usage_percent >= thresholds.memory) next.add('mem');
        else if (m.system.memory_usage_percent < thresholds.memory * 0.9)
          next.delete('mem');

        // Per-GPU utilization and temperature
        m.gpus.forEach((g, i) => {
          const gpuKey = `gpu${i}`;
          if (g.current.gpu_utilization >= thresholds.gpu) next.add(gpuKey);
          else if (g.current.gpu_utilization < thresholds.gpu * 0.9)
            next.delete(gpuKey);

          const tempKey = `temp${i}`;
          const temp = g.current.temperature_celsius;
          if (temp != null && temp >= thresholds.temperature) next.add(tempKey);
          else if (temp == null || temp < thresholds.temperature * 0.9)
            next.delete(tempKey);
        });

        // Skip the state update if nothing changed — avoids re-renders every poll
        if (
          next.size === prev.size &&
          [...next].every((k) => prev.has(k))
        ) {
          return prev;
        }
        return next;
      });
    },
    [thresholds],
  );

  // Human-readable labels for the alerts popover
  const buildAlertList = (): { key: string; label: string }[] => {
    if (!metrics) return [];
    const out: { key: string; label: string }[] = [];
    if (activeAlerts.has('cpu'))
      out.push({
        key: 'cpu',
        label: `CPU at ${metrics.system.cpu_usage.toFixed(0)}%`,
      });
    if (activeAlerts.has('mem'))
      out.push({
        key: 'mem',
        label: `Memory at ${metrics.system.memory_usage_percent.toFixed(0)}%`,
      });
    metrics.gpus.forEach((g, i) => {
      if (activeAlerts.has(`gpu${i}`))
        out.push({
          key: `gpu${i}`,
          label: `GPU at ${g.current.gpu_utilization.toFixed(0)}%`,
        });
      if (activeAlerts.has(`temp${i}`) && g.current.temperature_celsius != null)
        out.push({
          key: `temp${i}`,
          label: `GPU temp ${g.current.temperature_celsius.toFixed(0)}°C`,
        });
    });
    return out;
  };

  const fetchMetrics = useCallback(async () => {
    try {
      const m = await invokeTauri<AllMetrics>('get_all_metrics');
      setMetrics(m);
      setError(null);
      checkAlerts(m);

      const gpu = m.gpus[0];
      setHistory((h) => ({
        cpu: [...h.cpu.slice(-59), m.system.cpu_usage],
        mem: [...h.mem.slice(-59), m.system.memory_usage_percent],
        gpu: [...h.gpu.slice(-59), gpu?.current.gpu_utilization ?? 0],
        diskR: [...h.diskR.slice(-59), m.system.disk_read_bytes_sec],
        diskW: [...h.diskW.slice(-59), m.system.disk_write_bytes_sec],
        netRx: [...h.netRx.slice(-59), m.system.network_rx_bytes_sec],
        netTx: [...h.netTx.slice(-59), m.system.network_tx_bytes_sec],
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load metrics');
    }
  }, [checkAlerts]);

  useEffect(() => {
    if (!isTauriAvailable()) {
      setError('Metrics only available in desktop app');
      return;
    }

    // Start GPU polling when metrics page is opened
    (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('start_gpu_polling');
      } catch (err) {
        console.warn('Failed to start GPU polling:', err);
      }
    })();

    // Get initial polling interval from settings
    let pollingInterval = getStoredPollingInterval();
    let intervalId: ReturnType<typeof setInterval>;

    const startPolling = () => {
      fetchMetrics();
      intervalId = setInterval(fetchMetrics, pollingInterval);
    };

    startPolling();

    // Listen for polling interval changes
    const handleIntervalChange = (e: CustomEvent<number>) => {
      pollingInterval = e.detail;
      clearInterval(intervalId);
      intervalId = setInterval(fetchMetrics, pollingInterval);
    };

    window.addEventListener(
      'metrics-interval-changed',
      handleIntervalChange as EventListener,
    );

    return () => {
      clearInterval(intervalId);
      window.removeEventListener(
        'metrics-interval-changed',
        handleIntervalChange as EventListener,
      );
      // Stop GPU polling when metrics page is closed
      (async () => {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('stop_gpu_polling');
        } catch (err) {
          console.warn('Failed to stop GPU polling:', err);
        }
      })();
    };
  }, [fetchMetrics]);

  const handleSaveThresholds = (t: ThresholdConfig) => {
    setThresholds(t);
    saveThresholds(t);
    setActiveAlerts(new Set());
  };

  if (error) {
    return (
      <div className="metrics-page">
        <div className="metrics-error">
          <CpuIcon />
          <h2>Metrics Unavailable</h2>
          <p>{error}</p>
          <button onClick={fetchMetrics}>Retry</button>
        </div>
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="metrics-page">
        <div className="metrics-loading">
          <div className="loader" />
          <span>Loading metrics...</span>
        </div>
      </div>
    );
  }

  const sys = metrics.system;
  const gpu = metrics.gpus[0];
  // Per-metric brand assignment: CPU=cyan, Memory=indigo, GPU=violet,
  // Load=indigo. getColor swaps any of these to amber only when value
  // approaches the threshold.
  const cpuColor = getColor(sys.cpu_usage, thresholds.cpu, BRAND_CYAN);
  const memColor = getColor(
    sys.memory_usage_percent,
    thresholds.memory,
    BRAND_INDIGO,
  );
  const gpuColor = gpu
    ? getColor(gpu.current.gpu_utilization, thresholds.gpu, BRAND_VIOLET)
    : BRAND_VIOLET;
  // Load average: use CPU cores as baseline (load > cores = overloaded)
  // Load average is a ratio, not a percentage. For display purposes, we show it as
  // a percentage relative to core count, but cap the visual bar at 200% (2x cores)
  const coreCount = sys.per_core_usage?.length || 4;
  const loadRatio = sys.load_average[0] / coreCount; // e.g., 37.55 / 14 = 2.68 (268% of cores)
  const loadPercent = Math.min(loadRatio * 100, 200); // Cap visual at 200% for progress bar
  const loadColor = getColor(
    Math.min(loadRatio * 100, 100),
    100,
    BRAND_INDIGO,
  );

  return (
    <div className="metrics-page">
      {/* Header */}
      <header className="metrics-header">
        <div className="header-left">
          <h1>Performance Monitor</h1>
          <div className="header-stats">
            <span>
              <strong>Uptime:</strong> {formatUptime(sys.uptime_seconds)}
            </span>
            <span>
              <strong>Load:</strong> {sys.load_average[0].toFixed(2)} /{' '}
              {sys.load_average[1].toFixed(2)} /{' '}
              {sys.load_average[2].toFixed(2)}
            </span>
            <span>
              <strong>Cores:</strong> {sys.per_core_usage.length}
            </span>
          </div>
        </div>
        <div className="header-actions" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {activeAlerts.size > 0 && (
            <div className="alerts-badge-wrapper" style={{ position: 'relative' }}>
              <button
                type="button"
                className="alerts-badge"
                onClick={() => setAlertsPanelOpen((o) => !o)}
                aria-label={`${activeAlerts.size} active alert${activeAlerts.size === 1 ? '' : 's'}`}
                aria-expanded={alertsPanelOpen}
              >
                <span aria-hidden="true">⚠</span>
                <span className="alerts-badge-count">{activeAlerts.size}</span>
              </button>
              {alertsPanelOpen && (
                <div className="alerts-popover" role="dialog" aria-label="Active alerts">
                  <div className="alerts-popover-header">Active alerts</div>
                  <ul className="alerts-popover-list">
                    {buildAlertList().map(({ key, label }) => (
                      <li key={key}>
                        <button
                          type="button"
                          className="alerts-popover-item"
                          onClick={() => {
                            // alertId is a space-separated list (e.g. "gpu0 temp0")
                            // so we match with ~= which finds whole-word tokens
                            const el = document.querySelector<HTMLElement>(
                              `[data-alert-id~="${key}"]`,
                            );
                            if (el)
                              el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                            setAlertsPanelOpen(false);
                          }}
                        >
                          {label}
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}
          <button className="settings-btn" onClick={() => setSettingsOpen(true)}>
            <SettingsIcon />
          </button>
        </div>
      </header>

      {/* Quick Stats Bar */}
      <section className="quick-stats">
        <QuickStat
          label="CPU"
          value={sys.cpu_usage}
          detail={`${sys.per_core_usage.length} cores`}
          color={cpuColor}
        />
        <QuickStat
          label="RAM"
          value={sys.memory_usage_percent}
          detail={`${(sys.memory_used_mb / 1024).toFixed(1)} / ${(sys.memory_total_mb / 1024).toFixed(0)} GB`}
          color={memColor}
        />
        {gpu && (
          <>
            <QuickStat
              label="GPU"
              value={gpu.current.gpu_utilization}
              detail={gpu.info.name.slice(0, 16)}
              color={gpuColor}
            />
            <QuickStat
              label="VRAM"
              value={gpu.current.memory_utilization}
              detail={`${(gpu.current.memory_used_mb / 1024).toFixed(1)} / ${(gpu.info.memory_total_mb / 1024).toFixed(0)} GB`}
              color={getColor(gpu.current.memory_utilization)}
            />
            {gpu.current.temperature_celsius && (
              <QuickStat
                label="Temp"
                value={gpu.current.temperature_celsius}
                detail="°C"
                color={getColor(
                  gpu.current.temperature_celsius,
                  thresholds.temperature,
                )}
                showAsValue
              />
            )}
          </>
        )}
        <QuickStat
          label="Load"
          value={loadPercent}
          detail={`${sys.load_average[0].toFixed(2)} / ${coreCount} cores`}
          color={loadColor}
          maxValue={200}
        />
      </section>

      {/* Grid */}
      <section className="metrics-grid">
        {/* CPU */}
        <MetricCard
          icon={<CpuIcon />}
          title="CPU Usage"
          value={sys.cpu_usage.toFixed(1)}
          unit="%"
          statusColor={cpuColor}
          alerting={activeAlerts.has('cpu')}
          alertId="cpu"
          status={
            sys.cpu_usage >= thresholds.cpu
              ? 'crit'
              : sys.cpu_usage >= thresholds.cpu * 0.7
                ? 'warn'
                : 'good'
          }
        >
          <LiveChart data={history.cpu} color={cpuColor} height={80} />
          <div className="cores-grid">
            {sys.per_core_usage.map((v, i) => (
              <CoreBar key={i} value={v} index={i} />
            ))}
          </div>
        </MetricCard>

        {/* Memory */}
        <MetricCard
          icon={<MemIcon />}
          title="Memory"
          value={sys.memory_usage_percent.toFixed(1)}
          unit="%"
          subtitle={`${(sys.memory_used_mb / 1024).toFixed(1)} / ${(sys.memory_total_mb / 1024).toFixed(0)} GB`}
          statusColor={memColor}
          alerting={activeAlerts.has('mem')}
          alertId="mem"
        >
          <LiveChart data={history.mem} color={memColor} height={80} />
          {sys.swap_total_mb > 0 && (
            <div className="swap-info">
              <span>
                Swap:{' '}
                {((sys.swap_used_mb / sys.swap_total_mb) * 100).toFixed(0)}%
              </span>
              <span>
                {(sys.swap_used_mb / 1024).toFixed(1)} /{' '}
                {(sys.swap_total_mb / 1024).toFixed(0)} GB
              </span>
            </div>
          )}
        </MetricCard>

        {/* GPU */}
        {gpu && (
          <MetricCard
            icon={<GpuIcon />}
            title="GPU"
            value={gpu.current.gpu_utilization.toFixed(0)}
            unit="%"
            subtitle={gpu.info.name}
            statusColor={gpuColor}
            alerting={activeAlerts.has('gpu0') || activeAlerts.has('temp0')}
            alertId="gpu0 temp0"
          >
            <LiveChart data={history.gpu} color={gpuColor} height={80} />
            <div className="gpu-stats">
              {gpu.current.temperature_celsius && (
                <div className="gpu-stat">
                  <TempIcon />
                  <span>{gpu.current.temperature_celsius.toFixed(0)}°C</span>
                </div>
              )}
              {gpu.current.power_usage_watts && (
                <div className="gpu-stat">
                  <span>⚡</span>
                  <span>{gpu.current.power_usage_watts.toFixed(0)}W</span>
                </div>
              )}
              {gpu.current.fan_speed_percent !== null && (
                <div className="gpu-stat">
                  <span>🌀</span>
                  <span>{gpu.current.fan_speed_percent}%</span>
                </div>
              )}
              {gpu.current.clock_speed_mhz && (
                <div className="gpu-stat">
                  <span>Core</span>
                  <span>{gpu.current.clock_speed_mhz} MHz</span>
                </div>
              )}
            </div>
          </MetricCard>
        )}

        {/* Disk I/O */}
        <MetricCard
          icon={<DiskIcon />}
          title="Disk I/O"
          value={formatThroughput(
            sys.disk_read_bytes_sec + sys.disk_write_bytes_sec,
          )}
          unit=""
        >
          <div className="io-row">
            <IOStat
              label="Read"
              value={formatThroughput(sys.disk_read_bytes_sec)}
              icon={<UpIcon />}
              color={BRAND_CYAN}
            />
            <IOStat
              label="Write"
              value={formatThroughput(sys.disk_write_bytes_sec)}
              icon={<DownIcon />}
              color={BRAND_INDIGO}
            />
          </div>
          <div className="io-charts">
            <LiveChart
              data={history.diskR}
              color={BRAND_CYAN}
              height={50}
              showGrid={false}
            />
            <LiveChart
              data={history.diskW}
              color={BRAND_INDIGO}
              height={50}
              showGrid={false}
            />
          </div>
          <div className="io-peak">
            <span>Peak R: {formatBytes(Math.max(...history.diskR, 0))}/s</span>
            <span>Peak W: {formatBytes(Math.max(...history.diskW, 0))}/s</span>
          </div>
        </MetricCard>

        {/* Network */}
        <MetricCard
          icon={<NetIcon />}
          title="Network"
          value={formatThroughput(
            sys.network_rx_bytes_sec + sys.network_tx_bytes_sec,
          )}
          unit=""
        >
          <div className="io-row">
            <IOStat
              label="Download"
              value={formatThroughput(sys.network_rx_bytes_sec)}
              icon={<DownIcon />}
              color={BRAND_CYAN}
            />
            <IOStat
              label="Upload"
              value={formatThroughput(sys.network_tx_bytes_sec)}
              icon={<UpIcon />}
              color={BRAND_VIOLET}
            />
          </div>
          <div className="io-charts">
            <LiveChart
              data={history.netRx}
              color={BRAND_CYAN}
              height={50}
              showGrid={false}
            />
            <LiveChart
              data={history.netTx}
              color={BRAND_VIOLET}
              height={50}
              showGrid={false}
            />
          </div>
          <div className="io-peak">
            <span>Peak ↓: {formatBytes(Math.max(...history.netRx, 0))}/s</span>
            <span>Peak ↑: {formatBytes(Math.max(...history.netTx, 0))}/s</span>
          </div>
        </MetricCard>

        {/* System Load & Uptime */}
        <MetricCard
          icon={<LoadIcon />}
          title="System"
          value={sys.load_average[0].toFixed(2)}
          unit=""
          subtitle="Load Average"
          statusColor={loadColor}
        >
          <div className="load-grid">
            <div className="load-item">
              <span className="load-period">1m</span>
              <span className="load-value">
                {sys.load_average[0].toFixed(2)}
              </span>
            </div>
            <div className="load-item">
              <span className="load-period">5m</span>
              <span className="load-value">
                {sys.load_average[1].toFixed(2)}
              </span>
            </div>
            <div className="load-item">
              <span className="load-period">15m</span>
              <span className="load-value">
                {sys.load_average[2].toFixed(2)}
              </span>
            </div>
          </div>
          <div className="uptime-info">
            <span className="uptime-label">Uptime</span>
            <span className="uptime-value">
              {formatUptime(sys.uptime_seconds)}
            </span>
          </div>
        </MetricCard>
      </section>

      <SettingsModal
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        thresholds={thresholds}
        onSave={handleSaveThresholds}
      />
    </div>
  );
}
