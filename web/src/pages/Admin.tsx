import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { adminApi } from '../lib/api';
import type { PlatformStats, TenantOverview, GlobalUser, GlobalMachine } from '../lib/api';
import { Monitor, LogOut, ArrowLeft, Building2, Users, HardDrive, Shield, Trash2, Crown, Wifi, WifiOff } from 'lucide-react';

type Tab = 'stats' | 'tenants' | 'users' | 'machines';

export default function Admin() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>('stats');
  const [stats, setStats] = useState<PlatformStats | null>(null);
  const [tenants, setTenants] = useState<TenantOverview[]>([]);
  const [users, setUsers] = useState<GlobalUser[]>([]);
  const [machines, setMachines] = useState<GlobalMachine[]>([]);

  useEffect(() => {
    if (!user?.is_superadmin) {
      navigate('/');
      return;
    }
    loadTab(tab);
  }, [tab, user]);

  async function loadTab(t: Tab) {
    try {
      if (t === 'stats') {
        const { data } = await adminApi.stats();
        setStats(data);
      } else if (t === 'tenants') {
        const { data } = await adminApi.listTenants();
        setTenants(data);
      } else if (t === 'users') {
        const { data } = await adminApi.listUsers();
        setUsers(data);
      } else if (t === 'machines') {
        const { data } = await adminApi.listMachines();
        setMachines(data);
      }
    } catch (err: any) {
      alert(err.response?.data || 'Failed to load');
    }
  }

  const handleDeleteTenant = async (t: TenantOverview) => {
    if (!confirm(`Delete tenant "${t.name}" and ALL its users (${t.user_count}) and machines (${t.machine_count})?\n\nThis cannot be undone.`)) return;
    try {
      await adminApi.deleteTenant(t.id);
      loadTab('tenants');
    } catch (err: any) {
      alert(err.response?.data || 'Failed to delete');
    }
  };

  const handleToggleSuperadmin = async (u: GlobalUser) => {
    const action = u.is_superadmin ? 'revoke' : 'grant';
    if (!confirm(`${action} super-admin for ${u.email}?`)) return;
    try {
      await adminApi.setSuperadmin(u.id, !u.is_superadmin);
      loadTab('users');
    } catch (err: any) {
      alert(err.response?.data || 'Failed');
    }
  };

  const handleLogout = () => { logout(); navigate('/login'); };

  return (
    <div className="min-h-screen bg-gray-950">
      <header className="bg-gray-900 border-b border-gray-800 px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Shield className="w-6 h-6 text-red-400" />
          <h1 className="text-lg font-semibold text-white">Callmor Platform Admin</h1>
        </div>
        <div className="flex items-center gap-4">
          <button onClick={() => navigate('/')} className="text-sm text-gray-400 hover:text-white flex items-center gap-1">
            <ArrowLeft className="w-4 h-4" /> My Dashboard
          </button>
          <span className="text-sm text-gray-400">{user?.display_name}</span>
          <button onClick={handleLogout} className="text-gray-400 hover:text-white"><LogOut className="w-4 h-4" /></button>
        </div>
      </header>

      <nav className="bg-gray-900 border-b border-gray-800 px-6">
        <div className="max-w-5xl mx-auto flex gap-6">
          {[
            { id: 'stats', icon: Monitor, label: 'Overview' },
            { id: 'tenants', icon: Building2, label: 'Tenants' },
            { id: 'users', icon: Users, label: 'Users' },
            { id: 'machines', icon: HardDrive, label: 'Machines' },
          ].map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              onClick={() => setTab(id as Tab)}
              className={`flex items-center gap-2 py-3 px-1 text-sm border-b-2 ${
                tab === id ? 'border-red-400 text-white' : 'border-transparent text-gray-400 hover:text-white'
              }`}
            >
              <Icon className="w-4 h-4" /> {label}
            </button>
          ))}
        </div>
      </nav>

      <main className="max-w-5xl mx-auto px-6 py-8">
        {tab === 'stats' && stats && (
          <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
            <StatCard label="Tenants" value={stats.total_tenants} icon={Building2} color="blue" />
            <StatCard label="Users" value={stats.total_users} icon={Users} color="green" />
            <StatCard label="Machines" value={stats.total_machines} icon={HardDrive} color="purple" />
            <StatCard label="Online" value={stats.online_machines} icon={Wifi} color="emerald" />
            <StatCard label="Active Sessions" value={stats.active_sessions} icon={Monitor} color="orange" />
          </div>
        )}

        {tab === 'tenants' && (
          <div className="grid gap-2">
            {tenants.map((t) => (
              <div key={t.id} className="bg-gray-900 border border-gray-800 rounded p-4 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <Building2 className="w-5 h-5 text-blue-400" />
                  <div>
                    <div className="text-white font-medium">{t.name} <span className="text-gray-500 text-xs">/ {t.slug}</span></div>
                    <div className="text-xs text-gray-500">
                      {t.user_count} users · {t.machine_count} machines ({t.online_machines} online) · created {new Date(t.created_at).toLocaleDateString()}
                    </div>
                  </div>
                </div>
                <button
                  onClick={() => handleDeleteTenant(t)}
                  className="text-gray-500 hover:text-red-400 p-1"
                  title="Delete tenant and all its data"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
            {tenants.length === 0 && <p className="text-gray-500">No tenants.</p>}
          </div>
        )}

        {tab === 'users' && (
          <div className="grid gap-2">
            {users.map((u) => (
              <div key={u.id} className="bg-gray-900 border border-gray-800 rounded p-4 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  {u.is_superadmin ? <Shield className="w-5 h-5 text-red-400" /> : u.role === 'owner' ? <Crown className="w-5 h-5 text-yellow-500" /> : <Users className="w-5 h-5 text-gray-500" />}
                  <div>
                    <div className="text-white">{u.display_name}{u.id === user?.id && <span className="text-gray-500 text-xs ml-2">(you)</span>}</div>
                    <div className="text-xs text-gray-500">{u.email} · {u.role} · {u.tenant_name}{u.is_superadmin && ' · SUPER-ADMIN'}</div>
                  </div>
                </div>
                <button
                  onClick={() => handleToggleSuperadmin(u)}
                  className={`text-xs px-3 py-1 rounded ${u.is_superadmin ? 'bg-red-900 text-red-300 hover:bg-red-800' : 'bg-gray-800 text-gray-400 hover:bg-gray-700'}`}
                >
                  {u.is_superadmin ? 'Revoke super-admin' : 'Grant super-admin'}
                </button>
              </div>
            ))}
            {users.length === 0 && <p className="text-gray-500">No users.</p>}
          </div>
        )}

        {tab === 'machines' && (
          <div className="grid gap-2">
            {machines.map((m) => (
              <div key={m.id} className="bg-gray-900 border border-gray-800 rounded p-4 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  {m.is_online ? <Wifi className="w-5 h-5 text-green-400" /> : <WifiOff className="w-5 h-5 text-gray-500" />}
                  <div>
                    <div className="text-white">{m.name}</div>
                    <div className="text-xs text-gray-500">
                      {m.tenant_name} · {m.hostname || 'no hostname'} · {m.os || 'unknown OS'}
                      {m.last_seen && <> · last seen {new Date(m.last_seen).toLocaleString()}</>}
                    </div>
                  </div>
                </div>
              </div>
            ))}
            {machines.length === 0 && <p className="text-gray-500">No machines.</p>}
          </div>
        )}
      </main>
    </div>
  );
}

function StatCard({ label, value, icon: Icon, color }: { label: string; value: number; icon: any; color: string }) {
  const colorMap: Record<string, string> = {
    blue: 'bg-blue-900/30 text-blue-400',
    green: 'bg-green-900/30 text-green-400',
    purple: 'bg-purple-900/30 text-purple-400',
    emerald: 'bg-emerald-900/30 text-emerald-400',
    orange: 'bg-orange-900/30 text-orange-400',
  };
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
      <div className={`inline-flex p-2 rounded ${colorMap[color]} mb-2`}>
        <Icon className="w-5 h-5" />
      </div>
      <div className="text-2xl font-bold text-white">{value}</div>
      <div className="text-xs text-gray-500 uppercase tracking-wide">{label}</div>
    </div>
  );
}
