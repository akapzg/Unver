import React, { useState, useEffect } from 'react';
import { Globe, Save, Zap, Plus, Trash2, X } from 'lucide-react';
import { useStore } from '../store/useStore';
import { useToast } from '../components/Toast';
import { useConfirm } from '../components/ConfirmDialog';
import { useTranslation } from 'react-i18next';
import { api } from '../api/client';
import client from '../api/client';
import { logStyle, logIcon } from '../utils/logHelpers';

// ── Provider field definitions ──────────────────────────────────────────
// Each provider declares what credential fields it needs.
// Adding a new provider = just add an entry here + a provider_name option below.
const PROVIDER_FIELDS = {
  cloudflare: [
    { key: 'ddns_cf_token',  label: 'apiToken', type: 'password', placeholder: 'Cloudflare API Token' },
  ],
  aliyun: [
    { key: 'ddns_aliyun_access_key_id',     label: 'accessKeyId',     type: 'text',     placeholder: 'Aliyun AccessKey ID' },
    { key: 'ddns_aliyun_access_key_secret', label: 'accessKeySecret', type: 'password', placeholder: 'Aliyun AccessKey Secret' },
  ],
};

// All known provider credential keys (for initialization & cleanup)
const ALL_PROVIDER_KEYS = Object.values(PROVIDER_FIELDS).flat().map(f => f.key);

// Provider display names (user-visible labels)
const PROVIDER_LABELS = {
  cloudflare: 'Cloudflare',
  aliyun:     'Aliyun',
};

// ── Helpers ─────────────────────────────────────────────────────────────

/** Build a fresh default config with all provider fields set to '' */
const makeDefaultConfig = () => {
  const cfg = { id: Date.now(), ddns_enabled: false, ddns_provider: 'cloudflare', ddns_domains: '' };
  for (const k of ALL_PROVIDER_KEYS) cfg[k] = '';
  return cfg;
};

/** Clear all credential fields for a config (used when switching providers) */
const clearCredentials = (cfg) => {
  const cleaned = { ...cfg };
  for (const k of ALL_PROVIDER_KEYS) cleaned[k] = '';
  return cleaned;
};

// ── Component ────────────────────────────────────────────────────────────

const Ddns = () => {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { confirm } = useConfirm();
  const { settings, fetchSettings, updateSettings } = useStore();
  const [configs, setConfigs] = useState([makeDefaultConfig()]);
  const [domainStatuses, setDomainStatuses] = useState([]);
  const [saving, setSaving] = useState(false);

  // ── Load settings ──────────────────────────────────────────────────

  useEffect(() => { fetchSettings(); }, []);
  useEffect(() => {
    if (settings) {
      setConfigs(prev => prev.map((c, i) => i === 0 ? {
        ...c,
        ddns_enabled: settings.ddns_enabled || false,
        ddns_provider: settings.ddns_provider || 'cloudflare',
        ddns_domains: settings.ddns_domains || '',
        // Hydrate all known provider keys from settings
        ...Object.fromEntries(ALL_PROVIDER_KEYS.map(k => [k, settings[k] || ''])),
      } : c));
    }
  }, [settings]);

  // ── Domain status ──────────────────────────────────────────────────

  const fetchStatus = async () => {
    try {
      const { data } = await api.ddnsStatus();
      setDomainStatuses(data);
    } catch (_) {}
  };

  useEffect(() => {
    if (settings?.ddns_domains) fetchStatus();
  }, [settings?.ddns_domains]);

  // ── Config helpers ─────────────────────────────────────────────────

  const updateConfig = (id, field, value) => {
    setConfigs(prev => prev.map(c => c.id === id ? { ...c, [field]: value } : c));
  };

  const handleProviderChange = (cfgId, newProvider) => {
    setConfigs(prev => prev.map(c => {
      if (c.id !== cfgId) return c;
      return { ...clearCredentials(c), ddns_provider: newProvider };
    }));
  };

  const addConfig = () => {
    setConfigs(prev => [...prev, { ...makeDefaultConfig(), id: Date.now(), isNew: true }]);
  };

  const removeConfig = async (id, domainsStr) => {
    if (!await confirm(t('confirmDeleteDdns'))) return;
    const domains = (domainsStr || '').split(/[\n,]/).map(d => d.trim()).filter(Boolean);
    for (const domain of domains) {
      try {
        await client.delete(`/ddns/domain/${encodeURIComponent(domain)}`);
        addToast(t('domainCleared', { domain }), 'success');
      } catch (e) {
        addToast(t('domainClearFailed', { domain, error: e.response?.data?.error || e.message }), 'error');
      }
    }
    setConfigs(prev => prev.filter(c => c.id !== id));
  };

  // ── Save ───────────────────────────────────────────────────────────

  const handleSave = async () => {
    setSaving(true);
    try {
      const cfg = configs[0];
      const payload = {
        ddns_enabled: cfg.ddns_enabled,
        ddns_provider: cfg.ddns_provider,
        ddns_domains: cfg.ddns_domains,
      };
      for (const k of ALL_PROVIDER_KEYS) payload[k] = cfg[k] || '';
      await updateSettings(payload);
      addToast(t('configSaved'), 'success');
      await fetchStatus();
      // Auto pop log modal to show sync progress
      handleTest();
    } catch (e) {
      addToast(t('ddnsUpdateFailed'), 'error');
    }
    setSaving(false);
  };

  // ── Domain actions ─────────────────────────────────────────────────

  const toggleDomain = async (domain) => {
    const current = domainStatuses.find(d => d.domain === domain);
    if (!current) return;
    try {
      await api.ddnsToggle(domain, !current.enabled);
      await fetchStatus();
    } catch (e) {
      addToast(t('failed'), 'error');
    }
  };

  const deleteDomain = async (domain) => {
    if (!await confirm(t('confirmDeleteDomain', { domain }))) return;
    try {
      await api.ddnsDeleteDomain(domain);
      addToast(t('domainDeleted', { domain }), 'success');
      const cfg = configs[0];
      const domains = (cfg.ddns_domains || '').split(/[\n,]/).map(d => d.trim()).filter(Boolean);
      updateConfig(cfg.id, 'ddns_domains', domains.filter(d => d !== domain).join('\n'));
      await fetchStatus();
    } catch (e) {
      addToast(t('domainDeleteFailed', { domain, error: e.response?.data?.error || e.message }), 'error');
    }
  };

  // ── Test connection ────────────────────────────────────────────────

  const [testLogs, setTestLogs] = useState([]);
  const [showTestModal, setShowTestModal] = useState(false);
  const [testRunning, setTestRunning] = useState(false);

  const handleTest = async () => {
    setTestLogs([]);
    setShowTestModal(true);
    setTestRunning(true);
    const add = (msg, level = 'info') => setTestLogs(prev => [...prev, {
      timestamp: new Date().toLocaleTimeString('zh-CN', { hour12: false }),
      level, message: msg
    }]);
    add(t('testStart'));
    try {
      const { data } = await api.ddnsTest();
      const ipd = data.ip_detection;
      const cf = data.cf_connectivity;
      add(`📡 ${t('ipDetection')}: IPv4=${ipd?.ipv4 || '—'}, IPv6=${ipd?.ipv6 || '—'}`, ipd?.success ? 'success' : 'error');
      if (cf) {
        add(`🔗 Cloudflare Token: ${cf.token_valid ? `✅ ${t('valid')}` : `❌ ${t('invalid')}`}`, cf.token_valid ? 'success' : 'error');
        if (cf.token_valid) {
          add(`🌐 ${t('domainCountLabel')}: ${data.config?.domain_count || 0}`, 'info');
        }
      } else {
        add(t('noTokenConfigured'), 'info');
      }
      add(t('testComplete'), 'success');
    } catch (e) {
      add(`❌ ${t('testFailed')}: ${e.response?.data?.error || e.message}`, 'error');
    }
    setTestRunning(false);
  };

  // ── Render ──────────────────────────────────────────────────────────

  return (
    <div className="fade-in">
      <header className="page-header">
        <div className="flex items-c justify-b">
          <div>
            <h1 className="page-title text-gradient">{t('ddns')}</h1>
            <p className="page-subtitle">{t('ddnsPageSubtitle')}</p>
          </div>
          <div className="flex items-c gap-2">
            <button className="btn btn-ghost btn-sm" onClick={handleTest} disabled={testRunning} aria-label={t('testConnection')}>
              <Zap size={16} />
              <span>{testRunning ? t('testConnecting') : t('testConnectionShort')}</span>
            </button>
            <button className="btn btn-primary" onClick={addConfig} aria-label={t('addDdns')}>
              <Plus size={18} />
              <span>{t('addDdns')}</span>
            </button>
          </div>
        </div>
      </header>
      
      {configs.map((cfg, idx) => {
        const currentFields = PROVIDER_FIELDS[cfg.ddns_provider] || [];
        return (
        <div key={cfg.id} className="glass-panel glass-card" style={{
          marginBottom: 12,
          ...(cfg.isNew ? { borderLeft: '3px solid var(--accent)', background: 'rgba(108,142,255,0.04)' } : {})
        }}>
          <div className="flex items-c gap-3 mb-4" style={{ justifyContent: 'space-between' }}>
            <div className="flex items-c gap-3">
              <Globe className="text-success" />
              <h3>{t('ddnsSettings')} {idx === 0 ? t('default') : idx + 1}</h3>
              {cfg.isNew && <span className="badge" style={{ background: 'rgba(108,142,255,0.2)', color: 'var(--accent)', fontSize: '0.65rem' }}>{t('newBadge')}</span>}
            </div>
            {configs.length > 1 && idx > 0 && (
              <button className="btn btn-ghost btn-icon btn-sm" onClick={() => removeConfig(cfg.id, cfg.ddns_domains)} aria-label={t('delete')}>
                <Trash2 size={14} />
              </button>
            )}
          </div>

          <div className="form-toggle mb-4">
            <div>
              <div className="font-600">{t('enableDdns')}</div>
              <p className="text-muted text-sm">{t('ddnsAutoUpdateHint')}</p>
            </div>
            <label className="toggle" aria-label={t('enableDdns')}>
              <input type="checkbox" checked={cfg.ddns_enabled} onChange={e => updateConfig(cfg.id, 'ddns_enabled', e.target.checked)} />
              <span className="toggle-slider"></span>
            </label>
          </div>

          {cfg.ddns_enabled && (
            <>
              <div className="form-group mb-3">
                <label className="form-label">{t('ddnsProvider')}</label>
                <select className="form-input" value={cfg.ddns_provider}
                  onChange={e => handleProviderChange(cfg.id, e.target.value)}>
                  {Object.entries(PROVIDER_LABELS).map(([key, label]) => (
                    <option key={key} value={key}>{label}</option>
                  ))}
                </select>
              </div>

              {/* Dynamic credential fields */}
              {currentFields.map(field => (
                <div className="form-group mb-3" key={field.key}>
                  <label className="form-label">{t(field.label)}</label>
                  <input
                    className="form-input"
                    type={field.type}
                    value={cfg[field.key] || ''}
                    onChange={e => updateConfig(cfg.id, field.key, e.target.value)}
                    placeholder={t(field.placeholder)}
                  />
                </div>
              ))}

              <div className="form-group mb-4">
                <label className="form-label">{t('domains')}</label>
                <textarea className="form-input" rows={4} value={cfg.ddns_domains}
                  onChange={e => updateConfig(cfg.id, 'ddns_domains', e.target.value)}
                  placeholder="example.com&#10;*.example.com" style={{ resize: 'vertical' }} />
              </div>
            </>
          )}
        </div>
      )})}

      <div>
        {domainStatuses.length > 0 && (
          <div className="glass-panel glass-card mb-3">
            <h4 className="font-600 mb-3">{t('domainStatus')}</h4>
            <div className="table-wrap">
              <table style={{ tableLayout: 'fixed', width: '100%' }}>
                <thead className="sticky">
                  <tr>
                    <th style={{ width: '35%', textAlign: 'left', verticalAlign: 'middle' }}>{t('domain')}</th>
                    <th style={{ width: '30%', textAlign: 'left', verticalAlign: 'middle' }}>{t('ipAddress')}</th>
                    <th style={{ width: '15%', textAlign: 'center' }}>{t('recordType')}</th>
                    <th style={{ width: '20%', textAlign: 'center' }}>{t('perDomainToggle')}</th>
                  </tr>
                </thead>
                <tbody>
                  {domainStatuses.map((d) => (
                      <tr key={d.domain}>
                        <td style={{ textAlign: 'left', verticalAlign: 'middle' }}><span className="mono">{d.domain}</span></td>
                        <td style={{ textAlign: 'left', verticalAlign: 'middle' }}><span className="mono">{d.ipv4}{d.ipv6 ? ` / ${d.ipv6}` : ''}</span></td>
                        <td style={{ textAlign: 'center', verticalAlign: 'middle' }}><span className="badge badge-success">{d.ipv6 ? 'A/AAAA' : 'A'}</span></td>
                        <td style={{ textAlign: 'center', verticalAlign: 'middle' }}>
                          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                            <label className="toggle" aria-label={d.domain}>
                              <input type="checkbox" checked={d.enabled} onChange={() => toggleDomain(d.domain)} />
                              <span className="toggle-slider"></span>
                            </label>
                            <button className="btn btn-ghost btn-icon btn-sm" onClick={() => deleteDomain(d.domain)} aria-label={t('deleteDomainTitle', { domain: d.domain })} title={t('deleteDomainTitle', { domain: d.domain })}>
                              <Trash2 size={14} />
                            </button>
                          </div>
                        </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        <div style={{ display: 'flex', gap: 12 }}>
          <button className="btn btn-primary" onClick={handleSave} disabled={saving} aria-label={t('save')}>
            <Save size={16} />
            <span>{t('save')}</span>
          </button>
        </div>

      </div>
      {/* ── Test Connection Log Modal ── */}
      {showTestModal && (
        <div className="modal-overlay">
          <div className="modal glass" style={{ maxWidth: 520 }}>
            <header className="modal-header">
              <h2 className="modal-title" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Zap size={18} />
                DDNS {t('testConnection')}
              </h2>
            </header>
            <div className="modal-body" style={{ padding: '0 16px 16px' }}>
              <div style={{
                background: 'var(--bg-secondary, #1a1a2e)',
                color: '#e0e0e0',
                borderRadius: 8, padding: 12,
                minHeight: 140, maxHeight: 320, overflowY: 'auto',
                fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
                fontSize: 14, lineHeight: 1.7,
              }}>
                {testLogs.map((log, i) => (
                  <div key={i} style={logStyle(log.level)}>
                    <span style={{ opacity: 0.4 }}>{log.timestamp}</span>
                    {' '}{logIcon(log.level)}{' '}{log.message}
                  </div>
                ))}
                {testRunning && (
                  <div style={{ opacity: 0.3, marginTop: 4 }}>...</div>
                )}
              </div>
              <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 12 }}>
                <button className="btn btn-ghost btn-sm" onClick={() => setShowTestModal(false)} disabled={testRunning}>
                  <X size={16} />
                  {t('close')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default Ddns;
