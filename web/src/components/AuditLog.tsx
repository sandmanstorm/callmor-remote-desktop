import { useState, useEffect } from 'react';
import type { AuditEvent } from '../lib/api';
import { User, Shield, Monitor, Building2, UserPlus, LogIn, XCircle, Activity } from 'lucide-react';

interface Props {
  fetchEvents: (params?: { event_type?: string; limit?: number }) => Promise<{ data: AuditEvent[] }>;
  showTenant?: boolean;
}

const EVENT_ICONS: Record<string, { icon: any; color: string }> = {
  'auth.login': { icon: LogIn, color: 'text-green-400' },
  'auth.login_failed': { icon: XCircle, color: 'text-red-400' },
  'tenant.created': { icon: Building2, color: 'text-blue-400' },
  'machine.created': { icon: Monitor, color: 'text-purple-400' },
  'machine.deleted': { icon: Monitor, color: 'text-red-400' },
  'session.started': { icon: Activity, color: 'text-emerald-400' },
  'user.invited': { icon: UserPlus, color: 'text-yellow-400' },
  'user.role_changed': { icon: User, color: 'text-orange-400' },
  'user.removed': { icon: User, color: 'text-red-400' },
  'admin.tenant_deleted': { icon: Building2, color: 'text-red-500' },
  'admin.superadmin_granted': { icon: Shield, color: 'text-red-400' },
  'admin.superadmin_revoked': { icon: Shield, color: 'text-gray-400' },
};

function formatEvent(e: AuditEvent): string {
  const meta = e.metadata || {};
  switch (e.event_type) {
    case 'auth.login': return 'Logged in';
    case 'auth.login_failed': return `Login failed: ${meta.reason || 'unknown'} (${meta.email || ''})`;
    case 'tenant.created': return `Organization "${meta.tenant_name}" created`;
    case 'machine.created': return `Added machine "${meta.name}"`;
    case 'machine.deleted': return `Deleted machine "${meta.name || '?'}"`;
    case 'session.started': return `Started ${meta.permission?.replace('_', ' ')} session on "${meta.machine_name || '?'}"`;
    case 'user.invited': return `Invited ${meta.email} as ${meta.role}`;
    case 'user.role_changed': return `Changed role to ${meta.new_role}`;
    case 'user.removed': return `Removed user ${meta.email || ''}`;
    case 'admin.tenant_deleted': return `Deleted tenant (platform)`;
    case 'admin.superadmin_granted': return `Granted super-admin (platform)`;
    case 'admin.superadmin_revoked': return `Revoked super-admin (platform)`;
    default: return e.event_type;
  }
}

export default function AuditLog({ fetchEvents, showTenant = false }: Props) {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<string>('');

  useEffect(() => {
    load();
  }, [filter]);

  async function load() {
    setLoading(true);
    try {
      const { data } = await fetchEvents(filter ? { event_type: filter } : undefined);
      setEvents(data);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-4">
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="px-3 py-1.5 bg-gray-800 border border-gray-700 rounded text-sm text-gray-300"
        >
          <option value="">All events</option>
          <option value="auth.login">Logins</option>
          <option value="auth.login_failed">Failed logins</option>
          <option value="machine.created">Machines added</option>
          <option value="machine.deleted">Machines deleted</option>
          <option value="session.started">Sessions started</option>
          <option value="user.invited">Invitations</option>
          <option value="user.role_changed">Role changes</option>
          <option value="user.removed">User removals</option>
          {showTenant && <>
            <option value="admin.tenant_deleted">Tenant deletions</option>
            <option value="admin.superadmin_granted">Super-admin grants</option>
          </>}
        </select>
        <button
          onClick={load}
          className="text-sm text-gray-400 hover:text-white"
        >
          Refresh
        </button>
      </div>

      {loading ? (
        <p className="text-gray-500">Loading...</p>
      ) : events.length === 0 ? (
        <p className="text-gray-500">No events.</p>
      ) : (
        <div className="space-y-1">
          {events.map((e) => {
            const { icon: Icon, color } = EVENT_ICONS[e.event_type] || { icon: Activity, color: 'text-gray-400' };
            return (
              <div key={e.id} className="bg-gray-900 border border-gray-800 rounded px-4 py-2.5 flex items-start gap-3">
                <Icon className={`w-4 h-4 mt-0.5 shrink-0 ${color}`} />
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-white">{formatEvent(e)}</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    {e.actor_display || e.actor_email || 'system'}
                    {e.ip_address && <> · {e.ip_address}</>}
                    {' · '}
                    <time>{new Date(e.created_at).toLocaleString()}</time>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
