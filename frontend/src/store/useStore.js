import { create } from 'zustand'
import { api } from '../api/client'

export const useStore = create((set, get) => ({
  // ── Auth ──────────────────────────────────────────────────────────────────
  // NOTE: access_token stored in localStorage for simplicity.
  // This is vulnerable to XSS (if an attacker injects JS, they can steal the token).
  // For an internal-network management tool this is an acceptable trade-off.
  // Mitigations: HttpOnly refresh-token cookie (not accessible from JS),
  // short-lived access tokens (1 hour), automatic 401 → refresh flow.
  isAuthenticated: !!localStorage.getItem('access_token'),
  setupComplete: null, // null = unknown

  checkSetup: async () => {
    try {
      const { data } = await api.setupStatus()
      set({ setupComplete: data.setup_complete })
      return data.setup_complete
    } catch {
      set({ setupComplete: false })
      return false
    }
  },

  login: async (username, password) => {
    try {
      const { data } = await api.login({ username, password })
      localStorage.setItem('access_token', data.access_token)
      set({ isAuthenticated: true })
    } catch (e) {
      throw e
    }
  },

  logout: async () => {
    try { await api.logout() } catch (_) {}
    localStorage.removeItem('access_token')
    set({ isAuthenticated: false, proxies: [], stats: null })
  },

  // ── Proxies ───────────────────────────────────────────────────────────────
  proxies: [],
  proxiesLoading: false,
  proxiesError: null,

  fetchProxies: async () => {
    set({ proxiesLoading: true, proxiesError: null })
    try {
      const { data } = await api.listProxies()
      set({ proxies: data })
    } catch (e) {
      set({ proxiesError: e.response?.data?.error ?? 'Failed to load proxies' })
    } finally {
      set({ proxiesLoading: false })
    }
  },

  createProxy: async (body) => {
    try {
      const { data } = await api.createProxy(body)
      set(s => ({ proxies: [data, ...s.proxies] }))
      return data
    } catch (e) {
      throw e
    }
  },

  updateProxy: async (id, body) => {
    try {
      const { data } = await api.updateProxy(id, body)
      set(s => ({ proxies: s.proxies.map(p => (p.id === id ? data : p)) }))
      return data
    } catch (e) {
      throw e
    }
  },

  deleteProxy: async (id) => {
    try {
      await api.deleteProxy(id)
      set(s => ({ proxies: s.proxies.filter(p => p.id !== id) }))
    } catch (e) {
      throw e
    }
  },

  // ── Settings ──────────────────────────────────────────────────────────────
  settings: null,
  fetchSettings: async () => {
    try {
      const { data } = await api.getSettings()
      set({ settings: data })
    } catch (_) {}
  },
  updateSettings: async (body) => {
    try {
      const { data } = await api.updateSettings(body)
      set({ settings: data })
    } catch (e) {
      throw e
    }
  },

  // ── API Keys ──────────────────────────────────────────────────────────────
  apiKeys: [],
  fetchApiKeys: async () => {
    try {
      const { data } = await api.listApiKeys()
      set({ apiKeys: data })
    } catch (_) {}
  },
  createApiKey: async (name) => {
    try {
      const { data } = await api.createApiKey({ name })
      set(s => ({ apiKeys: [data, ...s.apiKeys] }))
      return data // contains the raw key — show to user ONCE
    } catch (e) {
      throw e
    }
  },
  deleteApiKey: async (id) => {
    try {
      await api.deleteApiKey(id)
      set(s => ({ apiKeys: s.apiKeys.filter(k => k.id !== id) }))
    } catch (e) {
      throw e
    }
  },

  // ── Stats ─────────────────────────────────────────────────────────────────
  stats: null,
  fetchStats: async () => {
    try {
      const { data } = await api.systemStats()
      set({ stats: data })
    } catch (_) {}
  },
}))
