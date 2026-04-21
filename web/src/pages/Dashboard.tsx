import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { machinesApi } from '../lib/api';
import type { Machine, CreateMachineResponse } from '../lib/api';
import { Monitor, Plus, Trash2, LogOut, Copy, Wifi, WifiOff } from 'lucide-react';

export default function Dashboard() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [machines, setMachines] = useState<Machine[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddModal, setShowAddModal] = useState(false);
  const [newMachineName, setNewMachineName] = useState('');
  const [newMachineResult, setNewMachineResult] = useState<CreateMachineResponse | null>(null);

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
      alert(err.response?.data || 'Failed to add machine');
    }
  };

  const handleDeleteMachine = async (id: string, name: string) => {
    if (!confirm(`Delete machine "${name}"?`)) return;
    await machinesApi.delete(id);
    fetchMachines();
  };

  const handleConnect = (machine: Machine) => {
    // Open viewer in new tab with machine ID
    const relayUrl = import.meta.env.VITE_RELAY_URL || 'ws://localhost:8080';
    window.open(`/viewer-test.html?relay=${encodeURIComponent(relayUrl)}&machine=${machine.id}`, '_blank');
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
          <span className="text-sm text-gray-400">{user?.display_name}</span>
          <button onClick={handleLogout} className="text-gray-400 hover:text-white" title="Sign out">
            <LogOut className="w-4 h-4" />
          </button>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-5xl mx-auto px-6 py-8">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white">Machines</h2>
          <button
            onClick={() => { setShowAddModal(true); setNewMachineName(''); setNewMachineResult(null); }}
            className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
          >
            <Plus className="w-4 h-4" /> Add Machine
          </button>
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
                  <button
                    onClick={() => handleConnect(m)}
                    disabled={!m.is_online}
                    className="px-4 py-1.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-30 disabled:cursor-not-allowed text-white rounded text-sm"
                  >
                    Connect
                  </button>
                  <button
                    onClick={() => handleDeleteMachine(m.id, m.name)}
                    className="p-1.5 text-gray-500 hover:text-red-400"
                    title="Delete"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
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
                <div className="bg-gray-800 border border-gray-700 rounded p-3 mb-4">
                  <p className="text-xs text-gray-400 mb-1">Run the agent with:</p>
                  <code className="text-xs text-blue-300">
                    AGENT_TOKEN={newMachineResult.agent_token} MACHINE_ID={newMachineResult.id} callmor-agent
                  </code>
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
    </div>
  );
}
