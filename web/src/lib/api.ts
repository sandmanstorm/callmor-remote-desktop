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

export default api;
