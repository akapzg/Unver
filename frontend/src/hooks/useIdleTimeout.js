import { useEffect, useRef, useCallback } from 'react';

/**
 * Auto-logout after `timeoutMinutes` of inactivity.
 * Shows a warning toast 60s before logout via `onWarning` callback.
 *
 * @param {number} timeoutMinutes — idle minutes before auto-logout (default 15)
 * @param {function} onTimeout    — called when timeout fires (do logout)
 * @param {function} onWarning    — called 60s before timeout (show warning)
 */
export default function useIdleTimeout(timeoutMinutes = 15, onTimeout, onWarning) {
  const lastActivity = useRef(Date.now());
  const warned = useRef(false);
  const onTimeoutRef = useRef(onTimeout);
  const onWarningRef = useRef(onWarning);

  // Keep refs fresh so callbacks don't need to be in deps
  useEffect(() => { onTimeoutRef.current = onTimeout; }, [onTimeout]);
  useEffect(() => { onWarningRef.current = onWarning; }, [onWarning]);

  const resetTimer = useCallback(() => {
    lastActivity.current = Date.now();
    warned.current = false;
  }, []);

  useEffect(() => {
    const events = ['mousemove', 'keydown', 'click', 'scroll', 'touchstart'];
    events.forEach(e => window.addEventListener(e, resetTimer, { passive: true }));

    const check = setInterval(() => {
      const idle = (Date.now() - lastActivity.current) / 1000; // seconds
      const limit = timeoutMinutes * 60;
      const warnAt = limit - 60;

      if (idle >= limit) {
        onTimeoutRef.current?.();
      } else if (idle >= warnAt && !warned.current) {
        warned.current = true;
        onWarningRef.current?.();
      }
    }, 1000);

    return () => {
      events.forEach(e => window.removeEventListener(e, resetTimer));
      clearInterval(check);
    };
  }, [timeoutMinutes, resetTimer]);

  return { resetTimer };
}
