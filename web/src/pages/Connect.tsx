import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { adhocApi, errMsg } from '../lib/api';
import { Monitor, Loader2 } from 'lucide-react';

/**
 * Public /connect — anyone with an access code + PIN can connect without
 * an account. The remote agent displays both on the remote screen.
 */
export default function Connect() {
  const navigate = useNavigate();
  const [code, setCode] = useState('');
  const [pin, setPin] = useState('');
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleConnect = async (e: React.FormEvent) => {
    e.preventDefault();
    if (connecting) return;
    setError(null);
    setConnecting(true);
    try {
      const { data } = await adhocApi.connect({ access_code: code, pin });
      const params = new URLSearchParams({
        relay: data.relay_url,
        machine: data.machine_id,
        token: data.session_token,
        permission: 'full_control',
        session: 'adhoc',
        hostname: data.hostname,
      });
      window.location.href = `/viewer-test.html?${params.toString()}`;
    } catch (err: any) {
      setError(errMsg(err, 'Could not connect'));
      setConnecting(false);
    }
  };

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="flex items-center gap-3 mb-6">
          <Monitor className="w-8 h-8 text-blue-400" />
          <h1 className="text-2xl font-semibold">Connect to a Computer</h1>
        </div>
        <p className="text-sm text-gray-400 mb-8">
          Enter the code and PIN shown on the remote computer. No account required.
        </p>

        <form onSubmit={handleConnect} className="space-y-4">
          <div>
            <label className="block text-sm text-gray-400 mb-2">Access Code</label>
            <input
              type="text"
              value={code}
              onChange={(e) => setCode(e.target.value.toUpperCase())}
              placeholder="ABCD-1234"
              autoFocus
              autoComplete="off"
              className="w-full px-4 py-3 bg-gray-900 border border-gray-700 rounded text-lg font-mono tracking-widest text-center text-white focus:border-blue-500 focus:outline-none"
              maxLength={10}
              required
            />
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-2">PIN (4 digits)</label>
            <input
              type="text"
              inputMode="numeric"
              pattern="[0-9]*"
              value={pin}
              onChange={(e) => setPin(e.target.value.replace(/\D/g, ''))}
              placeholder="1234"
              autoComplete="off"
              className="w-full px-4 py-3 bg-gray-900 border border-gray-700 rounded text-lg font-mono tracking-widest text-center text-white focus:border-blue-500 focus:outline-none"
              maxLength={4}
              required
            />
          </div>

          {error && (
            <div className="p-3 bg-red-950/50 border border-red-900 rounded text-sm text-red-300">
              {error}
            </div>
          )}

          <button
            type="submit"
            disabled={connecting || !code || pin.length !== 4}
            className="w-full flex items-center justify-center gap-2 py-3 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded font-medium"
          >
            {connecting ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
            {connecting ? 'Connecting…' : 'Connect'}
          </button>
        </form>

        <div className="mt-10 pt-6 border-t border-gray-800 text-sm text-gray-500">
          <p className="mb-2">Need to let someone connect to this computer?</p>
          <a href="/download" className="text-blue-400 hover:text-blue-300">
            → Download the agent
          </a>
          <span className="mx-2 text-gray-700">·</span>
          <button
            onClick={() => navigate('/login')}
            className="text-blue-400 hover:text-blue-300"
          >
            Sign in to manage machines
          </button>
        </div>
      </div>
    </div>
  );
}
