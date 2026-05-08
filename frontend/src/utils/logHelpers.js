// Shared log rendering helpers used by Ddns, Ssl, and other log views.
// Keeps log-line style and icon mappings in one place.

export const logStyle = (level) => {
  switch (level) {
    case 'success': return { color: 'var(--success)' };
    case 'error':   return { color: 'var(--danger)' };
    default:        return { color: 'var(--text-secondary)', opacity: 0.85 };
  }
};

export const logIcon = (level) => {
  switch (level) {
    case 'success': return '✅';
    case 'error':   return '❌';
    default:        return '  ';
  }
};
