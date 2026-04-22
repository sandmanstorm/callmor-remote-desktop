import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import { auditApi } from '../lib/api';
import AuditLog from '../components/AuditLog';
import { Monitor, LogOut, ArrowLeft, Activity as ActivityIcon } from 'lucide-react';

export default function Activity() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();

  const handleLogout = () => { logout(); navigate('/login'); };

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
          <button onClick={handleLogout} className="text-gray-400 hover:text-white"><LogOut className="w-4 h-4" /></button>
        </div>
      </header>

      <main className="max-w-5xl mx-auto px-6 py-8">
        <button
          onClick={() => navigate('/')}
          className="flex items-center gap-1 text-gray-400 hover:text-white text-sm mb-4"
        >
          <ArrowLeft className="w-4 h-4" /> Back to Machines
        </button>

        <h2 className="text-xl font-semibold text-white flex items-center gap-2 mb-6">
          <ActivityIcon className="w-5 h-5" /> Activity Log
        </h2>

        <AuditLog fetchEvents={auditApi.listTenant} />
      </main>
    </div>
  );
}
