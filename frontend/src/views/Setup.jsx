import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useStore } from '../store/useStore';
import { api } from '../api/client';
import { useTranslation } from 'react-i18next';
import { UserPlus } from 'lucide-react';

const Setup = () => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const { setupComplete, checkSetup } = useStore();
  const navigate = useNavigate();
  const { t } = useTranslation();

  useEffect(() => {
    if (setupComplete === true) {
      navigate('/login');
    }
  }, [setupComplete, navigate]);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');

    if (password !== confirmPassword) {
      setError(t('passwordMismatch'));
      return;
    }

    if (password.length < 8) {
      setError(t('passwordTooShort'));
      return;
    }

    setLoading(true);
    try {
      await api.setup({ username, password });
      await checkSetup();
      navigate('/login');
    } catch (err) {
      setError(err.response?.data?.error || t('setupFailed'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="auth-page">
      <div className="auth-card glass">
        <div className="auth-logo"><img src="/favicon.png" alt="Unver" style={{ height: 64, width: 'auto' }} /></div>
        <p className="auth-subtitle">{t('setupTitle')}</p>
        
        {error && <div className="auth-error">{error}</div>}
        
        <form className="auth-form" onSubmit={handleSubmit}>
          <div className="form-group">
            <label className="form-label">{t('adminUsername')}</label>
            <input
              type="text"
              className="form-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={t('adminPlaceholder')}
              required
              aria-label={t('adminUsername')}
            />
          </div>
          <div className="form-group">
            <label className="form-label">{t('loginPassword')}</label>
            <input
              type="password"
              className="form-input"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('setupPwPlaceholder')}
              required
              aria-label={t('loginPassword')}
            />
          </div>
          <div className="form-group">
            <label className="form-label">{t('confirmPassword')}</label>
            <input
              type="password"
              className="form-input"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={t('confirmPwPlaceholder')}
              required
              aria-label={t('confirmPassword')}
            />
          </div>
          <button type="submit" className="btn btn-primary w-full mt-4" disabled={loading} aria-label={t('setupButton')}>
            {loading ? <div className="spinner" /> : <><UserPlus size={16} />{t('setupButton')}</>}
          </button>
        </form>
      </div>
    </div>
  );
};

export default Setup;
