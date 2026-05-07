import React, { useEffect, useState } from 'react';
import { Outlet, NavLink, useNavigate } from 'react-router-dom';
import { LayoutDashboard, Globe, Shield, Settings, LogOut, Sun, Moon, Github, Languages, RefreshCw } from 'lucide-react';
import { useStore } from '../store/useStore';
import { useTheme } from '../store/ThemeContext';
import { useTranslation } from 'react-i18next';
import useIdleTimeout from '../hooks/useIdleTimeout';
import { useToast } from './Toast';

const Layout = () => {
  const { logout } = useStore();
  const navigate = useNavigate();
  const { isDark, toggle: toggleTheme } = useTheme();
  const { t, i18n } = useTranslation();
  const { addToast } = useToast();
  const [topBarHidden, setTopBarHidden] = useState(false);

  useEffect(() => {
    let lastY = window.scrollY;
    let ticking = false;
    const onScroll = () => {
      if (!ticking) {
        window.requestAnimationFrame(() => {
          const currentY = window.scrollY;
          if (currentY > lastY && currentY > 60) {
            setTopBarHidden(true);
          } else {
            setTopBarHidden(false);
          }
          lastY = currentY;
          ticking = false;
        });
        ticking = true;
      }
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  const toggleLang = () => {
    const next = i18n.language === 'zh' ? 'en' : 'zh';
    i18n.changeLanguage(next);
    localStorage.setItem('unver-lang', next);
  };

  // Idle timeout: auto-logout after 15 min inactivity (warn 1 min before)
  useIdleTimeout(15, async () => {
    await logout();
    addToast(t('idleLoggedOut'), 'info');
    navigate('/login');
  }, () => {
    addToast(t('idleWarning'), 'warn');
  });

  const handleLogout = async () => {
    try {
      await logout();
      navigate('/login');
    } catch (_) {
      navigate('/login');
    }
  };

  const NavItems = [
    { to: '/dashboard', icon: <LayoutDashboard size={20} />, label: t('dashboard') },
    { to: '/proxies',   icon: <Globe size={20} />,            label: t('proxies') },
    { to: '/ddns',      icon: <RefreshCw size={20} />,        label: t('ddns') },
    { to: '/ssl',       icon: <Shield size={20} />,           label: t('ssl') },
    { to: '/settings',  icon: <Settings size={20} />,         label: t('settings') },
  ];

  return (
    <div className="app-layout">
      <div className={`mobile-top-bar ${topBarHidden ? 'collapsed' : ''}`}>
        <span className="mobile-top-logo"><img src="/favicon.png" alt="Unver" style={{ height: 24, width: 'auto' }} /></span>
        <div className="mobile-top-actions">
          <a href="https://github.com/unver" target="_blank" rel="noreferrer" className="sidebar-ctrl-btn" title="GitHub" aria-label="GitHub">
            <Github size={20} />
          </a>
          <button onClick={toggleTheme} className="sidebar-ctrl-btn" aria-label={isDark ? t('lightMode') || 'Light mode' : t('darkMode') || 'Dark mode'}>
            {isDark ? <Sun size={20} /> : <Moon size={20} />}
          </button>
          <button onClick={toggleLang} className="sidebar-ctrl-btn" title={i18n.language === 'zh' ? 'English' : '中文'} aria-label={t('switchLang') || 'Switch language'}>
            <Languages size={20} />
          </button>
          <button onClick={handleLogout} className="sidebar-ctrl-btn" aria-label={t('logout')}>
            <LogOut size={20} />
          </button>
        </div>
      </div>

      {/* Desktop Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-logo">UNVER</div>
        <nav style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
          {NavItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) => `sidebar-item ${isActive ? 'active' : ''}`}
            >
              {item.icon}
              <span>{item.label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-spacer" />

        <div className="sidebar-bottom-controls">
          <button onClick={toggleTheme} className="sidebar-ctrl-btn" title={isDark ? '亮色模式' : '暗黑模式'} aria-label={isDark ? t('lightMode') || 'Light mode' : t('darkMode') || 'Dark mode'}>
            {isDark ? <Sun size={20} /> : <Moon size={20} />}
          </button>
          <button onClick={toggleLang} className="sidebar-ctrl-btn" title={i18n.language === 'zh' ? 'English' : '中文'} aria-label={t('switchLang') || 'Switch language'}>
            <Languages size={20} />
          </button>
          <a href="https://github.com/unver" target="_blank" rel="noreferrer" className="sidebar-ctrl-btn" title="GitHub" aria-label="GitHub">
            <Github size={20} />
          </a>
          <span style={{ width: 1, background: 'var(--glass-border)', margin: '0 2px' }} />
          <button onClick={handleLogout} className="sidebar-ctrl-btn" title={t('logout')} aria-label={t('logout')}>
            <LogOut size={20} />
          </button>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="main-content">
        <Outlet />
      </main>

      {/* Mobile Navigation Bar */}
      <nav className="mobile-nav glass">
        {NavItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) => `mobile-nav-item ${isActive ? 'active' : ''}`}
            aria-label={item.label}
          >
            {item.icon}
          </NavLink>
        ))}
      </nav>
    </div>
  );
};

export default Layout;
