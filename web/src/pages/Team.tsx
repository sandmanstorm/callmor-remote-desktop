import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { usersApi, invitationsApi, errMsg } from '../lib/api';
import type { User, Invitation, CreateInvitationResponse } from '../lib/api';
import { Monitor, Users, Mail, Trash2, LogOut, Copy, ArrowLeft, UserPlus, Crown, Shield } from 'lucide-react';

export default function Team() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [users, setUsers] = useState<User[]>([]);
  const [invitations, setInvitations] = useState<Invitation[]>([]);
  const [loading, setLoading] = useState(true);
  const [showInviteModal, setShowInviteModal] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('member');
  const [inviteResult, setInviteResult] = useState<CreateInvitationResponse | null>(null);

  const isOwner = user?.role === 'owner';
  const isAdmin = user?.role === 'admin' || isOwner;

  const fetchData = async () => {
    try {
      const [u, i] = await Promise.all([
        usersApi.list(),
        isAdmin ? invitationsApi.list() : Promise.resolve({ data: [] }),
      ]);
      setUsers(u.data);
      setInvitations(i.data as Invitation[]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  const handleInvite = async () => {
    if (!inviteEmail.trim()) return;
    try {
      const { data } = await invitationsApi.create(inviteEmail.trim(), inviteRole);
      setInviteResult(data);
      fetchData();
    } catch (err: any) {
      alert(errMsg(err, 'Failed to create invitation'));
    }
  };

  const handleChangeRole = async (id: string, newRole: string) => {
    if (!confirm(`Change role to ${newRole}?`)) return;
    try {
      await usersApi.update(id, newRole);
      fetchData();
    } catch (err: any) {
      alert(errMsg(err, 'Failed to change role'));
    }
  };

  const handleRemoveUser = async (id: string, name: string) => {
    if (!confirm(`Remove ${name} from the organization?`)) return;
    try {
      await usersApi.delete(id);
      fetchData();
    } catch (err: any) {
      alert(errMsg(err, 'Failed to remove user'));
    }
  };

  const handleRevokeInvite = async (id: string) => {
    await invitationsApi.delete(id);
    fetchData();
  };

  const handleLogout = () => {
    logout();
    navigate('/login');
  };

  const inviteLink = (token: string) =>
    `${window.location.origin}/invite/${token}`;

  return (
    <div className="min-h-screen bg-gray-950">
      <header className="bg-gray-900 border-b border-gray-800 px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Monitor className="w-6 h-6 text-blue-400" />
          <h1 className="text-lg font-semibold text-white">Callmor Remote</h1>
          <span className="text-sm text-gray-500">|</span>
          <span className="text-sm text-gray-400">{user?.tenant_name}</span>
        </div>
        <div className="flex items-center gap-4">
          <span className="text-sm text-gray-400">{user?.display_name}</span>
          <button onClick={handleLogout} className="text-gray-400 hover:text-white" title="Sign out">
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-6 py-8">
        <button
          onClick={() => navigate('/app')}
          className="flex items-center gap-1 text-gray-400 hover:text-white text-sm mb-4"
        >
          <ArrowLeft className="w-4 h-4" /> Back to Machines
        </button>

        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white flex items-center gap-2">
            <Users className="w-5 h-5" /> Team
          </h2>
          {isAdmin && (
            <button
              onClick={() => { setShowInviteModal(true); setInviteEmail(''); setInviteRole('member'); setInviteResult(null); }}
              className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
            >
              <UserPlus className="w-4 h-4" /> Invite User
            </button>
          )}
        </div>

        {loading ? (
          <p className="text-gray-500">Loading...</p>
        ) : (
          <>
            {/* Pending invitations */}
            {invitations.length > 0 && (
              <div className="mb-6">
                <h3 className="text-sm text-gray-400 mb-2 uppercase tracking-wide">Pending Invitations</h3>
                <div className="grid gap-2">
                  {invitations.map((inv) => (
                    <div key={inv.id} className="bg-gray-900 border border-gray-800 rounded px-4 py-3 flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <Mail className="w-4 h-4 text-yellow-500" />
                        <div>
                          <div className="text-white text-sm">{inv.email}</div>
                          <div className="text-xs text-gray-500">{inv.role} · expires {new Date(inv.expires_at).toLocaleDateString()}</div>
                        </div>
                      </div>
                      {isAdmin && (
                        <button
                          onClick={() => handleRevokeInvite(inv.id)}
                          className="text-gray-500 hover:text-red-400 p-1"
                          title="Revoke invitation"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Users */}
            <h3 className="text-sm text-gray-400 mb-2 uppercase tracking-wide">Members ({users.length})</h3>
            <div className="grid gap-2">
              {users.map((u) => (
                <div key={u.id} className="bg-gray-900 border border-gray-800 rounded px-4 py-3 flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    {u.role === 'owner' ? (
                      <Crown className="w-4 h-4 text-yellow-500" />
                    ) : u.role === 'admin' ? (
                      <Shield className="w-4 h-4 text-blue-400" />
                    ) : (
                      <Users className="w-4 h-4 text-gray-500" />
                    )}
                    <div>
                      <div className="text-white text-sm">{u.display_name}{u.id === user?.id && <span className="text-gray-500 text-xs ml-2">(you)</span>}</div>
                      <div className="text-xs text-gray-500">{u.email} · {u.role}</div>
                    </div>
                  </div>
                  {isOwner && u.id !== user?.id && u.role !== 'owner' && (
                    <div className="flex gap-2">
                      <select
                        value={u.role}
                        onChange={(e) => handleChangeRole(u.id, e.target.value)}
                        className="bg-gray-800 border border-gray-700 text-gray-300 text-xs rounded px-2 py-1"
                      >
                        <option value="member">Member</option>
                        <option value="admin">Admin</option>
                        <option value="owner">Owner</option>
                      </select>
                      <button
                        onClick={() => handleRemoveUser(u.id, u.display_name)}
                        className="text-gray-500 hover:text-red-400 p-1"
                        title="Remove user"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </>
        )}
      </main>

      {showInviteModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-md">
            {!inviteResult ? (
              <>
                <h3 className="text-lg font-semibold text-white mb-4">Invite User</h3>
                <div className="mb-3">
                  <label className="block text-sm text-gray-400 mb-1">Email</label>
                  <input
                    type="email"
                    value={inviteEmail}
                    onChange={(e) => setInviteEmail(e.target.value)}
                    className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white"
                    placeholder="colleague@example.com"
                    autoFocus
                  />
                </div>
                <div className="mb-4">
                  <label className="block text-sm text-gray-400 mb-1">Role</label>
                  <select
                    value={inviteRole}
                    onChange={(e) => setInviteRole(e.target.value)}
                    className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white"
                  >
                    <option value="member">Member (view assigned machines)</option>
                    {isOwner && <option value="admin">Admin (manage machines + invite)</option>}
                  </select>
                </div>
                <div className="flex gap-2 justify-end">
                  <button onClick={() => setShowInviteModal(false)} className="px-4 py-2 text-gray-400">Cancel</button>
                  <button onClick={handleInvite} className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">Create Invitation</button>
                </div>
              </>
            ) : (
              <>
                <h3 className="text-lg font-semibold text-white mb-2">Invitation Created</h3>
                {(inviteResult as any).email_sent ? (
                  <div className="bg-green-900/30 border border-green-700 rounded px-3 py-2 mb-4 text-green-300 text-sm">
                    ✓ Invitation email sent to <strong>{inviteResult.email}</strong>
                  </div>
                ) : (
                  <p className="text-sm text-yellow-400 mb-4">
                    SMTP not configured — copy this link and send it manually to <strong>{inviteResult.email}</strong>.
                  </p>
                )}
                <p className="text-sm text-gray-400 mb-2">Direct link (expires in 7 days):</p>
                <div className="bg-gray-800 border border-gray-700 rounded p-3 mb-4 flex items-center gap-2">
                  <code className="text-blue-300 text-xs break-all flex-1">{inviteLink(inviteResult.token)}</code>
                  <button
                    onClick={() => navigator.clipboard.writeText(inviteLink(inviteResult.token))}
                    className="p-1 text-gray-400 hover:text-white shrink-0"
                    title="Copy"
                  >
                    <Copy className="w-4 h-4" />
                  </button>
                </div>
                <div className="flex justify-end">
                  <button onClick={() => setShowInviteModal(false)} className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">Done</button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
