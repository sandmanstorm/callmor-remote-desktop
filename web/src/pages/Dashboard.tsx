import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { machinesApi, sessionsApi, machineAccessApi, usersApi, enrollmentApi, adhocApi, errMsg } from '../lib/api';
import type { Machine, AccessUser, User } from '../lib/api';
import { Monitor, Plus, Trash2, LogOut, Copy, Wifi, WifiOff, Download, Users, Eye, Settings, Lock, Globe, X, Shield, Activity, Film, RefreshCw, Key, Link2, ExternalLink, CheckCircle2 } from 'lucide-react';

// Format "123456789" → "123 456 789"
function formatRdId(id: string): string {
  const digits = id.replace(/\D/g, '');
  if (digits.length !== 9) return id;
  return `${digits.slice(0, 3)} ${digits.slice(3, 6)} ${digits.slice(6, 9)}`;
}

export default function Dashboard() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [machines, setMachines] = useState<Machine[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddModal, setShowAddModal] = useState(false);
  const [newMachineName, setNewMachineName] = useState('');
  const [newMachineRdId, setNewMachineRdId] = useState('');
  const [newMachinePassword, setNewMachinePassword] = useState('');
  const [addingMachine, setAddingMachine] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [accessModal, setAccessModal] = useState<Machine | null>(null);
  const [accessUsers, setAccessUsers] = useState<AccessUser[]>([]);
  const [orgUsers, setOrgUsers] = useState<User[]>([]);

  const isAdmin = user?.role === 'owner' || user?.role === 'admin';
  const isOwner = user?.role === 'owner';
  const [showTokenModal, setShowTokenModal] = useState(false);
  const [enrollmentToken, setEnrollmentToken] = useState<string | null>(null);
  const [rotating, setRotating] = useState(false);

  const showToast = (msg: string, ms = 3000) => {
    setToast(msg);
    window.setTimeout(() => setToast(null), ms);
  };

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

  // --- Claim (adhoc -> tenant) ---
  const [showClaimModal, setShowClaimModal] = useState(false);
  const [claimCode, setClaimCode] = useState('');
  const [claimPin, setClaimPin] = useState('');
  const [claimName, setClaimName] = useState('');
  const [claiming, setClaiming] = useState(false);
  const [claimError, setClaimError] = useState<string | null>(null);

  const openClaim = () => {
    setShowClaimModal(true);
    setClaimCode(''); setClaimPin(''); setClaimName(''); setClaimError(null);
  };

  const submitClaim = async () => {
    setClaimError(null);
    setClaiming(true);
    try {
      await adhocApi.claim({
        access_code: claimCode,
        pin: claimPin,
        name: claimName.trim() || undefined,
      });
      setShowClaimModal(false);
      fetchMachines();
    } catch (err: any) {
      setClaimError(errMsg(err, 'Claim failed'));
    } finally {
      setClaiming(false);
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

  const openAddModal = () => {
    setShowAddModal(true);
    setNewMachineName('');
    setNewMachineRdId('');
    setNewMachinePassword('');
    setAddError(null);
  };

  const handleAddMachine = async () => {
    setAddError(null);
    const name = newMachineName.trim();
    const rawId = newMachineRdId.replace(/\D/g, '');
    const password = newMachinePassword;

    if (!name) {
      setAddError('Display name is required');
      return;
    }
    if (rawId.length !== 9) {
      setAddError('RustDesk ID must be exactly 9 digits');
      return;
    }
    if (!password) {
      setAddError('Permanent password is required');
      return;
    }

    setAddingMachine(true);
    try {
      await machinesApi.create({
        name,
        rustdesk_id: rawId,
        rustdesk_password: password,
      });
      setShowAddModal(false);
      await fetchMachines();
      showToast('Machine added. Click Connect to launch remote session.');
    } catch (err: any) {
      setAddError(errMsg(err, 'Failed to add machine'));
    } finally {
      setAddingMachine(false);
    }
  };

  const handleDeleteMachine = async (id: string, name: string) => {
    if (!confirm(`Delete machine "${name}"?`)) return;
    await machinesApi.delete(id);
    fetchMachines();
  };

  // New RustDesk connect flow — launches rustdesk:// URI
  const handleRdConnect = async (machine: Machine) => {
    try {
      const { data } = await machinesApi.rdConnect(machine.id);
      showToast('Launching RustDesk…');
      window.location.href = data.launch_uri;
    } catch (err: any) {
      const status = err?.response?.status;
      if (status === 403) {
        alert('Access denied: you do not have permission to connect to this machine.');
      } else if (status === 404) {
        alert('This machine has no RustDesk ID configured. Edit the machine to add one.');
      } else {
        alert(errMsg(err, 'Failed to launch RustDesk session'));
      }
    }
  };

  // Legacy WebRTC connect flow (for webrtc_legacy machines)
  const handleLegacyConnect = async (machine: Machine, permission: 'full_control' | 'view_only' = 'full_control') => {
    try {
      const { data } = await sessionsApi.create(machine.id, permission);
      const params = new URLSearchParams({
        relay: data.relay_url,
        token: data.session_token,
        permission,
        session: data.session.id,
        hostname: machine.hostname || machine.name,
      });
      window.open(`/viewer/${encodeURIComponent(data.machine_id)}?${params.toString()}`, '_blank');
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
    } catch { /* ignore */ }
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

  const copyRdId = (id: string) => {
    navigator.clipboard.writeText(id);
    showToast('ID copied');
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

      {/* Toast */}
      {toast && (
        <div className="fixed top-16 left-1/2 -translate-x-1/2 z-50 bg-gray-800 border border-gray-700 text-white text-sm px-4 py-2 rounded shadow-lg flex items-center gap-2">
          <CheckCircle2 className="w-4 h-4 text-green-400" />
          {toast}
        </div>
      )}

      {/* Content */}
      <main className="max-w-5xl mx-auto px-6 py-8">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white">Machines</h2>
          {isAdmin && (
            <div className="flex items-center gap-2">
              <div className="relative group">
                <button
                  className="flex items-center gap-2 px-4 py-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-gray-300 rounded text-sm font-medium"
                  title="Download RustDesk client installer"
                >
                  <Download className="w-4 h-4" /> Download ▾
                </button>
                <div className="absolute right-0 mt-1 hidden group-hover:block bg-gray-900 border border-gray-700 rounded shadow-lg z-10 min-w-[280px]">
                  {(() => {
                    const base = import.meta.env.VITE_API_URL || '';
                    const t = localStorage.getItem('access_token') || '';
                    const qs = `?token=${encodeURIComponent(t)}`;
                    return (
                      <>
                        <a
                          href={`${base}/downloads/rustdesk/windows${qs}`}
                          className="block px-4 py-2 text-sm text-white hover:bg-gray-800 border-b border-gray-800"
                        >
                          <div className="flex items-center gap-2">
                            <span className="text-xs px-1.5 py-0.5 rounded bg-purple-900/50 border border-purple-700 text-purple-300 uppercase tracking-wide">Recommended</span>
                          </div>
                          <div className="font-medium mt-1">Callmor-RustDesk (Windows)</div>
                          <div className="text-xs text-gray-500">Pre-configured installer</div>
                        </a>
                        <a
                          href={`${base}/downloads/rustdesk/macos${qs}`}
                          className="block px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 border-b border-gray-800"
                        >
                          <div className="font-medium">Callmor-RustDesk (macOS)</div>
                          <div className="text-xs text-gray-500">Pre-configured installer</div>
                        </a>
                        <a
                          href={`${base}/downloads/rustdesk/linux${qs}`}
                          className="block px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 border-b border-gray-800"
                        >
                          <div className="font-medium">Callmor-RustDesk (Linux)</div>
                          <div className="text-xs text-gray-500">Pre-configured installer</div>
                        </a>
                        <a
                          href={`${base}/downloads/agent/public/windows`}
                          className="block px-4 py-2 text-xs text-gray-500 hover:bg-gray-800 hover:text-gray-400"
                        >
                          Legacy Callmor Agent (Windows)
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
                onClick={openClaim}
                className="flex items-center gap-2 px-4 py-2 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-gray-300 rounded text-sm font-medium"
                title="Import a computer that registered with a public installer (code + PIN)"
              >
                <Link2 className="w-4 h-4" /> Claim
              </button>
              <button
                onClick={openAddModal}
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
          <div className="bg-gray-900 border border-gray-800 rounded-lg p-10 max-w-2xl mx-auto">
            <div className="text-center mb-8">
              <Monitor className="w-12 h-12 text-gray-600 mx-auto mb-3" />
              <h3 className="text-lg font-semibold text-white mb-1">Get started with remote access</h3>
              <p className="text-sm text-gray-500">Three steps and you're connected.</p>
            </div>
            <ol className="space-y-5">
              <li className="flex gap-4">
                <div className="w-8 h-8 rounded-full bg-blue-600 text-white font-semibold flex items-center justify-center text-sm flex-shrink-0">1</div>
                <div className="flex-1">
                  <h4 className="text-white font-medium mb-1">Install Callmor-RustDesk</h4>
                  <p className="text-sm text-gray-400 mb-3">
                    Run the pre-configured installer on every machine you want to reach.
                  </p>
                  <div className="flex flex-wrap items-center gap-3">
                    <a
                      href="/rustdesk-setup"
                      className="inline-flex items-center gap-2 px-4 py-2 bg-purple-600 hover:bg-purple-700 text-white rounded text-sm font-medium"
                    >
                      <Download className="w-4 h-4" /> Download &amp; setup guide
                    </a>
                    <a
                      href="/rustdesk-setup"
                      className="text-xs text-gray-500 hover:text-gray-300 inline-flex items-center gap-1"
                    >
                      View all platforms <ExternalLink className="w-3 h-3" />
                    </a>
                  </div>
                </div>
              </li>
              <li className="flex gap-4">
                <div className="w-8 h-8 rounded-full bg-blue-600 text-white font-semibold flex items-center justify-center text-sm flex-shrink-0">2</div>
                <div className="flex-1">
                  <h4 className="text-white font-medium mb-1">Note your 9-digit ID</h4>
                  <p className="text-sm text-gray-400">
                    After install, open Callmor-RustDesk on the machine. The main window
                    shows a 9-digit ID like <span className="font-mono text-gray-200">123 456 789</span>.
                    Then set a permanent password under Settings → Security → Unlock Security Settings → Permanent Password.
                  </p>
                </div>
              </li>
              <li className="flex gap-4">
                <div className="w-8 h-8 rounded-full bg-blue-600 text-white font-semibold flex items-center justify-center text-sm flex-shrink-0">3</div>
                <div className="flex-1">
                  <h4 className="text-white font-medium mb-1">Add Machine</h4>
                  <p className="text-sm text-gray-400 mb-3">
                    Enter the ID and permanent password here. After that, click Connect any time to launch a remote session.
                  </p>
                  {isAdmin ? (
                    <button
                      onClick={openAddModal}
                      className="inline-flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
                    >
                      <Plus className="w-4 h-4" /> Add Machine
                    </button>
                  ) : (
                    <p className="text-xs text-gray-500">
                      Only admins or owners can add machines. Ask your administrator to invite the machine.
                    </p>
                  )}
                </div>
              </li>
            </ol>
          </div>
        ) : (
          <div className="grid gap-3">
            {machines.map((m) => {
              const isLegacy = m.connection_type === 'webrtc_legacy' || !m.rustdesk_id;
              return (
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
                      <div className="text-white font-medium flex items-center gap-2 flex-wrap">
                        <span>{m.name}</span>
                        <span className={`inline-block w-2 h-2 rounded-full ${m.is_online ? 'bg-green-400' : 'bg-gray-500'}`} title={m.is_online ? 'Online' : 'Offline'} />
                        {!isLegacy && m.rustdesk_id && (
                          <span className="flex items-center gap-1 text-xs font-mono text-gray-400 bg-gray-800 border border-gray-700 rounded px-2 py-0.5">
                            {formatRdId(m.rustdesk_id)}
                            <button
                              onClick={() => copyRdId(m.rustdesk_id!)}
                              className="text-gray-500 hover:text-white"
                              title="Copy RustDesk ID"
                            >
                              <Copy className="w-3 h-3" />
                            </button>
                          </span>
                        )}
                        {isLegacy && (
                          <span className="text-[10px] uppercase tracking-wide bg-amber-950/60 border border-amber-800/70 text-amber-300 rounded px-1.5 py-0.5">
                            legacy
                          </span>
                        )}
                      </div>
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
                    {isLegacy ? (
                      <>
                        <button
                          onClick={() => handleLegacyConnect(m, 'view_only')}
                          disabled={!m.is_online}
                          className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 disabled:opacity-30 text-gray-300 rounded text-sm flex items-center gap-1"
                          title="View only (legacy WebRTC)"
                        >
                          <Eye className="w-4 h-4" /> View
                        </button>
                        <button
                          onClick={() => handleLegacyConnect(m, 'full_control')}
                          disabled={!m.is_online}
                          className="px-4 py-1.5 bg-amber-700 hover:bg-amber-600 disabled:opacity-30 disabled:cursor-not-allowed text-white rounded text-sm"
                          title="Legacy WebRTC session (opens in browser)"
                        >
                          Legacy Connect
                        </button>
                      </>
                    ) : (
                      <button
                        onClick={() => handleRdConnect(m)}
                        className="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm"
                        title="Launch Callmor-RustDesk"
                      >
                        Connect
                      </button>
                    )}
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
              );
            })}
          </div>
        )}
      </main>

      {/* Add Machine Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-md">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">Add Machine</h3>
              <button onClick={() => setShowAddModal(false)} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Display name</label>
                <input
                  type="text"
                  value={newMachineName}
                  onChange={(e) => setNewMachineName(e.target.value)}
                  placeholder="e.g., Office PC, Dev Server"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">RustDesk ID</label>
                <input
                  type="text"
                  inputMode="numeric"
                  value={newMachineRdId}
                  onChange={(e) => setNewMachineRdId(e.target.value)}
                  placeholder="123 456 789"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white placeholder-gray-500 font-mono tracking-widest focus:outline-none focus:border-blue-500"
                  maxLength={13}
                />
                <p className="text-xs text-gray-500 mt-1">
                  Install Callmor-RustDesk on the machine, open it, and copy the ID shown in the main window.
                </p>
              </div>

              <div>
                <label className="block text-sm text-gray-400 mb-1">Permanent password</label>
                <input
                  type="password"
                  value={newMachinePassword}
                  onChange={(e) => setNewMachinePassword(e.target.value)}
                  placeholder="The password set in RustDesk"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                />
                <p className="text-xs text-gray-500 mt-1">
                  Set a permanent password in RustDesk: Settings → Security → Unlock Security Settings → Permanent Password.
                </p>
              </div>

              <div>
                <a href="/rustdesk-setup" className="text-xs text-blue-400 hover:text-blue-300 inline-flex items-center gap-1">
                  Don't have RustDesk installed? <ExternalLink className="w-3 h-3" />
                </a>
              </div>
            </div>

            {addError && (
              <div className="mt-4 p-3 bg-red-950/50 border border-red-900 rounded text-sm text-red-300">
                {addError}
              </div>
            )}

            <div className="flex gap-2 justify-end mt-5">
              <button
                onClick={() => setShowAddModal(false)}
                className="px-4 py-2 text-gray-400 hover:text-white"
                disabled={addingMachine}
              >
                Cancel
              </button>
              <button
                onClick={handleAddMachine}
                disabled={addingMachine}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded"
              >
                {addingMachine ? 'Adding…' : 'Add Machine'}
              </button>
            </div>
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

      {showClaimModal && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg max-w-md w-full p-6">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <Link2 className="w-5 h-5" /> Claim a Computer
              </h3>
              <button onClick={() => setShowClaimModal(false)} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>

            <p className="text-sm text-gray-400 mb-4">
              Enter the access code + PIN shown on the remote computer. Once claimed, the machine becomes part of your account and you can manage it like any other.
            </p>

            <div className="space-y-3">
              <div>
                <label className="block text-sm text-gray-400 mb-1">Access Code</label>
                <input
                  type="text"
                  value={claimCode}
                  onChange={(e) => setClaimCode(e.target.value.toUpperCase())}
                  placeholder="ABCD-1234"
                  autoFocus
                  autoComplete="off"
                  className="w-full px-3 py-2 bg-gray-950 border border-gray-700 rounded font-mono tracking-widest text-center text-white focus:border-blue-500 focus:outline-none"
                  maxLength={10}
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">PIN</label>
                <input
                  type="text"
                  inputMode="numeric"
                  pattern="[0-9]*"
                  value={claimPin}
                  onChange={(e) => setClaimPin(e.target.value.replace(/\D/g, ''))}
                  placeholder="1234"
                  autoComplete="off"
                  className="w-full px-3 py-2 bg-gray-950 border border-gray-700 rounded font-mono tracking-widest text-center text-white focus:border-blue-500 focus:outline-none"
                  maxLength={4}
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">Name (optional)</label>
                <input
                  type="text"
                  value={claimName}
                  onChange={(e) => setClaimName(e.target.value)}
                  placeholder="e.g. Office Desktop"
                  className="w-full px-3 py-2 bg-gray-950 border border-gray-700 rounded text-white focus:border-blue-500 focus:outline-none"
                  maxLength={100}
                />
              </div>
            </div>

            {claimError && (
              <div className="mt-3 p-3 bg-red-950/50 border border-red-900 rounded text-sm text-red-300">
                {claimError}
              </div>
            )}

            <div className="flex justify-end gap-2 mt-5">
              <button
                onClick={() => setShowClaimModal(false)}
                className="px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm font-medium"
              >
                Cancel
              </button>
              <button
                onClick={submitClaim}
                disabled={claiming || !claimCode || claimPin.length !== 4}
                className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded text-sm font-medium"
              >
                {claiming ? 'Claiming…' : 'Claim'}
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
