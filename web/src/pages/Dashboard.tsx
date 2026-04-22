import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { machinesApi, sessionsApi, machineAccessApi, usersApi, enrollmentApi, errMsg } from '../lib/api';
import type { Machine, CreateMachineResponse, AccessUser, User } from '../lib/api';
import { Monitor, Plus, Trash2, LogOut, Copy, Wifi, WifiOff, Download, Users, Eye, Settings, Lock, Globe, X, Shield, Activity, Film, RefreshCw, Key } from 'lucide-react';

export default function Dashboard() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [machines, setMachines] = useState<Machine[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddModal, setShowAddModal] = useState(false);
  const [newMachineName, setNewMachineName] = useState('');
  const [newMachineResult, setNewMachineResult] = useState<CreateMachineResponse | null>(null);
  const [accessModal, setAccessModal] = useState<Machine | null>(null);
  const [accessUsers, setAccessUsers] = useState<AccessUser[]>([]);
  const [orgUsers, setOrgUsers] = useState<User[]>([]);

  const isAdmin = user?.role === 'owner' || user?.role === 'admin';
  const isOwner = user?.role === 'owner';
  const [showTokenModal, setShowTokenModal] = useState(false);
  const [enrollmentToken, setEnrollmentToken] = useState<string | null>(null);
  const [rotating, setRotating] = useState(false);

  const openTokenModal = async () => {
    setShowTokenModal(true);
    if (enrollmentToken) return;
    try {
      const { data } = await enrollmentApi.get();
      setEnrollmentToken(data.enrollment_token);
    } catch (err: any) {
      alert(errMsg(err, 'Failed to load enrollment token'));
      setShowTokenModal(false);
    }
  };

  const rotateToken = async () => {
    if (!confirm('Rotate enrollment token? The old token stops working for new installs immediately. Already-enrolled machines keep working.')) return;
    setRotating(true);
    try {
      const { data } = await enrollmentApi.rotate();
      setEnrollmentToken(data.enrollment_token);
    } catch (err: any) {
      alert(errMsg(err, 'Failed to rotate token'));
    } finally {
      setRotating(false);
    }
  };

  const fetchMachines = async () => {
    try {
      const { data } = await machinesApi.list();
      setMachines(data);
    } catch {
      // Token might be expired
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMachines();
    const interval = setInterval(fetchMachines, 10000); // poll every 10s
    return () => clearInterval(interval);
  }, []);

  const handleAddMachine = async () => {
    if (!newMachineName.trim()) return;
    try {
      const { data } = await machinesApi.create(newMachineName.trim());
      setNewMachineResult(data);
      fetchMachines();
    } catch (err: any) {
      alert(errMsg(err, 'Failed to add machine'));
    }
  };

  const handleDeleteMachine = async (id: string, name: string) => {
    if (!confirm(`Delete machine "${name}"?`)) return;
    await machinesApi.delete(id);
    fetchMachines();
  };

  const handleConnect = async (machine: Machine, permission: 'full_control' | 'view_only' = 'full_control') => {
    try {
      const { data } = await sessionsApi.create(machine.id, permission);
      const params = new URLSearchParams({
        relay: data.relay_url,
        machine: data.machine_id,
        token: data.session_token,
        permission,
        session: data.session.id,
      });
      window.open(`/viewer-test.html?${params.toString()}`, '_blank');
    } catch (err: any) {
      alert(errMsg(err, 'Failed to start session'));
    }
  };

  const openAccessModal = async (machine: Machine) => {
    setAccessModal(machine);
    try {
      const [access, users] = await Promise.all([
        machineAccessApi.list(machine.id),
        usersApi.list(),
      ]);
      setAccessUsers(access.data);
      setOrgUsers(users.data);
    } catch {}
  };

  const handleToggleAccessMode = async () => {
    if (!accessModal) return;
    const newMode = accessModal.access_mode === 'public' ? 'restricted' : 'public';
    await machineAccessApi.updateMode(accessModal.id, newMode);
    setAccessModal({ ...accessModal, access_mode: newMode });
    fetchMachines();
  };

  const handleGrantAccess = async (userId: string) => {
    if (!accessModal) return;
    await machineAccessApi.grant(accessModal.id, userId);
    const res = await machineAccessApi.list(accessModal.id);
    setAccessUsers(res.data);
  };

  const handleRevokeAccess = async (userId: string) => {
    if (!accessModal) return;
    await machineAccessApi.revoke(accessModal.id, userId);
    const res = await machineAccessApi.list(accessModal.id);
    setAccessUsers(res.data);
  };

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  return (
    <div className="min-h-screen bg-gray-950">
      {/* Header */}
      <header className="bg-gray-900 border-b border-gray-800 px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Monitor className="w-6 h-6 text-blue-400" />
          <h1 className="text-lg font-semibold text-white">Callmor Remote</h1>
          <span className="text-sm text-gray-500">|</span>
          <span className="text-sm text-gray-400">{user?.tenant_name}</span>
        </div>
        <div className="flex items-center gap-4">
          {user?.is_superadmin && (
            <button onClick={() => navigate('/admin')} className="text-red-400 hover:text-red-300 flex items-center gap-1 text-sm" title="Platform admin">
              <Shield className="w-4 h-4" /> Admin
            </button>
          )}
          <button onClick={() => navigate('/recordings')} className="text-gray-400 hover:text-white flex items-center gap-1 text-sm" title="Recordings">
            <Film className="w-4 h-4" /> Recordings
          </button>
          {isAdmin && (
            <button onClick={() => navigate('/activity')} className="text-gray-400 hover:text-white flex items-center gap-1 text-sm" title="Activity log">
              <Activity className="w-4 h-4" /> Activity
            </button>
          )}
          <button onClick={() => navigate('/team')} className="text-gray-400 hover:text-white flex items-center gap-1 text-sm" title="Team">
            <Users className="w-4 h-4" /> Team
          </button>
          <span className="text-sm text-gray-400">{user?.display_name} <span className="text-xs text-gray-600">({user?.role})</span></span>
          <button onClick={handleLogout} className="text-gray-400 hover:text-white" title="Sign out">
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-5xl mx-auto px-6 py-8">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white">Machines</h2>
          {isAdmin && (
            <div className="flex items-center gap-2">
              <div className="relative group">
                <button
                  className="flex items-center gap-2 px-4 py-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-gray-300 rounded text-sm font-medium"
                  title="Download agent installer"
                >
                  <Download className="w-4 h-4" /> Download Agent ▾
                </button>
                <div className="absolute right-0 mt-1 hidden group-hover:block bg-gray-900 border border-gray-700 rounded shadow-lg z-10 min-w-[240px]">
                  {(() => {
                    const base = import.meta.env.VITE_API_URL || '';
                    const t = localStorage.getItem('access_token') || '';
                    const qs = `?token=${encodeURIComponent(t)}`;
                    return (
                      <>
                        <a
                          href={`${base}/downloads/agent/windows${qs}`}
                          className="block px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 border-b border-gray-800"
                        >
                          <div className="font-medium">Windows</div>
                          <div className="text-xs text-gray-500">callmor-agent-setup.exe</div>
                        </a>
                        <a
                          href={`${base}/downloads/agent/macos${qs}`}
                          className="block px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 border-b border-gray-800"
                        >
                          <div className="font-medium">macOS</div>
                          <div className="text-xs text-gray-500">callmor-agent.pkg</div>
                        </a>
                        <a
                          href={`${base}/downloads/agent/linux${qs}`}
                          className="block px-4 py-2 text-sm text-gray-300 hover:bg-gray-800"
                        >
                          <div className="font-medium">Linux</div>
                          <div className="text-xs text-gray-500">callmor-agent.deb</div>
                        </a>
                      </>
                    );
                  })()}
                </div>
              </div>
              {isOwner && (
                <button
                  onClick={openTokenModal}
                  className="flex items-center gap-2 px-4 py-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-gray-300 rounded text-sm font-medium"
                  title="View or rotate the enrollment token baked into your installers"
                >
                  <Key className="w-4 h-4" /> Token
                </button>
              )}
              <button
                onClick={() => { setShowAddModal(true); setNewMachineName(''); setNewMachineResult(null); }}
                className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
              >
                <Plus className="w-4 h-4" /> Add Machine
              </button>
            </div>
          )}
        </div>

        {loading ? (
          <p className="text-gray-500">Loading...</p>
        ) : machines.length === 0 ? (
          <div className="bg-gray-900 border border-gray-800 rounded-lg p-12 text-center">
            <Monitor className="w-12 h-12 text-gray-600 mx-auto mb-4" />
            <p className="text-gray-400 mb-2">No machines registered</p>
            <p className="text-sm text-gray-600">Add a machine to get started with remote access</p>
          </div>
        ) : (
          <div className="grid gap-3">
            {machines.map((m) => (
              <div
                key={m.id}
                className="bg-gray-900 border border-gray-800 rounded-lg p-4 flex items-center justify-between hover:border-gray-700"
              >
                <div className="flex items-center gap-4">
                  <div className={`p-2 rounded ${m.is_online ? 'bg-green-900/30' : 'bg-gray-800'}`}>
                    {m.is_online ? (
                      <Wifi className="w-5 h-5 text-green-400" />
                    ) : (
                      <WifiOff className="w-5 h-5 text-gray-500" />
                    )}
                  </div>
                  <div>
                    <div className="text-white font-medium">{m.name}</div>
                    <div className="text-sm text-gray-500">
                      {m.hostname || 'No hostname'} {m.os ? `(${m.os})` : ''}
                      {' '}&middot;{' '}
                      {m.last_seen ? `Last seen ${new Date(m.last_seen).toLocaleString()}` : 'Never connected'}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {m.access_mode === 'restricted' && (
                    <span className="text-xs text-yellow-500 flex items-center gap-1" title="Restricted access">
                      <Lock className="w-3 h-3" /> Restricted
                    </span>
                  )}
                  <button
                    onClick={() => handleConnect(m, 'view_only')}
                    disabled={!m.is_online}
                    className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 disabled:opacity-30 text-gray-300 rounded text-sm flex items-center gap-1"
                    title="View only"
                  >
                    <Eye className="w-4 h-4" /> View
                  </button>
                  <button
                    onClick={() => handleConnect(m, 'full_control')}
                    disabled={!m.is_online}
                    className="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-30 disabled:cursor-not-allowed text-white rounded text-sm"
                  >
                    Connect
                  </button>
                  {isAdmin && (
                    <>
                      <button
                        onClick={() => openAccessModal(m)}
                        className="p-1.5 text-gray-500 hover:text-white"
                        title="Access control"
                      >
                        <Settings className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => handleDeleteMachine(m.id, m.name)}
                        className="p-1.5 text-gray-500 hover:text-red-400"
                        title="Delete"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </main>

      {/* Add Machine Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-md">
            {!newMachineResult ? (
              <>
                <h3 className="text-lg font-semibold text-white mb-4">Add Machine</h3>
                <div className="mb-4">
                  <label className="block text-sm text-gray-400 mb-1">Machine Name</label>
                  <input
                    type="text"
                    value={newMachineName}
                    onChange={(e) => setNewMachineName(e.target.value)}
                    placeholder="e.g., Office PC, Dev Server"
                    className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                    onKeyDown={(e) => e.key === 'Enter' && handleAddMachine()}
                    autoFocus
                  />
                </div>
                <div className="flex gap-2 justify-end">
                  <button onClick={() => setShowAddModal(false)} className="px-4 py-2 text-gray-400 hover:text-white">
                    Cancel
                  </button>
                  <button onClick={handleAddMachine} className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">
                    Create
                  </button>
                </div>
              </>
            ) : (
              <>
                <h3 className="text-lg font-semibold text-white mb-2">Machine Created</h3>
                <p className="text-sm text-gray-400 mb-4">
                  Copy this agent token and use it to configure the agent on <strong>{newMachineResult.name}</strong>.
                  This token is shown only once.
                </p>
                <div className="bg-gray-800 border border-gray-700 rounded p-3 mb-4 flex items-center gap-2">
                  <code className="text-green-400 text-xs break-all flex-1">{newMachineResult.agent_token}</code>
                  <button
                    onClick={() => navigator.clipboard.writeText(newMachineResult.agent_token)}
                    className="p-1 text-gray-400 hover:text-white shrink-0"
                    title="Copy"
                  >
                    <Copy className="w-4 h-4" />
                  </button>
                </div>
                <div className="bg-gray-800 border border-gray-700 rounded p-3 mb-4 space-y-3">
                  <div>
                    <p className="text-xs text-gray-400 mb-1">Linux — edit <code>/etc/callmor-agent/agent.conf</code>:</p>
                    <code className="text-xs text-blue-300 block">
                      MACHINE_ID={newMachineResult.id}<br/>
                      AGENT_TOKEN={newMachineResult.agent_token}
                    </code>
                  </div>
                  <div>
                    <p className="text-xs text-gray-400 mb-1">Windows — edit <code>C:\ProgramData\Callmor\agent.conf</code>:</p>
                    <code className="text-xs text-blue-300 block">
                      MACHINE_ID={newMachineResult.id}<br/>
                      AGENT_TOKEN={newMachineResult.agent_token}
                    </code>
                  </div>
                </div>
                <div className="flex justify-end">
                  <button onClick={() => setShowAddModal(false)} className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">
                    Done
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {/* Access control modal */}
      {accessModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-md">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">Access: {accessModal.name}</h3>
              <button onClick={() => setAccessModal(null)} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="mb-4">
              <div className="flex items-center justify-between bg-gray-800 border border-gray-700 rounded p-3">
                <div className="flex items-center gap-2">
                  {accessModal.access_mode === 'public' ? (
                    <><Globe className="w-4 h-4 text-green-400" /><span className="text-white text-sm">Public (all org members)</span></>
                  ) : (
                    <><Lock className="w-4 h-4 text-yellow-500" /><span className="text-white text-sm">Restricted (specific users only)</span></>
                  )}
                </div>
                <button
                  onClick={handleToggleAccessMode}
                  className="text-xs bg-gray-700 hover:bg-gray-600 text-white px-2 py-1 rounded"
                >
                  Switch to {accessModal.access_mode === 'public' ? 'Restricted' : 'Public'}
                </button>
              </div>
            </div>

            {accessModal.access_mode === 'restricted' && (
              <>
                <h4 className="text-sm text-gray-400 mb-2 uppercase tracking-wide">Users with access ({accessUsers.length})</h4>
                {accessUsers.length === 0 ? (
                  <p className="text-sm text-gray-500 mb-3">No users have access yet. Only admins and owners can access this machine.</p>
                ) : (
                  <div className="space-y-1 mb-4">
                    {accessUsers.map((u) => (
                      <div key={u.user_id} className="flex items-center justify-between bg-gray-800 rounded px-3 py-2">
                        <div className="text-sm">
                          <div className="text-white">{u.display_name}</div>
                          <div className="text-xs text-gray-500">{u.email}</div>
                        </div>
                        <button onClick={() => handleRevokeAccess(u.user_id)} className="text-gray-500 hover:text-red-400" title="Revoke">
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <h4 className="text-sm text-gray-400 mb-2 uppercase tracking-wide">Grant access</h4>
                <div className="space-y-1 max-h-40 overflow-y-auto">
                  {orgUsers
                    .filter((u) => u.role === 'member' && !accessUsers.some((a) => a.user_id === u.id))
                    .map((u) => (
                      <div key={u.id} className="flex items-center justify-between bg-gray-800 rounded px-3 py-2">
                        <div className="text-sm">
                          <div className="text-white">{u.display_name}</div>
                          <div className="text-xs text-gray-500">{u.email}</div>
                        </div>
                        <button onClick={() => handleGrantAccess(u.id)} className="text-xs bg-blue-600 hover:bg-blue-700 text-white px-2 py-1 rounded">
                          Grant
                        </button>
                      </div>
                    ))}
                </div>
              </>
            )}

            <div className="flex justify-end mt-4">
              <button onClick={() => setAccessModal(null)} className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm">
                Done
              </button>
            </div>
          </div>
        </div>
      )}

      {showTokenModal && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg max-w-lg w-full p-6">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <Key className="w-5 h-5" /> Enrollment Token
              </h3>
              <button onClick={() => setShowTokenModal(false)} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>

            <p className="text-sm text-gray-400 mb-4">
              Every installer downloaded from this dashboard has your token baked in, so new machines enroll automatically with no setup. Treat this token like a password — anyone with it can enroll a machine into your tenant.
            </p>

            <div className="bg-gray-950 border border-gray-800 rounded px-3 py-2 flex items-center gap-2 mb-3">
              <code className="text-sm text-gray-200 flex-1 break-all">
                {enrollmentToken || 'Loading...'}
              </code>
              {enrollmentToken && (
                <button
                  onClick={() => { navigator.clipboard.writeText(enrollmentToken); }}
                  className="text-gray-400 hover:text-white p-1"
                  title="Copy"
                >
                  <Copy className="w-4 h-4" />
                </button>
              )}
            </div>

            <p className="text-xs text-gray-500 mb-4">
              Rotating revokes the current token immediately. New installer downloads use the new token; already-enrolled machines keep working because they have their own permanent credentials.
            </p>

            <div className="flex justify-end gap-2">
              <button
                onClick={rotateToken}
                disabled={rotating || !enrollmentToken}
                className="flex items-center gap-2 px-4 py-2 bg-red-700 hover:bg-red-600 disabled:opacity-50 text-white rounded text-sm font-medium"
              >
                <RefreshCw className={`w-4 h-4 ${rotating ? 'animate-spin' : ''}`} />
                {rotating ? 'Rotating...' : 'Rotate'}
              </button>
              <button
                onClick={() => setShowTokenModal(false)}
                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm font-medium"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
