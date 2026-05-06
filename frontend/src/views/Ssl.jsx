import React, { useState, useEffect, useRef } from 'react';
import { useStore } from '../store/useStore';
import { useToast } from '../components/Toast';
import { useTranslation } from 'react-i18next';
import { Shield, Plus, RefreshCw, Trash2, Terminal, Upload, Download } from 'lucide-react';
import { api } from '../api/client';
import { logStyle, logIcon } from '../utils/logHelpers';

const Ssl = () => {
  const { t } = useTranslation();
  const { addToast } = useToast();
  const { settings, fetchSettings } = useStore();
  const [certs, setCerts] = useState([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [form, setForm] = useState({ domain: '', sans: '', method: 'dns01', cf_token: '', email: '' });

  // Issue progress tracking
  const [issuing, setIssuing] = useState(false);
  const [issueJobId, setIssueJobId] = useState(null);
  const [issueLogs, setIssueLogs] = useState([]);
  const [issueStatus, setIssueStatus] = useState(null); // 'running' | 'completed' | 'failed'
  const [issueError, setIssueError] = useState(null);
  const pollRef = useRef(null);
  const notifiedRef = useRef(false); // prevent duplicate toast

  useEffect(() => { fetchSettings(); }, []);
  useEffect(() => { fetchCerts(); }, []);

  const fetchCerts = async () => {
    setLoading(true);
    try {
      const { data } = await api.listCertificates();
      setCerts(data);
    } catch (e) {
      addToast(t('failed'), 'error');
    }
    setLoading(false);
  };

  const handleRenew = async () => {
    try {
      await api.renewSsl();
      addToast(t('certRenewed'), 'success');
      await fetchCerts();
    } catch (e) {
      addToast(e.response?.data?.error || t('failed'), 'error');
    }
  };

  const openModal = () => {
    setForm({ domain: '', sans: '', method: 'dns01', cf_token: settings?.ddns_cf_token || '', email: settings?.acme_email || '' });
    setShowModal(true);
  };

  const handleIssue = async (e) => {
    e.preventDefault();
    try {
      const { data } = await api.issueCertificate({
        domain: form.domain,
        sans: form.sans || undefined,
        method: form.method,
        cf_token: form.method === 'dns01' ? (form.cf_token || undefined) : undefined,
        email: form.email || undefined,
      });
      setShowModal(false);
      setIssueJobId(data.job_id);
      notifiedRef.current = false;
      setIssueLogs([]);
      setIssueStatus('running');
      setIssueError(null);
      setIssuing(true);
    } catch (e) {
      addToast(e.response?.data?.error || t('failed'), 'error');
    }
  };

  // Poll for issue progress
  useEffect(() => {
    if (!issuing || !issueJobId) return;
    const poll = async () => {
      try {
        const { data } = await api.certificateStatus(issueJobId);
        setIssueLogs(prev => {
          const seen = new Set(prev.map(l => l.timestamp + l.message));
          const fresh = data.logs.filter(l => !seen.has(l.timestamp + l.message));
          return [...prev, ...fresh];
        });
        setIssueStatus(data.status);
        if (data.error) setIssueError(data.error);
        if (data.status !== 'running') {
          if (!notifiedRef.current) {
            notifiedRef.current = true;
            if (data.status === 'completed') {
              addToast(t('certIssuanceCompleted'), 'success');
              await fetchCerts();
            } else {
              addToast(t('certIssuanceFailed'), 'error');
            }
          }
        }
      } catch (e) {
        // ignore polling errors
      }
    };
    poll(); // immediate first poll
    pollRef.current = setInterval(poll, 1000);
    return () => {
      if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; }
    };
  }, [issuing, issueJobId]);

  const closeIssueLog = () => {
    setIssuing(false);
    setIssueJobId(null);
    setIssueLogs([]);
    setIssueStatus(null);
    setIssueError(null);
  };

  // Upload modal state
  const [showUpload, setShowUpload] = useState(false);
  const [uploadForm, setUploadForm] = useState({ domain: '', cert_pem: '', key_pem: '' });

  const handleUpload = async (e) => {
    e.preventDefault();
    try {
      await api.uploadCertificate(uploadForm);
      addToast(t('certUploaded'), 'success');
      setShowUpload(false);
      setUploadForm({ domain: '', cert_pem: '', key_pem: '' });
      await fetchCerts();
    } catch (e) {
      addToast(e.response?.data?.error || t('failed'), 'error');
    }
  };

  const handleDownload = async (cert) => {
    try {
      const { data } = await api.downloadCertificate(cert.id);
      const blob = new Blob([
        data.cert_pem, '\n', data.key_pem
      ], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${cert.domain}.pem`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      addToast(t('failed'), 'error');
    }
  };

  const toggleAutoRenew = async (cert) => {
    try {
      await api.updateCertificate(cert.id, { auto_renew: !cert.auto_renew });
      await fetchCerts();
    } catch (e) {
      addToast(t('failed'), 'error');
    }
  };

  const deleteCert = async (cert) => {
    if (!window.confirm(t('confirmDelete'))) return;
    try {
      await api.deleteCertificate(cert.id);
      addToast(t('certRemoved'), 'info');
      await fetchCerts();
    } catch (e) {
      addToast(e.response?.data?.error || t('failed'), 'error');
    }
  };

  const statusLabel = () => {
    if (issueStatus === 'completed') return t('issueCompleted');
    if (issueStatus === 'failed') return t('issueFailed');
    return t('issuing');
  };

  return (
    <div className="fade-in">
      <header className="page-header flex justify-b items-c">
        <div>
          <h1 className="page-title text-gradient">{t('ssl')}</h1>
          <p className="page-subtitle">{t('sslPageSubtitle')}</p>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="btn btn-ghost" onClick={() => { setShowUpload(true); setUploadForm({ domain: '', cert_pem: '', key_pem: '' }); }} aria-label={t('uploadCert')}>
            <Upload size={16} />
            <span>{t('uploadCert')}</span>
          </button>
          <button className="btn btn-primary" onClick={openModal} aria-label={t('add')}>
            <Plus size={18} />
            <span>{t('add')}</span>
          </button>
        </div>
      </header>

      <div className="glass-panel glass-card">
        <div className="flex items-c gap-3 mb-4">
          <Shield className="text-accent" />
          <h3>{t('sslExistingCerts')}</h3>
        </div>

        {loading ? (
          <div className="text-center py-8"><div className="spinner" style={{margin:'0 auto'}} /></div>
        ) : certs.length === 0 ? (
          <div className="empty-state">
            <span className="text-muted">{t('noCertificatesConfigured')}</span>
          </div>
        ) : (
          <div className="table-wrap">
            <table className="centered">
              <thead className="sticky">
                <tr>
                  <th>{t('certDomain')}</th>
                  <th className="hide-mobile">{t('expires')}</th>
                  <th>{t('status')}</th>
                  <th>{t('autoRenew')}</th>
                  <th>{t('actions')}</th>
                </tr>
              </thead>
              <tbody>
                {certs.map(cert => (
                  <tr key={cert.id}>
                    <td><span className="mono">{cert.domain}</span></td>
                    <td className="hide-mobile"><span className="text-sm">{cert.expires_at ? new Date(cert.expires_at).toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' }) : '-'}</span></td>
                    <td style={{ textAlign: 'center' }}>
                      <span className={`badge ${cert.status === 'expired' ? 'badge-danger' : 'badge-success'}`}>
                        <span className="pulse-dot" />
                        {cert.status === 'expired' ? t('certExpired') : t('certValid')}
                      </span>
                    </td>
                    <td>
                      <label className="toggle toggle-sm" aria-label={t('autoRenew')}>
                        <input type="checkbox" checked={cert.auto_renew} onChange={() => toggleAutoRenew(cert)} />
                        <span className="toggle-slider"></span>
                      </label>
                    </td>
                    <td>
                      <div className="flex items-c gap-2" style={{ justifyContent: 'center' }}>
                        <button className="btn btn-ghost btn-icon btn-sm" onClick={() => handleDownload(cert)} title={t('certDownload')} aria-label={t('certDownload')}>
                          <Download size={14} />
                        </button>
                        <button className="btn btn-ghost btn-icon btn-sm" onClick={handleRenew} title={t('renewNow')} aria-label={t('renewNow')}>
                          <RefreshCw size={14} />
                        </button>
                        <button className="btn btn-ghost btn-icon btn-sm" onClick={() => deleteCert(cert)} aria-label={t('delete')}>
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

      </div>

      {/* ── Issue Progress Log Modal ── */}
      {issuing && (
        <div className="modal-overlay">
          <div className="modal glass" style={{ maxWidth: 560 }}>
            <header className="modal-header">
              <h2 className="modal-title" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <Terminal size={18} />
                {t('issuingCert')}
              </h2>
            </header>
            <div className="modal-body" style={{ padding: '0 16px 16px' }}>
              <div style={{
                background: 'var(--bg-secondary, #1a1a2e)',
                borderRadius: 8,
                padding: 12,
                minHeight: 180,
                maxHeight: 360,
                overflowY: 'auto',
                fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
                fontSize: 14,
                lineHeight: 1.7,
              }}>
                {issueLogs.length === 0 && (
                  <div style={{ opacity: 0.5, textAlign: 'center', paddingTop: 60 }}>
                    ⏳ {t('waitingForLogs')}
                  </div>
                )}
                {issueLogs.map((log, i) => (
                  <div key={i} style={logStyle(log.level)}>
                    <span style={{ opacity: 0.4 }}>{log.timestamp}</span>
                    {' '}{logIcon(log.level)}{' '}{log.message}
                  </div>
                ))}
                {issueLogs.length > 0 && issueStatus === 'running' && (
                  <div style={{ opacity: 0.3, marginTop: 4 }}>...</div>
                )}
              </div>
              {issueError && (
                <div style={{
                  marginTop: 8, padding: '8px 12px',
                  background: 'var(--danger-bg, rgba(239,68,68,0.1))',
                  borderLeft: '3px solid var(--danger)',
                  borderRadius: 4, fontSize: 12,
                  color: 'var(--danger)',
                }}>
                  ❌ {issueError}
                </div>
              )}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 12 }}>
                <span style={{ fontSize: 13, opacity: 0.7 }}>
                  {statusLabel()}
                  {issueStatus === 'running' && <span className="spinner" style={{ width: 14, height: 14, display: 'inline-block', marginLeft: 8, verticalAlign: 'middle' }} />}
                </span>
                <button className="btn btn-ghost btn-sm" onClick={closeIssueLog}>
                  {t('close')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {showModal && (
        <div className="modal-overlay">
          <div className="modal glass">
            <header className="modal-header">
              <h2 className="modal-title">{t('issueCert')}</h2>
            </header>
            <form onSubmit={handleIssue} className="modal-body">
              <div className="form-group">
                <label className="form-label">{t('acmeEmail')}</label>
                <input className="form-input" type="email" value={form.email} onChange={e => setForm({...form, email: e.target.value})} placeholder="admin@example.com" />
                <p style={{fontSize:11,opacity:0.5,margin:'4px 0 0'}}>{t('acmeEmailHint')}</p>
              </div>
              <div className="form-group">
                <label className="form-label">{t('certDomain')}</label>
                <input className="form-input" value={form.domain} onChange={e => setForm({...form, domain: e.target.value})} placeholder="example.com" required />
              </div>
              <div className="form-group">
                <label className="form-label">{t('sansHint')}</label>
                <textarea className="form-input" rows={2} value={form.sans} onChange={e => setForm({...form, sans: e.target.value})} placeholder={t('sansPlaceholder')} style={{resize:'vertical'}} />
              </div>
              <div className="form-group">
                <label className="form-label">{t('verificationMethod')}</label>
                <select className="form-input" value={form.method} onChange={e => {
                  const method = e.target.value;
                  setForm({...form, method, cf_token: method === 'dns01' ? (settings?.ddns_cf_token || '') : ''});
                }}>
                  <option value="dns01">{t('dns01Cloudflare')}</option>
                </select>
              </div>
              {form.method === 'dns01' && (
                <div className="form-group">
                  <label className="form-label">{t('apiToken')}</label>
                  <input className="form-input" value={form.cf_token} readOnly />
                </div>
              )}
              <footer className="modal-footer">
                <button type="button" className="btn btn-ghost" onClick={() => setShowModal(false)}>{t('cancel')}</button>
                <button type="submit" className="btn btn-primary">{t('issueCert')}</button>
              </footer>
            </form>
          </div>
        </div>
      )}

      {/* ── Upload Certificate Modal ── */}
      {showUpload && (
        <div className="modal-overlay">
          <div className="modal glass">
            <header className="modal-header">
              <h2 className="modal-title">{t('uploadCert')}</h2>
            </header>
            <form onSubmit={handleUpload} className="modal-body">
              <div className="form-group">
                <label className="form-label">{t('certDomain')}</label>
                <input className="form-input" value={uploadForm.domain} onChange={e => setUploadForm({...uploadForm, domain: e.target.value})} placeholder="example.com" required />
              </div>
              <div className="form-group">
                <label className="form-label">{t('certPem')}</label>
                <textarea className="form-input mono" rows={6} value={uploadForm.cert_pem} onChange={e => setUploadForm({...uploadForm, cert_pem: e.target.value})} placeholder="-----BEGIN CERTIFICATE-----&#10;..." required style={{fontSize:11,resize:'vertical'}} />
              </div>
              <div className="form-group">
                <label className="form-label">{t('keyPem')}</label>
                <textarea className="form-input mono" rows={6} value={uploadForm.key_pem} onChange={e => setUploadForm({...uploadForm, key_pem: e.target.value})} placeholder="-----BEGIN PRIVATE KEY-----&#10;..." required style={{fontSize:11,resize:'vertical'}} />
              </div>
              <footer className="modal-footer">
                <button type="button" className="btn btn-ghost" onClick={() => setShowUpload(false)}>{t('cancel')}</button>
                <button type="submit" className="btn btn-primary">{t('certUpload')}</button>
              </footer>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export default Ssl;
