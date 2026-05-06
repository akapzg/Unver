import React, { useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router-dom';
import { useStore } from './store/useStore';
import Layout from './components/Layout';
import Login from './views/Login';
import Setup from './views/Setup';
import Dashboard from './views/Dashboard';
import Settings from './views/Settings';
import Proxies from './views/Proxies';
import Ddns from './views/Ddns';
import Ssl from './views/Ssl';

const ProtectedRoute = ({ children }) => {
  const { isAuthenticated, setupComplete, checkSetup } = useStore();
  const location = useLocation();

  useEffect(() => {
    checkSetup();
  }, [checkSetup]);

  if (setupComplete === false) {
    return <Navigate to="/setup" replace />;
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" state={{ from: location }} replace />;
  }

  return children;
};

function App() {
  const { checkSetup } = useStore();

  useEffect(() => {
    checkSetup();
  }, [checkSetup]);

  return (
    <BrowserRouter>
      <div className="mesh-bg" />
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/setup" element={<Setup />} />

        <Route path="/" element={
          <ProtectedRoute>
            <Layout />
          </ProtectedRoute>
        }>
          <Route index element={<Navigate to="/dashboard" replace />} />
          <Route path="dashboard" element={<Dashboard />} />
          <Route path="proxies" element={<Proxies />} />
          <Route path="ddns" element={<Ddns />} />
          <Route path="ssl" element={<Ssl />} />
          <Route path="settings" element={<Settings />} />
        </Route>

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
