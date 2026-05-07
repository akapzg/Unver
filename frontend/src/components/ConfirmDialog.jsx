import React, { createContext, useContext, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

const ConfirmContext = createContext();

export const useConfirm = () => useContext(ConfirmContext);

export const ConfirmProvider = ({ children }) => {
  const { t } = useTranslation();
  const [dialog, setDialog] = useState(null); // { message, onConfirm, onCancel }

  const confirm = useCallback((message) => {
    return new Promise((resolve) => {
      setDialog({
        message,
        resolve,
      });
    });
  }, []);

  const handleConfirm = () => {
    dialog?.resolve(true);
    setDialog(null);
  };

  const handleCancel = () => {
    dialog?.resolve(false);
    setDialog(null);
  };

  return (
    <ConfirmContext.Provider value={{ confirm }}>
      {children}
      {dialog && (
        <div className="modal-overlay">
          <div className="modal glass" style={{ maxWidth: 420 }}>
            <header className="modal-header">
              <h2 className="modal-title">{t('confirm')}</h2>
            </header>
            <div className="modal-body">
              <p style={{ fontSize: '0.95rem', lineHeight: 1.6 }}>{dialog.message}</p>
            </div>
            <footer className="modal-footer">
              <button className="btn btn-ghost" onClick={handleCancel}>{t('cancel')}</button>
              <button className="btn btn-danger" onClick={handleConfirm}>{t('confirm')}</button>
            </footer>
          </div>
        </div>
      )}
    </ConfirmContext.Provider>
  );
};
