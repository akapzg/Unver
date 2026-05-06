import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useStore } from '../store/useStore';
import { useTheme } from '../store/ThemeContext';
import { useTranslation } from 'react-i18next';
import { Sun, Moon, Languages } from 'lucide-react';

const Login = () => {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const { login } = useStore();
  const navigate = useNavigate();
  const { t, i18n } = useTranslation();
  const { isDark, toggle: toggleTheme } = useTheme();

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      await login(username, password);
      navigate('/dashboard');
    } catch (err) {
      setError(err.response?.data?.error || t('loginFailed'));
    } finally {
      setLoading(false);
    }
  };

  const toggleLang = () => {
    const next = i18n.language === 'zh' ? 'en' : 'zh';
    i18n.changeLanguage(next);
    localStorage.setItem('unver-lang', next);
  };

  return (
    <div className="auth-page">
      <div className="auth-card glass">
        <div className="auth-logo"><img src="/favicon.png" alt="Unver" style={{ height: 64, width: 'auto' }} /></div>
        <p className="auth-subtitle">{t('loginTitle')}</p>
        
        {error && <div className="auth-error">{error}</div>}
        
        <form className="auth-form" onSubmit={handleSubmit}>
          <div className="form-group">
            <label className="form-label">{t('loginUsername')}</label>
            <input
              type="text"
              className="form-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={t('usernamePlaceholder')}
              required
              aria-label={t('loginUsername')}
            />
          </div>
          <div className="form-group">
            <label className="form-label">{t('loginPassword')}</label>
            <input
              type="password"
              className="form-input"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('passwordPlaceholder')}
              required
              aria-label={t('loginPassword')}
            />
          </div>
          <button type="submit" className="btn btn-primary w-full mt-4" disabled={loading} aria-label={t('loginButton')}>
            {loading ? <div className="spinner" /> : t('loginButton')}
          </button>
        </form>

        <div className="login-controls" style={{ marginTop: 'clamp(24px, 6vh, 48px)' }}>
          <button onClick={toggleTheme} className="sidebar-ctrl-btn" aria-label={isDark ? t('lightMode') || 'Light mode' : t('darkMode') || 'Dark mode'}>
            {isDark ? <Sun size={20} /> : <Moon size={20} />}
          </button>
          <button onClick={toggleLang} className="sidebar-ctrl-btn" aria-label={t('switchLang') || 'Switch language'}>
            <Languages size={20} />
          </button>
        </div>
      </div>
    </div>
  );
};

export default Login;
