import { useState, useEffect } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { invitationsApi } from '../lib/api';
import type { InvitationDetails } from '../lib/api';
import { useAuth } from '../lib/auth';

export default function AcceptInvite() {
  const { token } = useParams<{ token: string }>();
  const navigate = useNavigate();
  const { setAuth } = useAuth();
  const [invite, setInvite] = useState<InvitationDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!token) return;
    invitationsApi.getByToken(token)
      .then((res) => setInvite(res.data))
      .catch((err) => setError(err.response?.data || 'Invitation not found or expired'))
      .finally(() => setLoading(false));
  }, [token]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token) return;
    setSubmitting(true);
    setError('');
    try {
      const { data } = await invitationsApi.accept(token, password, displayName);
      setAuth(data.user, data.access_token, data.refresh_token);
      navigate('/');
    } catch (err: any) {
      setError(err.response?.data || 'Failed to accept invitation');
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-950 flex items-center justify-center">
        <p className="text-gray-400">Loading invitation...</p>
      </div>
    );
  }

  if (error && !invite) {
    return (
      <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
        <div className="bg-gray-900 border border-red-800 rounded-lg p-6 max-w-md">
          <h2 className="text-lg font-semibold text-red-400 mb-2">Invitation Invalid</h2>
          <p className="text-gray-400 text-sm">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <h1 className="text-3xl font-bold text-white text-center mb-2">Join {invite?.tenant_name}</h1>
        <p className="text-gray-400 text-center mb-6">
          {invite?.invited_by_name} invited <strong>{invite?.email}</strong> as <strong>{invite?.role}</strong>
        </p>

        <form onSubmit={handleSubmit} className="bg-gray-900 rounded-lg p-6 space-y-4 border border-gray-800">
          {error && (
            <div className="bg-red-900/50 border border-red-700 text-red-300 px-4 py-2 rounded text-sm">
              {error}
            </div>
          )}

          <div>
            <label className="block text-sm text-gray-400 mb-1">Your Name</label>
            <input
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              required
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white"
              autoFocus
            />
          </div>

          <div>
            <label className="block text-sm text-gray-400 mb-1">Choose Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              minLength={8}
              className="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-white"
            />
          </div>

          <button
            type="submit"
            disabled={submitting}
            className="w-full py-2 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white rounded font-medium"
          >
            {submitting ? 'Joining...' : `Join ${invite?.tenant_name}`}
          </button>
        </form>
      </div>
    </div>
  );
}
