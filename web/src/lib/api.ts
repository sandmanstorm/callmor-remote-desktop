import axios from 'axios';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3000';

/// Safely extract a human-readable error message from an axios error.
/// Backend sometimes returns a string body, sometimes a JSON object —
/// this handles both without React crashing on "Objects are not valid as child".
export function errMsg(err: any, fallback = 'An error occurred'): string {
  const data = err?.response?.data;
  if (typeof data === 'string') return data;
  if (data?.error) return String(data.error);
  if (data?.message) return String(data.message);
  if (err?.message) return String(err.message);
  return fallback;
}

const api = axios.create({ baseURL: API_BASE });

// Attach JWT to every request
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('access_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Auto-refresh on 401
// Concurrent 401s must share a single refresh request, not each try their own.
let refreshInflight: Promise<string> | null = null;

async function refreshAccessToken(): Promise<string> {
  if (refreshInflight) return refreshInflight;
  refreshInflight = (async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (!refreshToken) throw new Error('no refresh token');
    try {
      const { data } = await axios.post(`${API_BASE}/auth/refresh`, {
        refresh_token: refreshToken,
      });
      localStorage.setItem('access_token', data.access_token);
      localStorage.setItem('refresh_token', data.refresh_token);
      return data.access_token as string;
    } finally {
      // Allow next cycle to start its own refresh (after current resolves)
      setTimeout(() => { refreshInflight = null; }, 0);
    }
  })();
  return refreshInflight;
}

api.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && original && !original._retry) {
      original._retry = true;
      try {
        const newToken = await refreshAccessToken();
        original.headers = original.headers || {};
        original.headers.Authorization = `Bearer ${newToken}`;
        return api(original);
      } catch {
        // Refresh failed — clear auth state and bounce to login
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
        localStorage.removeItem('user');
        if (window.location.pathname !== '/login') {
          window.location.href = '/login';
        }
      }
    }
    return Promise.reject(error);
  }
);

export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  user: UserInfo;
}

export interface UserInfo {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_superadmin: boolean;
  tenant_id: string;
  tenant_name: string;
  tenant_slug: string;
}

export interface Machine {
  id: string;
  tenant_id: string;
  name: string;
  hostname: string | null;
  os: string | null;
  last_seen: string | null;
  is_online: boolean;
  access_mode: string;
  created_at: string;
  rustdesk_id: string | null;
  rustdesk_password: string | null;
  connection_type: 'rustdesk' | 'webrtc_legacy';
}

export interface CreateMachineRequest {
  name: string;
  rustdesk_id?: string;
  rustdesk_password?: string;
}

export interface CreateMachineResponse {
  id: string;
  name: string;
  agent_token: string;
}

export interface RdConnectResponse {
  rustdesk_id: string;
  password: string;
  launch_uri: string;
}

export const authApi = {
  register: (data: { email: string; password: string; display_name: string; tenant_name: string }) =>
    api.post<AuthResponse>('/auth/register', data),
  login: (data: { email: string; password: string; tenant_slug: string }) =>
    api.post<AuthResponse>('/auth/login', data),
  logout: () => {
    const token = localStorage.getItem('refresh_token');
    if (token) api.post('/auth/logout', { refresh_token: token });
    localStorage.clear();
  },
};

export const machinesApi = {
  list: () => api.get<Machine[]>('/machines'),
  get: (id: string) => api.get<Machine>(`/machines/${id}`),
  create: (data: CreateMachineRequest) =>
    api.post<CreateMachineResponse>('/machines', data),
  update: (id: string, patch: Partial<CreateMachineRequest>) =>
    api.patch<Machine>(`/machines/${id}`, patch),
  delete: (id: string) => api.delete(`/machines/${id}`),
  rdConnect: (id: string) =>
    api.post<RdConnectResponse>(`/machines/${id}/rd-connect`),
};

export interface SessionResponse {
  session: { id: string; machine_id: string; started_at: string };
  session_token: string;
  machine_id: string;
  relay_url: string;
}

export const sessionsApi = {
  create: (machineId: string, permission = 'full_control') =>
    api.post<SessionResponse>('/sessions', { machine_id: machineId, permission }),
};

// --- Users ---
export interface User {
  id: string;
  email: string;
  display_name: string;
  role: string;
  created_at: string;
}

export const usersApi = {
  list: () => api.get<User[]>('/users'),
  update: (id: string, role: string) => api.patch(`/users/${id}`, { role }),
  delete: (id: string) => api.delete(`/users/${id}`),
};

// --- Invitations ---
export interface Invitation {
  id: string;
  email: string;
  role: string;
  expires_at: string;
  created_at: string;
}

export interface CreateInvitationResponse {
  id: string;
  email: string;
  role: string;
  token: string;
  expires_at: string;
  email_sent: boolean;
}

export interface InvitationDetails {
  email: string;
  role: string;
  tenant_name: string;
  tenant_slug: string;
  invited_by_name: string;
  expires_at: string;
}

export const invitationsApi = {
  list: () => api.get<Invitation[]>('/invitations'),
  create: (email: string, role = 'member') =>
    api.post<CreateInvitationResponse>('/invitations', { email, role }),
  delete: (id: string) => api.delete(`/invitations/${id}`),
  getByToken: (token: string) => api.get<InvitationDetails>(`/invitations/token/${token}`),
  accept: (token: string, password: string, display_name: string) =>
    api.post<AuthResponse>(`/invitations/token/${token}/accept`, { password, display_name }),
};

// --- Machine access ---
export interface AccessUser {
  user_id: string;
  email: string;
  display_name: string;
}

export const machineAccessApi = {
  list: (machineId: string) => api.get<AccessUser[]>(`/machines/${machineId}/access`),
  grant: (machineId: string, userId: string) =>
    api.post(`/machines/${machineId}/access`, { user_id: userId }),
  revoke: (machineId: string, userId: string) =>
    api.delete(`/machines/${machineId}/access/${userId}`),
  updateMode: (machineId: string, access_mode: 'public' | 'restricted') =>
    api.patch(`/machines/${machineId}`, { access_mode }),
};

// --- Super-admin (platform) ---

export interface PlatformStats {
  total_tenants: number;
  total_users: number;
  total_machines: number;
  online_machines: number;
  active_sessions: number;
}

export interface TenantOverview {
  id: string;
  name: string;
  slug: string;
  user_count: number;
  machine_count: number;
  online_machines: number;
  created_at: string;
}

export interface GlobalUser {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_superadmin: boolean;
  tenant_id: string;
  tenant_name: string;
  created_at: string;
}

export interface GlobalMachine {
  id: string;
  name: string;
  hostname: string | null;
  os: string | null;
  is_online: boolean;
  last_seen: string | null;
  tenant_id: string;
  tenant_name: string;
}

export const adminApi = {
  stats: () => api.get<PlatformStats>('/admin/stats'),
  listTenants: () => api.get<TenantOverview[]>('/admin/tenants'),
  deleteTenant: (id: string) => api.delete(`/admin/tenants/${id}`),
  listUsers: () => api.get<GlobalUser[]>('/admin/users'),
  setSuperadmin: (userId: string, is_superadmin: boolean) =>
    api.patch(`/admin/users/${userId}/superadmin`, { is_superadmin }),
  listMachines: () => api.get<GlobalMachine[]>('/admin/machines'),
};

// --- SMTP Settings ---

export interface SmtpSettings {
  host: string;
  port: number;
  username: string;
  from_email: string;
  from_name: string;
  tls: 'starttls' | 'implicit' | 'none';
  has_password: boolean;
  configured: boolean;
}

export interface UpdateSmtpRequest {
  host: string;
  port: number;
  username: string;
  password?: string; // only sent if changing
  from_email: string;
  from_name: string;
  tls: 'starttls' | 'implicit' | 'none';
}

export const settingsApi = {
  getSmtp: () => api.get<SmtpSettings>('/admin/settings/smtp'),
  updateSmtp: (data: UpdateSmtpRequest) => api.put('/admin/settings/smtp', data),
  clearSmtp: () => api.delete('/admin/settings/smtp'),
  testEmail: (to: string) =>
    api.post<{ sent: boolean; message: string }>('/admin/test-email', { to }),
};

// --- Audit log ---
export interface AuditEvent {
  id: string;
  tenant_id: string | null;
  actor_id: string | null;
  actor_email: string | null;
  actor_display: string | null;
  event_type: string;
  entity_type: string | null;
  entity_id: string | null;
  metadata: Record<string, any>;
  ip_address: string | null;
  created_at: string;
}

export const auditApi = {
  listTenant: (params?: { event_type?: string; limit?: number }) =>
    api.get<AuditEvent[]>('/audit', { params }),
  listPlatform: (params?: { event_type?: string; limit?: number }) =>
    api.get<AuditEvent[]>('/admin/audit', { params }),
};

// --- Recordings ---
export interface Recording {
  id: string;
  session_id: string;
  machine_id: string;
  machine_name: string;
  size_bytes: number;
  duration_ms: number | null;
  content_type: string;
  created_at: string;
  started_by: string | null;
}

export interface TenantSettings {
  recording_enabled: boolean;
}

export const recordingsApi = {
  list: () => api.get<Recording[]>('/recordings'),
  playbackUrl: (id: string) => `${import.meta.env.VITE_API_URL || ''}/recordings/${id}/playback`,
  delete: (id: string) => api.delete(`/recordings/${id}`),
};

export const tenantSettingsApi = {
  get: () => api.get<TenantSettings>('/tenant/settings'),
  update: (data: TenantSettings) => api.put('/tenant/settings', data),
};

export interface TenantEnrollmentInfo {
  enrollment_token: string;
}

export const enrollmentApi = {
  get: () => api.get<TenantEnrollmentInfo>('/tenant/enrollment'),
  rotate: () => api.post<TenantEnrollmentInfo>('/tenant/enrollment/rotate'),
};

// --- Ad-hoc (login-less) flow ---

export interface ConnectRequest {
  access_code: string;
  pin: string;
}

export interface ConnectResponse {
  machine_id: string;
  session_token: string;
  relay_url: string;
  hostname: string;
}

export interface ClaimRequest {
  access_code: string;
  pin: string;
  name?: string;
}

export interface ClaimResponse {
  machine_id: string;
  name: string;
}

export const adhocApi = {
  // Public — no auth header required, but axios will send one if the user
  // happens to be logged in; the API ignores it for these endpoints.
  connect: (data: ConnectRequest) =>
    api.post<ConnectResponse>('/connect', data),
  claim: (data: ClaimRequest) =>
    api.post<ClaimResponse>('/machines/claim', data),
};

export default api;
