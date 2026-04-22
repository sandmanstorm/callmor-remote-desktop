import axios from 'axios';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3000';

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
api.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && !original._retry) {
      original._retry = true;
      const refreshToken = localStorage.getItem('refresh_token');
      if (refreshToken) {
        try {
          const { data } = await axios.post(`${API_BASE}/auth/refresh`, {
            refresh_token: refreshToken,
          });
          localStorage.setItem('access_token', data.access_token);
          localStorage.setItem('refresh_token', data.refresh_token);
          original.headers.Authorization = `Bearer ${data.access_token}`;
          return api(original);
        } catch {
          localStorage.clear();
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
}

export interface CreateMachineResponse {
  id: string;
  name: string;
  agent_token: string;
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
  create: (name: string) => api.post<CreateMachineResponse>('/machines', { name }),
  delete: (id: string) => api.delete(`/machines/${id}`),
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

export default api;
