import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { adminApi, settingsApi, auditApi, errMsg } from '../lib/api';
import type { PlatformStats, TenantOverview, GlobalUser, GlobalMachine, SmtpSettings } from '../lib/api';
import AuditLog from '../components/AuditLog';
import { Monitor, LogOut, ArrowLeft, Building2, Users, HardDrive, Shield, Trash2, Crown, Wifi, WifiOff, Settings, Mail, Send, Activity } from 'lucide-react';

type Tab = 'stats' | 'tenants' | 'users' | 'machines' | 'audit' | 'settings';

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
      navigate('/app');
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
      alert(errMsg(err, 'Failed to load'));
    }
  }

  const handleDeleteTenant = async (t: TenantOverview) => {
    if (!confirm(`Delete tenant "${t.name}" and ALL its users (${t.user_count}) and machines (${t.machine_count})?\n\nThis cannot be undone.`)) return;
    try {
      await adminApi.deleteTenant(t.id);
      loadTab('tenants');
    } catch (err: any) {
      alert(errMsg(err, 'Failed to delete'));
    }
  };

  const handleToggleSuperadmin = async (u: GlobalUser) => {
    const action = u.is_superadmin ? 'revoke' : 'grant';
    if (!confirm(`${action} super-admin for ${u.email}?`)) return;
    try {
      await adminApi.setSuperadmin(u.id, !u.is_superadmin);
      loadTab('users');
    } catch (err: any) {
      alert(errMsg(err, 'Failed'));
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
          <button onClick={() => navigate('/app')} className="text-sm text-gray-400 hover:text-white flex items-center gap-1">
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
            { id: 'audit', icon: Activity, label: 'Audit' },
            { id: 'settings', icon: Settings, label: 'Settings' },
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

        {tab === 'audit' && <AuditLog fetchEvents={auditApi.listPlatform} showTenant />}

        {tab === 'settings' && <SmtpSettingsForm />}
      </main>
    </div>
  );
}

function SmtpSettingsForm() {
  const [loading, setLoading] = useState(true);
  const [current, setCurrent] = useState<SmtpSettings | null>(null);
  const [form, setForm] = useState({
    host: '',
    port: 587,
    username: '',
    password: '',
    from_email: '',
    from_name: 'Callmor Remote',
    tls: 'starttls' as 'starttls' | 'implicit' | 'none',
  });
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [testEmail, setTestEmail] = useState('');
  const [testing, setTesting] = useState(false);

  const load = async () => {
    try {
      const { data } = await settingsApi.getSmtp();
      setCurrent(data);
      setForm({
        host: data.host,
        port: data.port,
        username: data.username,
        password: '',
        from_email: data.from_email,
        from_name: data.from_name || 'Callmor Remote',
        tls: data.tls as any,
      });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, []);

  const handleSave = async () => {
    setSaving(true);
    setMessage(null);
    try {
      const body: any = { ...form };
      if (!body.password) delete body.password; // only send password if user entered one
      await settingsApi.updateSmtp(body);
      setMessage({ type: 'success', text: 'SMTP settings saved.' });
      await load();
    } catch (err: any) {
      setMessage({ type: 'error', text: errMsg(err, 'Failed to save') });
    } finally {
      setSaving(false);
    }
  };

  const handleClear = async () => {
    if (!confirm('Clear all SMTP settings? Emails will be disabled until reconfigured.')) return;
    try {
      await settingsApi.clearSmtp();
      setMessage({ type: 'success', text: 'SMTP settings cleared.' });
      await load();
    } catch (err: any) {
      setMessage({ type: 'error', text: errMsg(err, 'Failed') });
    }
  };

  const handleTest = async () => {
    if (!testEmail.trim()) return;
    setTesting(true);
    try {
      const { data } = await settingsApi.testEmail(testEmail.trim());
      setMessage({
        type: data.sent ? 'success' : 'error',
        text: data.message,
      });
    } catch (err: any) {
      setMessage({ type: 'error', text: errMsg(err, 'Test failed') });
    } finally {
      setTesting(false);
    }
  };

  if (loading) return <p className="text-gray-500">Loading settings...</p>;

  return (
    <div className="max-w-2xl">
      <div className="flex items-center gap-2 mb-4">
        <Mail className="w-5 h-5 text-blue-400" />
        <h2 className="text-lg text-white font-semibold">SMTP / Email</h2>
        {current?.configured && (
          <span className="text-xs bg-green-900/40 text-green-400 px-2 py-0.5 rounded">Configured</span>
        )}
      </div>

      <p className="text-sm text-gray-400 mb-4">
        Configure an SMTP server so invitations are emailed automatically instead of copy-paste.
        For Mail-in-a-Box: port 587, STARTTLS, use a mailbox you created as the username.
      </p>

      {message && (
        <div className={`mb-4 px-3 py-2 rounded text-sm ${message.type === 'success' ? 'bg-green-900/30 border border-green-700 text-green-300' : 'bg-red-900/30 border border-red-700 text-red-300'}`}>
          {message.text}
        </div>
      )}

      <div className="bg-gray-900 border border-gray-800 rounded-lg p-5 space-y-3">
        <div className="grid grid-cols-[1fr_120px] gap-3">
          <div>
            <label className="block text-sm text-gray-400 mb-1">SMTP Host</label>
            <input type="text" value={form.host} onChange={(e) => setForm({ ...form, host: e.target.value })}
              placeholder="box.yourdomain.com"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">Port</label>
            <input type="number" value={form.port} onChange={(e) => setForm({ ...form, port: parseInt(e.target.value) || 587 })}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">TLS Mode</label>
          <select value={form.tls} onChange={(e) => setForm({ ...form, tls: e.target.value as any })}
            className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white">
            <option value="starttls">STARTTLS (port 587, recommended)</option>
            <option value="implicit">Implicit TLS (port 465)</option>
            <option value="none">None (plaintext, NOT recommended)</option>
          </select>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">Username (full email)</label>
          <input type="text" value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })}
            placeholder="noreply@yourdomain.com"
            className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Password {current?.has_password && <span className="text-xs text-green-400">(saved; leave blank to keep)</span>}
          </label>
          <input type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })}
            placeholder={current?.has_password ? '••••••••' : 'mailbox password'}
            className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-sm text-gray-400 mb-1">From email</label>
            <input type="text" value={form.from_email} onChange={(e) => setForm({ ...form, from_email: e.target.value })}
              placeholder="(defaults to username)"
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">From name</label>
            <input type="text" value={form.from_name} onChange={(e) => setForm({ ...form, from_name: e.target.value })}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
          </div>
        </div>

        <div className="flex gap-2 pt-2">
          <button onClick={handleSave} disabled={saving}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white rounded text-sm">
            {saving ? 'Saving...' : 'Save SMTP Settings'}
          </button>
          {current?.configured && (
            <button onClick={handleClear}
              className="px-4 py-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-red-400 rounded text-sm">
              Clear
            </button>
          )}
        </div>
      </div>

      {/* Test */}
      {current?.configured && (
        <div className="bg-gray-900 border border-gray-800 rounded-lg p-5 mt-4">
          <h3 className="text-white font-medium mb-2 flex items-center gap-2">
            <Send className="w-4 h-4" /> Send Test Email
          </h3>
          <div className="flex gap-2">
            <input type="email" value={testEmail} onChange={(e) => setTestEmail(e.target.value)}
              placeholder="your@email.com"
              className="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white" />
            <button onClick={handleTest} disabled={testing || !testEmail}
              className="px-4 py-2 bg-green-700 hover:bg-green-600 disabled:opacity-50 text-white rounded text-sm">
              {testing ? 'Sending...' : 'Send Test'}
            </button>
          </div>
        </div>
      )}
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
