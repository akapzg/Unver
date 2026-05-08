import React, { useEffect, useState } from 'react';
import { useStore } from '../store/useStore';
import { useTranslation } from 'react-i18next';
import { useToast } from '../components/Toast';
import { useConfirm } from '../components/ConfirmDialog';
import { Plus, Trash2, Edit3, ExternalLink, Globe, ChevronDown, ChevronUp, Copy, Save, Check, X } from 'lucide-react';
import client from '../api/client';

const Proxies = () => {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { confirm } = useConfirm();
  const { fetchProxies, createProxy, updateProxy, deleteProxy } = useStore();
  const [portGroups, setPortGroups] = useState([]);
  const [expanded, setExpanded] = useState({});
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');

  // Modal state
  const [showPgModal, setShowPgModal] = useState(false);
  const [showRuleModal, setShowRuleModal] = useState(false);
  const [pgForm, setPgForm] = useState({ name: '', listen_port: 8443, enabled: true, skip_tls_verify: false, force_https: false });
  const [editingPgId, setEditingPgId] = useState(null);
  const [ruleForm, setRuleForm] = useState({ name: '', domain: '', target_url: '', rule_type: 'proxy', redirect_code: 301, ssl_enabled: false, cert_id: '', port_group_id: '' });
  const [editingRuleId, setEditingRuleId] = useState(null);
  const [certificates, setCertificates] = useState([]);

  const loadData = async () => {
    setLoading(true);
    try {
      const pgRes = await client.get('/port-groups');
      await fetchProxies();
      const certRes = await client.get('/certificates');

      const groups = pgRes.data || [];
      const allProxies = useStore.getState().proxies || [];
      setCertificates(certRes.data || []);

      // Group proxies by port_group_id instead of broken /port-groups/:id/rules
      const groupsWithRules = groups.map(g => ({
        ...g,
        rules: allProxies.filter(p => p.port_group_id === g.id)
      }));

      setPortGroups(groupsWithRules);
      // Auto-expand all
      const exp = {};
      groupsWithRules.forEach(g => { exp[g.id] = true; });
      setExpanded(exp);
    } catch (e) {
      console.error('Failed to load port groups:', e);
    }
    setLoading(false);
  };

  useEffect(() => { loadData(); }, []);

  const toggleExpand = (id) => {
    setExpanded(prev => ({ ...prev, [id]: !prev[id] }));
  };

  // ── Port Group CRUD ──
  const openPgModal = (pg = null) => {
    if (pg) {
      setPgForm({ name: pg.name, listen_port: pg.listen_port, enabled: pg.enabled, skip_tls_verify: pg.skip_tls_verify, force_https: pg.force_https });
      setEditingPgId(pg.id);
    } else {
      setPgForm({ name: '', listen_port: 8443, enabled: true, skip_tls_verify: false, force_https: false });
      setEditingPgId(null);
    }
    setShowPgModal(true);
  };

  const handlePgSubmit = async (e) => {
    e.preventDefault();
    try {
      if (editingPgId) {
        await client.patch(`/port-groups/${editingPgId}`, pgForm);
        addToast(t('portGroupUpdated'), 'success');
      } else {
        await client.post('/port-groups', pgForm);
        addToast(t('portGroupCreated'), 'success');
      }
      setShowPgModal(false);
      loadData();
    } catch (err) {
      const raw = err?.response?.data?.error || err?.response?.data?.message || err?.response?.data || err?.message || '';
      let msg = raw;
      const m = typeof raw === 'string' ? raw.match(/^Port (\d+) is already in use$/) : null;
      if (m) msg = t('portAlreadyInUse', { port: m[1] });
      const m2 = typeof raw === 'string' ? raw.match(/^Port (\d+) is occupied by another process$/) : null;
      if (m2) msg = t('portOccupied', { port: m2[1] });
      addToast(msg || t('failed'), 'error');
    }
  };

  const handlePgDelete = async (id) => {
    if (!await confirm(t('deletePortGroupConfirm'))) return;
    try {
      await client.delete(`/port-groups/${id}`);
      addToast(t('portGroupDeleted'), 'success');
      loadData();
    } catch { addToast(t('failed'), 'error'); }
  };

  // ── Rule CRUD ──
  const openRuleModal = (pgId, rule = null) => {
    if (rule) {
      setRuleForm({ name: rule.name, domain: rule.domain, target_url: rule.target_url, rule_type: rule.rule_type || 'proxy', redirect_code: rule.redirect_code || 301, ssl_enabled: rule.ssl_enabled, cert_id: rule.cert_id || '', port_group_id: pgId });
      setEditingRuleId(rule.id);
    } else {
      setRuleForm({ name: '', domain: '', target_url: '', rule_type: 'proxy', redirect_code: 301, ssl_enabled: false, cert_id: '', port_group_id: pgId });
      setEditingRuleId(null);
    }
    setShowRuleModal(true);
  };

  const handleRuleSubmit = async (e) => {
    e.preventDefault();
    try {
      if (editingRuleId) {
        await updateProxy(editingRuleId, ruleForm);
        addToast(t('proxyUpdated'), 'success');
      } else {
        await createProxy(ruleForm);
        addToast(t('proxyCreated'), 'success');
      }
      setShowRuleModal(false);
      loadData();
    } catch (err) {
      const raw = err?.response?.data?.error || err?.response?.data?.message || err?.response?.data || err?.message || '';
      let msg = raw;
      const m = typeof raw === 'string' ? raw.match(/^Port (\d+) is already in use$/) : null;
      if (m) msg = t('portAlreadyInUse', { port: m[1] });
      const m2 = typeof raw === 'string' ? raw.match(/^Port (\d+) is occupied by another process$/) : null;
      if (m2) msg = t('portOccupied', { port: m2[1] });
      addToast(msg || t('failed'), 'error');
    }
  };

  const handleRuleDelete = async (id) => {
    if (!await confirm(t('confirmDelete'))) return;
    try {
      await deleteProxy(id);
      addToast(t('proxyDeleted'), 'success');
      loadData();
    } catch (_) {
      addToast(t('failed'), 'error');
    }
  };

  const toggleRuleStatus = async (rule) => {
    try {
      await updateProxy(rule.id, { enabled: !rule.enabled });
      loadData();
    } catch {
      addToast(t('failed'), 'error');
    }
  };

  const toggleGroupStatus = async (pg) => {
    try {
      await client.patch(`/port-groups/${pg.id}`, { enabled: !pg.enabled });
      addToast(pg.enabled ? t('portGroupDisabled') : t('portGroupEnabled'), 'success');
      loadData();
    } catch { addToast(t('failed'), 'error'); }
  };

  const getRuleUrl = (pg, rule) => {
    const isSsl = rule.ssl_enabled || pg.listen_port === 443;
    const protocol = isSsl ? 'https' : 'http';
    const defaultPort = isSsl ? 443 : 80;
    const portStr = pg.listen_port === defaultPort ? '' : `:${pg.listen_port}`;
    return `${protocol}://${rule.domain}${portStr}`;
  };

  const copyUrl = (pg, rule) => {
    const url = getRuleUrl(pg, rule);
    const doToast = (msg, type) => addToast(msg, type);
    // Try modern clipboard API first
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(url).then(
        () => doToast(t('copiedToClipboard'), 'success'),
        () => fallbackCopy(url, doToast)
      );
    } else {
      fallbackCopy(url, doToast);
    }
  };
  const fallbackCopy = (text, doToast) => {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.left = '-9999px';
    ta.style.top = '-9999px';
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    try {
      document.execCommand('copy');
      doToast(t('copiedToClipboard'), 'success');
    } catch (e) {
      doToast(t('copyFailed'), 'error');
    }
    document.body.removeChild(ta);
  };

  // Filter
  const filteredGroups = portGroups.filter(g => {
    if (!search) return true;
    const s = search.toLowerCase();
    return g.name.toLowerCase().includes(s) ||
      String(g.listen_port).includes(s) ||
      (Array.isArray(g.rules) ? g.rules : []).some(r => r.domain.toLowerCase().includes(s) || r.name.toLowerCase().includes(s));
  });

  return (
    <div className="fade-in">
      <header className="page-header flex justify-b items-c">
        <div>
          <h1 className="page-title text-gradient">{t('proxyRules')}</h1>
          <p className="page-subtitle">{t('proxyRulesSubtitle')}</p>
        </div>
        <div className="flex items-c gap-2">
          <input
            className="search-input"
            placeholder={t('searchPlaceholder')}
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
          <button className="btn btn-primary" onClick={() => openPgModal()}>
            <Plus size={18} />
            <span>{t('newGroup')}</span>
          </button>
        </div>
      </header>

      {loading ? (
        <div className="text-center py-8"><div className="spinner" style={{ margin: '0 auto' }} /></div>
      ) : filteredGroups.length === 0 ? (
        <div className="empty-state mt-4">
          <Globe size={40} />
          <span>{t('noPortGroups')}</span>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12, marginTop: 16 }}>
          {filteredGroups.map(pg => (
            <div key={pg.id} className="glass-panel" style={{ borderRadius: 'var(--radius-md)' }}>
              {/* Group Header */}
              <div
                className="flex items-c justify-b proxy-group-header"
                style={{ padding: '12px 16px', cursor: 'pointer', userSelect: 'none' }}
                onClick={() => toggleExpand(pg.id)}
              >
                <div className="flex items-c gap-3">
                  <Globe size={18} className="text-accent" />
                  <div>
                    <div className="flex items-c" style={{ fontWeight: 600, fontSize: '0.9rem', gap: 6 }}>
                      {pg.name}
                      <span style={{
                        padding: '2px 8px',
                        borderRadius: 'var(--radius-sm)',
                        background: 'var(--accent)',
                        color: '#fff',
                        fontSize: '0.8rem',
                        fontWeight: 700,
                      }}>{pg.listen_port}</span>
                      <label className="toggle" aria-label={pg.enabled ? t('disable') : t('enable')} onClick={e => e.stopPropagation()}>
                        <input type="checkbox" checked={pg.enabled} onChange={() => toggleGroupStatus(pg)} />
                        <span className="toggle-slider"></span>
                      </label>
                    </div>

                  </div>
                </div>
                <div className="flex items-c gap-2">
                  <div className="flex items-c gap-2" onClick={e => e.stopPropagation()}>
                  <button className="btn btn-primary" onClick={() => openRuleModal(pg.id)}>
                    <Plus size={14} /> {t('addSubRule')}
                  </button>
                  <button className="btn btn-ghost btn-icon" onClick={() => openPgModal(pg)}><Edit3 size={14} /></button>
                  <button className="btn btn-ghost btn-icon" onClick={() => handlePgDelete(pg.id)}><Trash2 size={14} /></button>
                  </div>
                  {expanded[pg.id] ? <ChevronUp size={16} className="text-muted" /> : <ChevronDown size={16} className="text-muted" />}
                </div>
              </div>

              {/* Rules Table */}
              {expanded[pg.id] && (
                <div style={{ borderTop: '1px solid var(--glass-border)' }}>
                  {(Array.isArray(pg.rules) ? pg.rules : []).length === 0 ? (
                    <div className="text-center py-4 text-muted text-sm">{t('noRulesInGroup')}</div>
                  ) : (
                    <div className="table-wrap">
                      <table className="proxy-table">
                        <thead>
                          <tr>
                            <th>{t('type')}</th>
                            <th>{t('name')}</th>
                            <th>{t('domain')}</th>
                            <th>{t('target')}</th>
                            <th className="hide-mobile">{t('health')}</th>
                            <th>{t('status')}</th>
                            <th className="hide-mobile">{t('ssl')}</th>
                            <th>{t('actions')}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {pg.rules.map(rule => (
                            <tr key={rule.id} className={!pg.enabled ? 'opacity-50' : ''}>
                              <td className="text-sm">
                                {rule.rule_type === 'redirect' ? t('ruleTypeRedirect') : rule.rule_type === 'tcp' ? t('ruleTypeTcp') : t('ruleTypeProxy')}
                              </td>
                              <td><div className="font-600">{rule.name}</div></td>
                              <td>
                                <div className="flex items-c gap-2" style={{ justifyContent: 'center' }}>
                                  <span className="mono">{rule.domain}</span>
                                  <a href={getRuleUrl(pg, rule)} target="_blank" rel="noreferrer" className="text-accent"><ExternalLink size={14} /></a>
                                  <button className="btn btn-ghost btn-icon" onClick={() => copyUrl(pg, rule)} title={t('copyAddress')}><Copy size={14} /></button>
                                </div>
                              </td>
                              <td><span className="mono text-muted">{rule.target_url}</span></td>
                              <td className="hide-mobile">
                                <span className={`badge ${rule.status === 'online' ? 'badge-success' : rule.status === 'error' ? 'badge-warn' : 'badge-danger'}`}>
                                  {(rule.status || 'unknown').toUpperCase()}
                                </span>
                              </td>
                              <td>
                                <label className="toggle" aria-label={rule.enabled ? t('active') : t('disabled')}>
                                  <input type="checkbox" checked={rule.enabled} onChange={() => toggleRuleStatus(rule)} disabled={!pg.enabled} />
                                  <span className="toggle-slider"></span>
                                </label>
                              </td>
                              <td className="hide-mobile">
                                {rule.ssl_enabled ? <span className="badge badge-success">SSL</span> : <span className="text-muted text-sm">HTTP</span>}
                              </td>
                              <td>
                                <div className="flex items-c gap-2" style={{ justifyContent: 'center' }}>
                                  <button className="btn btn-ghost btn-icon" onClick={() => openRuleModal(pg.id, rule)} disabled={!pg.enabled}><Edit3 size={14} /></button>
                                  <button className="btn btn-ghost btn-icon" onClick={() => handleRuleDelete(rule.id)} disabled={!pg.enabled}><Trash2 size={14} /></button>
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Port Group Modal */}
      {showPgModal && (
        <div className="modal-overlay">
          <div className="modal glass">
            <header className="modal-header">
              <h2 className="modal-title">{editingPgId ? t('editPortGroup') : t('newPortGroup')}</h2>
            </header>
            <form onSubmit={handlePgSubmit} className="modal-body">
              <div className="form-group">
                <label className="form-label">{t('name')}</label>
                <input className="form-input" placeholder="e.g. Web 服务" value={pgForm.name}
                  onChange={e => setPgForm({ ...pgForm, name: e.target.value })} required />
              </div>
              <div className="form-group">
                <label className="form-label">{t('listenPort')}</label>
                <input className="form-input" type="number" min={1} max={65535} value={pgForm.listen_port}
                  onChange={e => setPgForm({ ...pgForm, listen_port: e.target.value === '' ? '' : parseInt(e.target.value) })} required />
              </div>
              <div className="form-group">
                <label className="form-label" style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
                  <input type="checkbox" checked={pgForm.force_https || false}
                    onChange={e => setPgForm({ ...pgForm, force_https: e.target.checked })} />
                  <span>{t('forceHttps')}</span>
                </label>
                <p className="form-hint" style={{ marginTop: 4 }}>{t('forceHttpsDesc')}</p>
              </div>
              <div className="form-group">
                <label className="form-label" style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
                  <input type="checkbox" checked={pgForm.skip_tls_verify || false}
                    onChange={e => setPgForm({ ...pgForm, skip_tls_verify: e.target.checked })} />
                  <span>{t('skipTlsVerify')}</span>
                </label>
                <p className="form-hint" style={{ marginTop: 4 }}>{t('skipTlsVerifyDesc')}</p>
              </div>
              <footer className="modal-footer">
                <button type="button" className="btn btn-ghost" onClick={() => setShowPgModal(false)}><X size={16} />{t('cancel')}</button>
                <button type="submit" className="btn btn-primary">{editingPgId ? <><Check size={16} />{t('update')}</> : <><Plus size={16} />{t('create')}</>}</button>
              </footer>
            </form>
          </div>
        </div>
      )}

      {/* Rule Modal */}
      {showRuleModal && (
        <div className="modal-overlay">
          <div className="modal glass">
            <header className="modal-header">
              <h2 className="modal-title">{editingRuleId ? t('editSubRule') : t('newSubRule')}</h2>
            </header>
            <form onSubmit={handleRuleSubmit} className="modal-body">
              <div className="form-group">
                <label className="form-label">{t('type')}</label>
                <select className="form-input" value={ruleForm.rule_type}
                  onChange={e => setRuleForm({ ...ruleForm, rule_type: e.target.value })}>
                  <option value="proxy">{t('proxyTypeReverse')}</option>
                  <option value="redirect">{t('proxyTypeRedirect')}</option>
                  <option value="tcp">{t('proxyTypeTcp')}</option>
                </select>
              </div>
              <div className="form-group">
                <label className="form-label">{t('friendlyName')}</label>
                <input className="form-input" placeholder="e.g. Home Assistant" value={ruleForm.name}
                  onChange={e => setRuleForm({ ...ruleForm, name: e.target.value })} required />
              </div>
              <div className="form-group">
                <label className="form-label">{t('domainName')}</label>
                <input className="form-input" placeholder="e.g. hass.example.com" value={ruleForm.domain}
                  onChange={e => setRuleForm({ ...ruleForm, domain: e.target.value })} required />
              </div>
              {ruleForm.rule_type === 'proxy' && (
                <>
                  <div className="form-group">
                    <label className="form-label">{t('targetUrl')}</label>
                    <input className="form-input" placeholder="e.g. http://192.168.1.100:8123" value={ruleForm.target_url}
                      onChange={e => setRuleForm({ ...ruleForm, target_url: e.target.value })} required />
                  </div>
                  <div className="form-toggle">
                    <span className="text-muted text-sm">{t('enableSsl')}</span>
                    <label className="toggle" aria-label={t('enableSsl')}>
                      <input type="checkbox" checked={ruleForm.ssl_enabled}
                        onChange={e => setRuleForm({ ...ruleForm, ssl_enabled: e.target.checked })} />
                      <span className="toggle-slider"></span>
                    </label>
                  </div>
                  {ruleForm.ssl_enabled && certificates.length > 0 && (
                    <div className="form-group">
                      <label className="form-label">{t('selectCert')}</label>
                      <select className="form-input" value={ruleForm.cert_id}
                        onChange={e => setRuleForm({ ...ruleForm, cert_id: e.target.value })}>
                        <option value="">{t('autoMatch')}</option>
                        {certificates.map(c => (
                          <option key={c.id} value={c.id}>{c.domain}{c.source === 'manual' ? ' (📤)' : ''}</option>
                        ))}
                      </select>
                    </div>
                  )}
                </>
              )}
              {ruleForm.rule_type === 'redirect' && (
                <>
                  <div className="form-group">
                    <label className="form-label">{t('redirectTargetUrl')}</label>
                    <input className="form-input" placeholder="e.g. https://new.example.com" value={ruleForm.target_url}
                      onChange={e => setRuleForm({ ...ruleForm, target_url: e.target.value })} required />
                  </div>
                  <div className="form-group">
                    <label className="form-label">{t('statusCode')}</label>
                    <select className="form-input" value={ruleForm.redirect_code || 301}
                      onChange={e => setRuleForm({ ...ruleForm, redirect_code: e.target.value === '' ? '' : parseInt(e.target.value) })}>
                      <option value={301}>{t('redirect301')}</option>
                      <option value={302}>{t('redirect302')}</option>
                      <option value={307}>{t('redirect307')}</option>
                      <option value={308}>{t('redirect308')}</option>
                    </select>
                  </div>
                </>
              )}
              {ruleForm.rule_type === 'tcp' && (
                <div className="form-group">
                  <label className="form-label">{t('backendAddress')}</label>
                  <input className="form-input" placeholder="e.g. 10.0.0.5:3306" value={ruleForm.target_url}
                    onChange={e => setRuleForm({ ...ruleForm, target_url: e.target.value })} required />
                </div>
              )}
              <footer className="modal-footer mt-2">
                <button type="button" className="btn btn-ghost" onClick={() => setShowRuleModal(false)}><X size={16} />{t('cancel')}</button>
                <button type="submit" className="btn btn-primary">{editingRuleId ? <><Check size={16} />{t('updateProxy')}</> : <><Save size={16} />{t('save')}</>}</button>
              </footer>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export default Proxies;
