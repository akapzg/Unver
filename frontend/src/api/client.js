import axios from 'axios'

const client = axios.create({
  baseURL: '/api',
  withCredentials: true, // send cookies (refresh token)
  headers: { 'Content-Type': 'application/json' },
})

let isRefreshing = false
let failedQueue = []

const processQueue = (error, token = null) => {
  failedQueue.forEach(prom => (error ? prom.reject(error) : prom.resolve(token)))
  failedQueue = []
}

// Intercept 401 → try refresh → retry once
client.interceptors.response.use(
  res => res,
  async err => {
    const original = err.config
    if (err.response?.status === 401 && !original._retry && original.url !== '/auth/login') {
      if (isRefreshing) {
        return new Promise((resolve, reject) => {
          failedQueue.push({ resolve, reject })
        })
          .then(token => {
            original.headers['Authorization'] = `Bearer ${token}`
            return client(original)
          })
          .catch(Promise.reject)
      }

      original._retry = true
      isRefreshing = true

      try {
        const { data } = await client.post('/auth/refresh')
        const token = data.access_token
        localStorage.setItem('access_token', token)
        client.defaults.headers.common['Authorization'] = `Bearer ${token}`
        original.headers['Authorization'] = `Bearer ${token}`
        processQueue(null, token)
        return client(original)
      } catch (refreshErr) {
        processQueue(refreshErr, null)
        localStorage.removeItem('access_token')
        window.location.href = '/login'
        return Promise.reject(refreshErr)
      } finally {
        isRefreshing = false
      }
    }
    return Promise.reject(err)
  }
)

// Attach stored token to every request
client.interceptors.request.use(config => {
  const token = localStorage.getItem('access_token')
  if (token) config.headers['Authorization'] = `Bearer ${token}`
  return config
})

export default client

// ── API helpers ───────────────────────────────────────────────────────────────
export const api = {
  // Auth
  setupStatus:    ()       => client.get('/setup/status'),
  setup:          (body)   => client.post('/setup', body),
  login:          (body)   => client.post('/auth/login', body),
  logout:         ()       => client.post('/auth/logout'),
  changePassword: (body)   => client.post('/auth/change-password', body),

  // Proxies
  listProxies:   ()       => client.get('/proxies'),
  getProxy:      (id)     => client.get(`/proxies/${id}`),
  createProxy:   (body)   => client.post('/proxies', body),
  updateProxy:   (id, b)  => client.patch(`/proxies/${id}`, b),
  deleteProxy:   (id)     => client.delete(`/proxies/${id}`),

  // Settings
  getSettings:   ()       => client.get('/settings'),
  updateSettings:(body)   => client.patch('/settings', body),

  // API Keys
  listApiKeys:   ()       => client.get('/settings/api-keys'),
  createApiKey:  (body)   => client.post('/settings/api-keys', body),
  deleteApiKey:  (id)     => client.delete(`/settings/api-keys/${id}`),

  // System
  systemStats:  ()       => client.get('/system/stats'),
  publicIp:     ()       => client.get('/system/public-ip'),
  listLogs:     ()       => client.get('/system/logs'),
  listLogsByCategory: (cat) => client.get(`/system/logs/${encodeURIComponent(cat)}`),
  networkStats:  ()       => client.get('/system/network'),
  exportConfig:  ()       => client.get('/system/backup'),
  importConfig:  (body)   => client.post('/system/restore', body),
  renewSsl:      ()       => client.post('/system/renew-ssl'),
  checkUpdate:   ()       => client.get('/system/check-update'),
  performUpdate: ()       => client.post('/system/update'),

  // DDNS
  ddnsStatus:    ()       => client.get('/ddns/status'),
  ddnsToggle:    (domain, enabled) => client.patch(`/ddns/toggle/${encodeURIComponent(domain)}`, { enabled }),
  ddnsTest:      ()       => client.post('/ddns/test'),
  ddnsListZones:     ()       => client.get('/ddns/zones'),
  ddnsDeleteDomain: (domain) => client.delete(`/ddns/domain/${encodeURIComponent(domain)}`),

  // Certificates
  listCertificates:   ()       => client.get('/certificates'),
  issueCertificate:   (body)   => client.post('/certificates', body),
  uploadCertificate:  (body)   => client.post('/certificates/upload', body),
  certificateStatus:  (jobId)  => client.get(`/certificates/status/${encodeURIComponent(jobId)}`),
  deleteCertificate:  (id)     => client.delete(`/certificates/${id}`),
  downloadCertificate: (id)    => client.get(`/certificates/${id}/download`),
  updateCertificate:  (id, b)  => client.patch(`/certificates/${id}`, b),
  testCertificate:    (domain) => client.post('/certificates/test', { domain }),
}
