import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../lib/auth';
import api, { recordingsApi, tenantSettingsApi, errMsg } from '../lib/api';
import type { Recording } from '../lib/api';
import { Monitor, LogOut, ArrowLeft, Film, Play, Trash2, X, Video, VideoOff } from 'lucide-react';

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function formatDuration(ms: number | null): string {
  if (!ms) return '—';
  const seconds = Math.floor(ms / 1000);
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export default function Recordings() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [loading, setLoading] = useState(true);
  const [playing, setPlaying] = useState<Recording | null>(null);
  const [playbackUrl, setPlaybackUrl] = useState<string | null>(null);
  const [recordingEnabled, setRecordingEnabled] = useState<boolean | null>(null);
  const [toggling, setToggling] = useState(false);

  const isOwner = user?.role === 'owner';
  const isAdmin = isOwner || user?.role === 'admin';

  const load = async () => {
    try {
      const [recs, settings] = await Promise.all([
        recordingsApi.list(),
        tenantSettingsApi.get(),
      ]);
      setRecordings(recs.data);
      setRecordingEnabled(settings.data.recording_enabled);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, []);

  // Revoke blob URL when component unmounts or URL changes (prevents leak)
  useEffect(() => {
    return () => {
      if (playbackUrl) URL.revokeObjectURL(playbackUrl);
    };
  }, [playbackUrl]);

  const handleDelete = async (r: Recording) => {
    if (!confirm(`Delete recording of "${r.machine_name}" from ${new Date(r.created_at).toLocaleString()}?`)) return;
    await recordingsApi.delete(r.id);
    load();
  };

  const handlePlay = async (r: Recording) => {
    // Revoke any previous blob URL before starting a new playback
    if (playbackUrl) URL.revokeObjectURL(playbackUrl);
    setPlaybackUrl(null);
    setPlaying(r);
    try {
      const resp = await api.get(`/recordings/${r.id}/playback`, { responseType: 'blob' });
      const url = URL.createObjectURL(resp.data as Blob);
      setPlaybackUrl(url);
    } catch (err: any) {
      alert(errMsg(err, 'Failed to load recording'));
      setPlaying(null);
    }
  };

  const closePlayer = () => {
    if (playbackUrl) URL.revokeObjectURL(playbackUrl);
    setPlaybackUrl(null);
    setPlaying(null);
  };

  const handleToggle = async () => {
    if (!isOwner || recordingEnabled === null) return;
    setToggling(true);
    try {
      await tenantSettingsApi.update({ recording_enabled: !recordingEnabled });
      setRecordingEnabled(!recordingEnabled);
    } catch (err: any) {
      alert(errMsg(err, 'Failed'));
    } finally {
      setToggling(false);
    }
  };

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
        <button onClick={() => navigate('/app')} className="flex items-center gap-1 text-gray-400 hover:text-white text-sm mb-4">
          <ArrowLeft className="w-4 h-4" /> Back to Machines
        </button>

        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-semibold text-white flex items-center gap-2">
            <Film className="w-5 h-5" /> Session Recordings
          </h2>
          {isAdmin && (
            <div className="flex items-center gap-3">
              <div className={`flex items-center gap-2 px-3 py-1.5 rounded text-sm border ${
                recordingEnabled ? 'bg-green-900/30 border-green-700 text-green-300' : 'bg-gray-800 border-gray-700 text-gray-400'
              }`}>
                {recordingEnabled ? <Video className="w-4 h-4" /> : <VideoOff className="w-4 h-4" />}
                Recording: {recordingEnabled === null ? '…' : recordingEnabled ? 'ON' : 'OFF'}
              </div>
              {isOwner && recordingEnabled !== null && (
                <button
                  onClick={handleToggle}
                  disabled={toggling}
                  className="px-3 py-1.5 bg-gray-800 hover:bg-gray-700 border border-gray-700 text-white rounded text-sm"
                >
                  {recordingEnabled ? 'Disable' : 'Enable'}
                </button>
              )}
            </div>
          )}
        </div>

        {loading ? (
          <p className="text-gray-500">Loading...</p>
        ) : recordings.length === 0 ? (
          <div className="bg-gray-900 border border-gray-800 rounded-lg p-12 text-center">
            <Film className="w-12 h-12 text-gray-600 mx-auto mb-4" />
            <p className="text-gray-400 mb-1">No recordings yet</p>
            <p className="text-sm text-gray-600">
              {recordingEnabled
                ? 'Recordings will appear here after a session ends.'
                : 'Enable recording (owner only) to save sessions for later playback.'}
            </p>
          </div>
        ) : (
          <div className="grid gap-2">
            {recordings.map((r) => (
              <div key={r.id} className="bg-gray-900 border border-gray-800 rounded p-4 flex items-center justify-between">
                <div className="flex items-center gap-3 min-w-0 flex-1">
                  <div className="p-2 bg-purple-900/30 rounded">
                    <Film className="w-5 h-5 text-purple-400" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-white font-medium truncate">{r.machine_name}</div>
                    <div className="text-xs text-gray-500">
                      {new Date(r.created_at).toLocaleString()} · {formatDuration(r.duration_ms)} · {formatSize(r.size_bytes)}
                      {r.started_by && <> · started by {r.started_by}</>}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => handlePlay(r)}
                    className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm"
                  >
                    <Play className="w-4 h-4" /> Play
                  </button>
                  {isAdmin && (
                    <button
                      onClick={() => handleDelete(r)}
                      className="p-1.5 text-gray-500 hover:text-red-400"
                      title="Delete"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </main>

      {playing && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4" onClick={closePlayer}>
          <div className="bg-gray-900 border border-gray-800 rounded-lg p-4 max-w-5xl w-full" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-3">
              <div>
                <div className="text-white font-medium">{playing.machine_name}</div>
                <div className="text-xs text-gray-500">{new Date(playing.created_at).toLocaleString()}</div>
              </div>
              <button onClick={closePlayer} className="text-gray-400 hover:text-white">
                <X className="w-5 h-5" />
              </button>
            </div>
            {playbackUrl ? (
              <video
                src={playbackUrl}
                controls
                autoPlay
                className="w-full bg-black rounded"
                style={{ maxHeight: '70vh' }}
              />
            ) : (
              <div className="w-full h-64 bg-black rounded flex items-center justify-center text-gray-500">
                Loading...
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
