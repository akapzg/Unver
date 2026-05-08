import React, { useEffect, useState } from 'react';
import { useStore } from '../store/useStore';
import { useToast } from '../components/Toast';
import { useConfirm } from '../components/ConfirmDialog';
import { useTranslation } from 'react-i18next';
import { api } from '../api/client';
import client from '../api/client';
import { Key, Plus, Trash2, Copy, Check, User, Download, Upload, Server, RefreshCw, Save } from 'lucide-react';

const Settings = () => {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { confirm } = useConfirm();
  const {
    settings, fetchSettings, updateSettings,
    apiKeys, fetchApiKeys, createApiKey, deleteApiKey
  } = useStore();

  const [newKeyName, setNewKeyName] = useState('');
  const [generatedKey, setGeneratedKey] = useState(null);
  const [copied, setCopied] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [apiKeyFull, setApiKeyFull] = useState('');
  const [hoveredKey, setHoveredKey] = useState(false);

  // Account
  const [pwForm, setPwForm] = useState({ current: '', newPw: '', confirm: '' });
  const [pwSaving, setPwSaving] = useState(false);
  const [pwError, setPwError] = useState('');

  // Web panel
  const [panelPort, setPanelPort] = useState(19688);
  const [panelLanOnly, setPanelLanOnly] = useState(false);
  const [trustedProxy, setTrustedProxy] = useState('127.0.0.1, ::1');

  useEffect(() => {
    fetchSettings();
    fetchApiKeys();
  }, [fetchSettings, fetchApiKeys]);

  useEffect(() => {
    if (settings) {
      setPanelPort(settings.web_port || 19688);
      setPanelLanOnly(settings.panel_lan_only === true || settings.web_interface === 'lan');
      setTrustedProxy(settings.trusted_proxy || '127.0.0.1, ::1');
    }
  }, [settings]);

  // ── API Keys ──
  const handleCreateKey = async (e) => {
    e.preventDefault();
    if (!newKeyName) return;
    try {
      const key = await createApiKey(newKeyName);
      setGeneratedKey(key);
      setNewKeyName('');
    } catch (err) {
      addToast(err.response?.data?.error || t('failed'), 'error');
    }
  };

  const handleCreateSingleKey = async () => {
    try {
      const key = await createApiKey('default');
      setApiKeyFull(key.key);
    } catch (err) {
      addToast(err.response?.data?.error || t('failed'), 'error');
    }
  };

  const copyToClipboard = async (text) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      addToast(t('copySuccess'), 'success');
    } catch (_clipboardError) {
      // Fallback for non-HTTPS (IP access): use execCommand
      try {
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.left = '-9999px';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        document.body.removeChild(ta);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
        addToast(t('copySuccess'), 'success');
      } catch (_execError) {
        addToast(t('copyFailed'), 'error');
      }
    }
  };

  const handleDeleteKey = async (key) => {
    if (!await confirm(t('confirmDelete'))) return;
    try {
      deleteApiKey(key.id);
      setApiKeyFull('');
    } catch {
      addToast(t('failed'), 'error');
    }
  };

  // ── Password ──
  const handleChangePassword = async () => {
    setPwError('');
    if (pwForm.newPw.length < 8) { setPwError(t('passwordTooShort')); return; }
    if (pwForm.newPw !== pwForm.confirm) { setPwError(t('passwordMismatch')); return; }
    setPwSaving(true);
    try {
      await api.changePassword({ current_password: pwForm.current, new_password: pwForm.newPw });
      addToast(t('passwordChanged'), 'success');
      setPwForm({ current: '', newPw: '', confirm: '' });
    } catch (err) {
      setPwError(err.response?.data?.error || t('failed'));
    } finally {
      setPwSaving(false);
    }
  };

  // ── Backup ──
  const handleExport = async () => {
    setExporting(true);
    try {
      const { data } = await api.exportConfig();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `unver-backup-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      addToast(t('exportSuccess'), 'success');
    } catch (e) {
      addToast(t('exportFailed'), 'error');
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async (event) => {
      try {
        const json = JSON.parse(event.target.result);
        await api.importConfig(json);
        addToast(t('importSuccess'), 'success');
        setTimeout(() => window.location.reload(), 1500);
      } catch (err) {
        addToast(t('importFailed'), 'error');
      }
    };
    reader.readAsText(file);
  };

  // ── Web Panel ──
  const handleSavePanel = async () => {
    if (panelPort === '' || panelPort < 1 || panelPort > 65535) {
      addToast(t('portInvalid'), 'error');
      return;
    }
    try {
      await updateSettings({ web_port: panelPort, panel_lan_only: panelLanOnly, trusted_proxy: trustedProxy });
      addToast(t('configSaved'), 'success');
    } catch (e) {
      addToast(t('saveFailed'), 'error');
    }
  };

  const handleRestart = async () => {
    if (!await confirm(t('confirmRestart'))) return;
    try {
      await client.post('/system/restart');
      addToast(t('restarting'), 'info');
    } catch (e) {
      addToast(t('restartFailed'), 'error');
    }
  };

  return (
    <div className="fade-in">
      <header className="page-header">
        <h1 className="page-title text-gradient">{t('settings')}</h1>
        <p className="page-subtitle">{t('settingsSubtitle')}</p>
      </header>

      <div className="settings-grid">

        {/* Row 1 Left — Account */}
        <section className="glass-panel glass-card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="flex items-c gap-3 mb-4">
            <User className="text-accent" />
            <h3>{t('accountManagement')}</h3>
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('username')}</label>
            <input className="form-input" value={settings?.username || 'admin'} readOnly />
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('currentPassword')}</label>
            <input type="password" className="form-input" value={pwForm.current}
              onChange={e => setPwForm({...pwForm, current: e.target.value})} />
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('newPassword')}</label>
            <input type="password" className="form-input" value={pwForm.newPw}
              onChange={e => setPwForm({...pwForm, newPw: e.target.value})} />
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('confirmPassword')}</label>
            <input type="password" className="form-input" value={pwForm.confirm}
              onChange={e => setPwForm({...pwForm, confirm: e.target.value})} />
          </div>
          {pwError && <div className="auth-error mb-3">{pwError}</div>}
          <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 'auto' }}>
            <button className="btn btn-primary" onClick={handleChangePassword} disabled={pwSaving} aria-label={t('changePassword')}>
              <Key size={16} />
              <span>{t('changePassword')}</span>
            </button>
          </div>
        </section>

        {/* Row 1 Right — Web Panel Config */}
        <section className="glass-panel glass-card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="flex items-c gap-3 mb-4">
            <Server className="text-accent-2" />
            <h3>{t('webPanelConfig')}</h3>
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('listenPort')}</label>
            <input type="number" className="form-input" value={panelPort}
              onChange={e => { const v = e.target.value.trim(); setPanelPort(v === '' ? '' : parseInt(v)); }} />
          </div>
          <div className="form-group mb-3">
            <div className="flex items-c justify-b">
              <div>
                <label className="form-label" style={{ margin: 0 }}>{t('lanOnlyLabel')}</label>
                <p className="text-muted text-sm" style={{ marginTop: 2 }}>
                  {panelLanOnly ? t('lanOnlyDesc') : t('allInterfacesDesc')}
                </p>
              </div>
              <label className="toggle" aria-label={t('lanOnlyLabel')}>
                <input type="checkbox" checked={panelLanOnly}
                  onChange={e => setPanelLanOnly(e.target.checked)} />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>
          <div className="form-group mb-3">
            <label className="form-label">{t('trustedProxy')}</label>
            <input className="form-input" value={trustedProxy}
              onChange={e => setTrustedProxy(e.target.value)}
              placeholder="127.0.0.1, ::1" />
            <p className="text-muted text-sm" style={{ marginTop: 2 }}>{t('trustedProxyHint')}</p>
          </div>
          <div style={{ marginTop: 'auto' }}>
            <p className="text-muted text-sm mb-2">{t('requireRestart')}</p>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn-primary" onClick={handleSavePanel} aria-label={t('save')}>
                <Save size={16} />
                <span>{t('save')}</span>
              </button>
              <button className="btn btn-ghost" onClick={handleRestart}>
                <RefreshCw size={16} />
                <span>{t('restart')}</span>
              </button>
            </div>
          </div>
        </section>

        {/* Row 2 Left — API Interface + Keys */}
        <section className="glass-panel glass-card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="flex items-c gap-3 mb-2">
            <Key className="text-accent" />
            <h3>{t('apiInterface')}</h3>
          </div>
          <p className="text-muted text-sm mb-3">{t('apiAccessHint')}</p>
          <div className="form-toggle mb-3">
            <div>
              <div className="font-600">{t('enable')}</div>
            </div>
            <label className="toggle" aria-label={t('enableApi')}>
              <input type="checkbox" checked={settings?.api_auth_enabled || false}
                onChange={e => updateSettings({ api_auth_enabled: e.target.checked })} />
              <span className="toggle-slider"></span>
            </label>
          </div>

          <div style={{ marginTop: 'auto' }}>
          {apiKeys.length === 0 ? (
            <div className="flex items-c" style={{ padding: '6px 0' }}>
              <span style={{ fontWeight: 500, flex: 1 }}>{t('apiKeyLabel')}</span>
              <span style={{ flex: 2, textAlign: 'center' }}>
                <span className="text-muted mono">----</span>
                <span className="text-muted text-sm"> ({t('noApiKey')})</span>
              </span>
              <div style={{ flex: 1, display: 'flex', justifyContent: 'flex-end' }}>
                <button className="btn btn-primary btn-sm" onClick={handleCreateSingleKey}>
                  <Plus size={14} /><span>{t('createApiKeyBtn')}</span>
                </button>
              </div>
            </div>
          ) : (
            apiKeys.slice(0, 1).map(key => {
              const displayKey = apiKeyFull || (key.key_prefix || '');
              const masked = displayKey.length > 4
                ? `${displayKey.slice(0, 2)}****${displayKey.slice(-2)}`
                : '****';
              return (
                <div key={key.id} className="flex items-c" style={{ padding: '6px 0' }}>
                  <span style={{ fontWeight: 500, flex: 1 }}>{t('apiKeyLabel')}</span>
                  <span style={{ flex: 2, textAlign: 'center' }}>
                    <span
                      className="mono"
                      style={{ cursor: 'pointer', userSelect: 'all' }}
                      onMouseEnter={() => setHoveredKey(true)}
                      onMouseLeave={() => setHoveredKey(false)}
                    >
                      {hoveredKey ? displayKey : masked}
                    </span>
                  </span>
                  <div style={{ flex: 1, display: 'flex', justifyContent: 'flex-end', gap: 6 }}>
                    <button className="btn btn-ghost btn-icon btn-sm" onClick={() => copyToClipboard(displayKey)} title={t('copy')}>
                      {copied ? <Check size={14} className="text-success" /> : <Copy size={14} />}
                    </button>
                    <button className="btn btn-ghost btn-icon btn-sm" onClick={() => handleDeleteKey(key)} title={t('delete')}>
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              );
            })
          )}
          </div>
        </section>

        {/* Row 2 Right — Backup */}
        <section className="glass-panel glass-card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="flex items-c gap-3 mb-2">
            <Download className="text-warning" />
            <h3>{t('backupRestore')}</h3>
          </div>
          <p className="text-muted text-sm mb-3">{t('exportNote')}</p>
          <div className="flex gap-2" style={{ marginTop: 'auto' }}>
            <button className="btn btn-ghost w-full" onClick={handleExport} disabled={exporting} aria-label={t('exportConfig')}>
              {exporting ? <div className="spinner" /> : <Upload size={16} />}
              <span>{t('exportConfig')}</span>
            </button>
            <label className="btn btn-ghost w-full text-center">
              <Download size={16} />
              <span>{t('importConfig')}</span>
              <input type="file" hidden onChange={handleImport} accept=".json" />
            </label>
          </div>
        </section>

      </div>
    </div>
  );
};

export default Settings;
