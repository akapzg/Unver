import React, { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useStore } from '../store/useStore';
import { api } from '../api/client';
import { Shield, Globe, Zap, ChevronDown, Cpu, HardDrive, Server, RefreshCw, Settings, Clock } from 'lucide-react';
import { useConfirm } from '../components/ConfirmDialog';

// ── Mini Line Chart (SVG) — smooth ────────────────────────────────────
const MiniLineChart = ({ data, color, height = 44 }) => {
  const svgW = 200;
  const h = height;
  const pad = 2;
  if (data.length < 2) return <svg viewBox={`0 0 ${svgW} ${h}`} style={{ width: '100%', height: 'auto' }} />;

  const max = Math.max(...data, 1);
  const min = Math.min(...data, 0);
  const range = max - min || 1;

  const toX = (i) => pad + (i / (data.length - 1)) * (svgW - 2 * pad);
  const toY = (v) => h - pad - ((v - min) / range) * (h - 2 * pad);

  // Smooth cubic bezier path
  const points = data.map((v, i) => ({ x: toX(i), y: toY(v) }));
  let path = `M${points[0].x},${points[0].y}`;
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[i], p1 = points[i + 1];
    const cp1x = p0.x + (p1.x - p0.x) * 0.4;
    const cp2x = p1.x - (p1.x - p0.x) * 0.4;
    path += ` C${cp1x},${p0.y} ${cp2x},${p1.y} ${p1.x},${p1.y}`;
  }
  const polyPts = `${pad},${h - pad} ${points.map(p => `${p.x},${p.y}`).join(' ')} ${svgW - pad},${h - pad}`;

  return (
    <svg viewBox={`0 0 ${svgW} ${h}`} className="mini-chart" style={{ color, width: '100%', height: 'auto' }}>
      <defs>
        <linearGradient id={`grad-${color.replace(/[^a-zA-Z0-9]/g,'')}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="currentColor" stopOpacity="0.25" />
          <stop offset="100%" stopColor="currentColor" stopOpacity="0.02" />
        </linearGradient>
      </defs>
      <polygon points={polyPts} fill={`url(#grad-${color.replace(/[^a-zA-Z0-9]/g,'')})`} />
      <path d={path} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
};

// ── Horizontal Bar ─────────────────────────────────────────────────────
const ProgressBar = ({ percent, label, color, sub }) => (
  <div style={{ marginBottom: 10 }}>
    <div className="flex justify-b items-c" style={{ marginBottom: 3 }}>
      <span style={{ fontSize: '0.75rem', color: 'var(--text-2)' }}>{label}</span>
      <span style={{ fontSize: '0.75rem', fontWeight: 600, color }}>{percent}%</span>
    </div>
    <div className="progress-bar-track">
      <div
        className="progress-bar-fill"
        style={{ width: `${Math.min(percent, 100)}%`, background: color }}
      />
    </div>
    {sub && <div style={{ fontSize: '0.65rem', color: 'var(--text-3)', marginTop: 2 }}>{sub}</div>}
  </div>
);

// ── Collapsible Card ───────────────────────────────────────────────────
const CollapsibleCard = ({ icon, title, count, logs, loading, extra, emptyText = 'No records', error = false }) => {
  const [open, setOpen] = useState(false);
  return (
    <div className="glass-card collapsible-card">
      <div className="flex items-c justify-b collapsible-header" onClick={() => setOpen(!open)}>
        <div className="flex items-c gap-2">
          {icon}
          <span style={{ fontWeight: 600, fontSize: '0.85rem' }}>{title}</span>
          {count > 0 && <span className="log-count-badge">{count}</span>}
          {error && <span className="badge badge-danger" style={{ fontSize: '0.65rem' }}>!</span>}
        </div>
        <div className="flex items-c gap-2">
          {extra && <span style={{ fontSize: '0.65rem', color: 'var(--text-3)' }}>{extra}</span>}
          <ChevronDown size={16} style={{ transform: open ? 'rotate(180deg)' : 'rotate(0deg)', transition: 'var(--transition)' }} />
        </div>
      </div>
      {open && (
        <div className="collapsible-body">
          {loading ? (
            <div className="text-center py-2"><div className="spinner" style={{ margin: '0 auto' }} /></div>
          ) : error ? (
            <div className="text-center py-2 text-danger text-sm">{t('loadFailed')}</div>
          ) : logs.length === 0 ? (
            <div className="text-muted text-sm text-center py-2">{emptyText}</div>
          ) : (
            logs.map((log, i) => (
              <div key={log.id || i} className="log-line">
                <span className={`badge ${log.level === 'ERROR' ? 'badge-danger' : log.level === 'WARN' ? 'badge-warn' : 'badge-success'}`} style={{ marginTop: 1 }}>
                  {log.level}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div className="text-sm truncate">{log.message}</div>
                  <div className="text-muted" style={{ fontSize: '0.65rem' }}>{log.created_at}</div>
                </div>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
};

const Dashboard = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { confirm } = useConfirm();
  const { stats, fetchStats } = useStore();

  const [ddnsLogs, setDdnsLogs] = useState([]);
  const [sslLogs, setSslLogs] = useState([]);
  const [loginLogs, setLoginLogs] = useState([]);
  const [proxyLogs, setProxyLogs] = useState([]);
  const [loadingLogs, setLoadingLogs] = useState({});
  const [netData, setNetData] = useState({ rx_rate: 0, tx_rate: 0, total_rx: 0, total_tx: 0, rx_rate_str: '0 B/s', tx_rate_str: '0 B/s', total_rx_str: '0 B', total_tx_str: '0 B', container_mode: false });
  const [netHistory, setNetHistory] = useState({ rx: [], tx: [] });
  const [publicIpData, setPublicIpData] = useState(null);
  const [currentTime, setCurrentTime] = useState('');

  // Update check
  const [updateInfo, setUpdateInfo] = useState(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updating, setUpdating] = useState(false);

  // Per-section error states
  const [categorizedLogsErrors, setCategorizedLogsErrors] = useState({});
  const [netError, setNetError] = useState(false);
  const [publicIpError, setPublicIpError] = useState(false);

  // Live clock
  useEffect(() => {
    const tick = () => {
      const now = new Date();
      setCurrentTime(now.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }));
    };
    tick();
    const timer = setInterval(tick, 10000);
    return () => clearInterval(timer);
  }, []);

  // Fetch stats
  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 10000);
    return () => clearInterval(interval);
  }, [fetchStats]);

  const fetchCategorizedLogs = useCallback(async (category, setter) => {
    setLoadingLogs(prev => ({ ...prev, [category]: true }));
    try {
      const { data } = await api.listLogsByCategory(category);
      setter(data);
      setCategorizedLogsErrors(prev => ({ ...prev, [category]: false }));
    } catch (_) {
      setCategorizedLogsErrors(prev => ({ ...prev, [category]: true }));
    }
    setLoadingLogs(prev => ({ ...prev, [category]: false }));
  }, []);

  // Fetch network stats
  const fetchNetwork = useCallback(async () => {
    try {
      const { data } = await api.networkStats();
      setNetData(data);
      setNetHistory(prev => ({
        rx: [...prev.rx.slice(-59), data.rx_rate || 0],
        tx: [...prev.tx.slice(-59), data.tx_rate || 0],
      }));
      setNetError(false);
    } catch (_) { setNetError(true); }
  }, []);

  // Fetch public IP
  const fetchPublicIp = useCallback(async () => {
    try {
      const { data } = await api.publicIp();
      setPublicIpData(data);
      setPublicIpError(false);
    } catch (_) { setPublicIpError(true); }
  }, []);

  // ── Update check handlers ──────────────────────────────────────────────
  const handleCheckUpdate = async () => {
    setCheckingUpdate(true);
    setUpdateInfo(null);
    try {
      const { data } = await api.checkUpdate();
      setUpdateInfo(data);
    } catch (_) {
      setUpdateInfo({ error: true });
    }
    setCheckingUpdate(false);
  };

  const handlePerformUpdate = async () => {
    if (!await confirm(t('updateConfirm'))) return;
    setUpdating(true);
    try {
      const { data } = await api.performUpdate();
      setUpdateInfo(data);
    } catch (_) {
      setUpdateInfo({ error: true, status: 'failed' });
    }
    setUpdating(false);
  };

  useEffect(() => {
    fetchCategorizedLogs('ddns', setDdnsLogs);
    fetchCategorizedLogs('ssl', setSslLogs);
    fetchCategorizedLogs('login', setLoginLogs);
    fetchCategorizedLogs('proxy', setProxyLogs);
    fetchNetwork();
    fetchPublicIp();

    const logInterval = setInterval(() => {
      fetchCategorizedLogs('ddns', setDdnsLogs);
      fetchCategorizedLogs('ssl', setSslLogs);
      fetchCategorizedLogs('login', setLoginLogs);
      fetchCategorizedLogs('proxy', setProxyLogs);
    }, 60000);

    const netInterval = setInterval(fetchNetwork, 3000);

    return () => {
      clearInterval(logInterval);
      clearInterval(netInterval);
    };
  }, [fetchCategorizedLogs, fetchNetwork, fetchPublicIp]);

  // Update sub text renderer
  const renderUpdateSub = () => {
    if (checkingUpdate) return <span className="text-muted text-sm">{t('checkingUpdate')}</span>;
    if (updateInfo?.error) return <span className="text-sm" style={{ color: 'var(--danger)' }}>{t('updateCheckFailed')}</span>;
    if (updateInfo?.status === 'updating') return <span className="text-sm" style={{ color: 'var(--accent)' }}>{t('updateUpdating')}</span>;
    if (updateInfo?.has_update && updateInfo?.mode === 'docker') return <span className="text-sm" style={{ color: 'var(--success)' }}>{t('updateFoundDocker', { version: updateInfo.latest })}</span>;
    if (updateInfo?.has_update) return <span className="text-sm" style={{ color: 'var(--success)' }}>{t('updateFound', { version: updateInfo.latest })}</span>;
    if (updateInfo) return <span className="text-sm text-muted">{t('upToDate')}</span>;
    return t('checkUpdate');
  };

  // Clickable stat cards
  const cards = [
    {
      label: t('activeProxies'),
      value: stats?.active_proxies || 0,
      icon: <Zap className="text-success" />,
      sub: t('ofTotal', { count: stats?.proxy_rules || 0 }),
      onClick: () => navigate('/proxies'),
      clickable: true,
    },
    {
      label: t('certificates'),
      value: stats?.certificates || 0,
      icon: <Shield className="text-accent" />,
      sub: t('autoRenewCount', { count: stats?.auto_renew_certs || 0 }),
      onClick: () => navigate('/ssl'),
      clickable: true,
    },
    {
      label: t('version'),
      value: stats?.version || '0.1.0',
      icon: <Globe className="text-accent-2" />,
      sub: updateInfo ? renderUpdateSub() : t('checkUpdate'),
      onClick: () => {
        if (checkingUpdate || updating) return;
        if (updateInfo?.has_update && updateInfo?.mode === 'bare_metal') handlePerformUpdate();
        else if (updateInfo?.has_update && updateInfo?.mode === 'docker') window.open(updateInfo.html_url, '_blank');
        else handleCheckUpdate();
      },
      clickable: true,
    },
  ];

  // Format uptime
  const uptimeSecs = stats?.uptime_seconds || 0;
  const uptimeDays = Math.floor(uptimeSecs / 86400);
  const uptimeHours = Math.floor((uptimeSecs % 86400) / 3600);
  const uptimeMins = Math.floor((uptimeSecs % 3600) / 60);
  const parts = [];
  if (uptimeDays > 0) parts.push(`${uptimeDays}${t('day')}`);
  if (uptimeHours > 0 || parts.length > 0) parts.push(`${uptimeHours}${t('hour')}`);
  parts.push(`${uptimeMins}${t('minute')}`);
  const uptimeStr = parts.join(' ');
  const dbSizeStr = stats?.db_size_bytes ? (stats.db_size_bytes >= 1048576 ? `${(stats.db_size_bytes / 1048576).toFixed(1)} MB` : `${Math.round(stats.db_size_bytes / 1024)} KB`) : t('unknown');
  const cpuPercent = stats?.cpu_percent ?? 0;
  const memPercent = stats?.mem_percent ?? 0;
  const memUsed = stats?.mem_used ?? 0;
  const memTotal = stats?.mem_total ?? 0;

  const formatMem = (bytes) => {
    if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
    if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(0)} MB`;
    return `${(bytes / 1024).toFixed(0)} KB`;
  };

  return (
    <div className="fade-in">
      <header className="page-header">
        <h1 className="page-title text-gradient">{t('dashboard')}</h1>
        <p className="page-subtitle">{t('dashboardSubtitle')}</p>
      </header>

      {/* ── Stat Cards Row ── */}
      <div className="stats-grid mt-4">
        {cards.map((card, i) => (
          <div
            key={i}
            className={`stat-card glass ${card.clickable ? 'stat-card-clickable' : ''}`}
            style={{ display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}
            onClick={card.onClick}
          >
            <div className="flex justify-b items-c mb-2">
              <span className="text-muted">{card.icon}</span>
            </div>
            <div>
              <div className="stat-value">{card.value}</div>
              <div className="stat-label">{card.label}</div>
              <p className="text-muted text-sm mt-1">{card.sub}</p>
            </div>
          </div>
        ))}

        {/* System Load Card — horizontal bars */}
        <div className="stat-card glass" style={{ display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
          <div className="flex items-c mb-3">
            <Cpu className="text-accent" />
          </div>
          <div style={{ marginTop: 'auto' }}>
            <ProgressBar
              percent={cpuPercent}
              label="CPU"
              color="var(--accent)"
            />
            <ProgressBar
              percent={memPercent}
              label={t('memory')}
              color="var(--accent-2)"
              sub={`${formatMem(memUsed)} / ${formatMem(memTotal)}`}
            />
          </div>
        </div>
      </div>

      {/* ── System Status + Network Chart Row ── */}
      <div className="dashboard-panels mt-4">
        {/* Left: System Status */}
        <div className="glass-panel glass-card">
          <h3 style={{ marginBottom: 12 }}>{t('systemStatus')}</h3>
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <div className="status-row">
              <div><Settings size={14} style={{opacity:0.4}} /></div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: '0.8rem', fontWeight: 500 }}>{t('proxyEngine')}</div>
                <div style={{ fontSize: '0.65rem', color: 'var(--text-3)' }}>{t('uptime')} {uptimeStr}</div>
              </div>
              <div className="flex items-c gap-2">
                <span className="pulse-dot online" />
                <span style={{ fontSize: '0.75rem', color: 'var(--success)' }}>{t('running')}</span>
              </div>
            </div>
            <div className="status-row">
              <div><HardDrive size={14} style={{opacity:0.4}} /></div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: '0.8rem', fontWeight: 500 }}>{t('database')}</div>
                <div style={{ fontSize: '0.65rem', color: 'var(--text-3)' }}>{t('dbSize')} {dbSizeStr}</div>
              </div>
              <div className="flex items-c gap-2">
                <span className="pulse-dot online" />
                <span style={{ fontSize: '0.75rem', color: 'var(--success)' }}>{t('healthy')}</span>
              </div>
            </div>
            <div className="status-row">
              <div><Globe size={14} style={{opacity:0.4}} /></div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: '0.8rem', fontWeight: 500 }}>{t('ipAddress')}</div>
                {publicIpData?.ipv6 && (
                  <div style={{ fontSize: '0.65rem', color: 'var(--text-3)' }} className="mono">{publicIpData.ipv6}</div>
                )}
              </div>
              <div className="flex items-c">
                {publicIpError ? (
                  <span className="text-muted text-sm" style={{ color: 'var(--danger)' }}>{t('loadFailed')}</span>
                ) : (
                  <span className="mono" style={{ fontSize: '0.8rem', fontWeight: 600 }}>{publicIpData?.ipv4 || '—'}</span>
                )}
              </div>
            </div>
            <div className="status-row">
              <div><Clock size={14} style={{opacity:0.4}} /></div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: '0.8rem', fontWeight: 500 }}>{t('currentTime')}</div>
              </div>
              <div className="flex items-c">
                <span className="mono" style={{ fontSize: '0.8rem', fontWeight: 600 }}>{currentTime}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Right: Network Speed Monitor */}
        <div className="glass-panel glass-card" style={{ display: 'flex', flexDirection: 'column' }}>
          <h3 style={{ marginBottom: 12 }}>{t('realtimeNetworkSpeed')}</h3>
          {netError ? (
            <div className="text-center py-4 text-muted text-sm" style={{ color: 'var(--danger)', flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              {t('loadFailed')}
            </div>
          ) : (
            <>
              <div className="flex gap-4" style={{ flex: 1, marginBottom: 8 }}>
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                  <div style={{ fontSize: '0.65rem', color: 'var(--text-3)', marginBottom: 2 }}>📥 {t('download')}</div>
                  <div style={{ flex: 1, display: 'flex', alignItems: 'center' }}>
                    <MiniLineChart data={netHistory.rx} color="var(--accent)" height={80} />
                  </div>
                  <div style={{ fontSize: '0.8rem', fontWeight: 500, color: 'var(--accent)' }}>{netData.rx_rate_str}</div>
                </div>
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
                  <div style={{ fontSize: '0.65rem', color: 'var(--text-3)', marginBottom: 2 }}>📤 {t('upload')}</div>
                  <div style={{ flex: 1, display: 'flex', alignItems: 'center' }}>
                    <MiniLineChart data={netHistory.tx} color="var(--accent-2)" height={80} />
                  </div>
                  <div style={{ fontSize: '0.8rem', fontWeight: 500, color: 'var(--accent-2)' }}>{netData.tx_rate_str}</div>
                </div>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--text-2)', borderTop: '1px solid var(--glass-border)', paddingTop: 8, marginTop: 'auto' }}>
                <span>{t('totalReceived')} {netData.total_rx_str}</span>
                <span>{t('totalSent')} {netData.total_tx_str}</span>
              </div>
            </>
          )}
        </div>
      </div>

      {/* ── System Logs — Collapsible Cards ── */}
      <div className="w-full" style={{ marginTop: 16 }}>
        <h3 style={{ marginBottom: 12, fontSize: '0.95rem' }}>{t('recentActivity')}</h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <CollapsibleCard
            icon={<RefreshCw size={14} />}
            title={t('ddnsLogs')}
            count={ddnsLogs.length}
            logs={ddnsLogs}
            loading={loadingLogs['ddns']}
            error={categorizedLogsErrors['ddns']}
            extra={ddnsLogs.length > 0 ? t('lastSync', { time: ddnsLogs[0]?.created_at || '' }) : null}
            emptyText={t('noRecords')}
          />
          <CollapsibleCard
            icon={<Shield size={14} />}
            title={t('sslLogs')}
            count={sslLogs.length}
            logs={sslLogs}
            loading={loadingLogs['ssl']}
            error={categorizedLogsErrors['ssl']}
            emptyText={t('noRecords')}
          />
          <CollapsibleCard
            icon={<Server size={14} />}
            title={t('loginLogs')}
            count={loginLogs.length}
            logs={loginLogs}
            loading={loadingLogs['login']}
            error={categorizedLogsErrors['login']}
            emptyText={t('noRecords')}
          />
          <CollapsibleCard
            icon={<Globe size={14} />}
            title={t('proxyLogs')}
            count={proxyLogs.length}
            logs={proxyLogs}
            loading={loadingLogs['proxy']}
            error={categorizedLogsErrors['proxy']}
            emptyText={t('noRecords')}
          />
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
