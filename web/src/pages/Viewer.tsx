import { useEffect, useRef, useState, useCallback } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { Maximize, LogOut, Terminal, ArrowLeft, Wifi, WifiOff, Loader2 } from 'lucide-react';

type ConnState = 'idle' | 'connecting' | 'negotiating' | 'streaming' | 'failed' | 'ended';
type LogCls = 'sys' | 'err' | 'ok' | 'ice';

interface LogEntry {
  ts: string;
  text: string;
  cls: LogCls;
}

interface IceServer {
  urls: string | string[];
  username?: string;
  credential?: string;
}

/**
 * Full-screen remote desktop viewer. Reads connection params from the
 * route + query string, fetches TURN credentials, opens a WebSocket to
 * the relay, and negotiates WebRTC with the agent. Pure port of the
 * behavior in public/viewer-test.html — no control inputs, everything
 * comes from the URL.
 */
export default function Viewer() {
  const { machineId: machineIdParam } = useParams<{ machineId: string }>();
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();

  const relayUrl = searchParams.get('relay') || '';
  const sessionToken = searchParams.get('token') || '';
  const permission = (searchParams.get('permission') || 'full_control') as
    | 'full_control'
    | 'view_only';
  const sessionId = searchParams.get('session') || null;
  const hostname = searchParams.get('hostname') || '';
  const machineId = machineIdParam || searchParams.get('machine') || '';

  const [connState, setConnState] = useState<ConnState>('idle');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [needsClickToPlay, setNeedsClickToPlay] = useState(false);

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const pcRef = useRef<RTCPeerConnection | null>(null);
  const inputChannelRef = useRef<RTCDataChannel | null>(null);
  const readyIntervalRef = useRef<number | null>(null);
  const statsIntervalRef = useRef<number | null>(null);
  const inputAbortRef = useRef<AbortController | null>(null);
  const remoteSizeRef = useRef({ width: 1920, height: 1080 });
  const lastStatsRef = useRef({ bytes: 0, frames: 0 });
  const iceServersRef = useRef<IceServer[]>([
    { urls: 'stun:stun.l.google.com:19302' },
  ]);
  const logsEndRef = useRef<HTMLDivElement | null>(null);

  const log = useCallback((text: string, cls: LogCls = 'sys') => {
    const ts = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev, { ts, text, cls }]);
  }, []);

  useEffect(() => {
    if (showLogs) {
      logsEndRef.current?.scrollIntoView({ block: 'end' });
    }
  }, [logs, showLogs]);

  // --- Helpers ---

  const sendSignal = useCallback((payload: unknown) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify({ type: 'relay', payload }));
  }, []);

  const sendInput = useCallback((event: unknown) => {
    const ch = inputChannelRef.current;
    if (ch && ch.readyState === 'open') {
      ch.send(JSON.stringify(event));
    }
  }, []);

  const videoToRemote = useCallback((clientX: number, clientY: number) => {
    const video = videoRef.current;
    if (!video) return { x: 0, y: 0 };
    const rect = video.getBoundingClientRect();
    const relX = (clientX - rect.left) / rect.width;
    const relY = (clientY - rect.top) / rect.height;
    return {
      x: Math.round(relX * remoteSizeRef.current.width),
      y: Math.round(relY * remoteSizeRef.current.height),
    };
  }, []);

  const setupInputCapture = useCallback(() => {
    if (inputAbortRef.current) inputAbortRef.current.abort();
    inputAbortRef.current = new AbortController();
    const signal = inputAbortRef.current.signal;
    const video = videoRef.current;
    if (!video) return;

    video.addEventListener(
      'mousemove',
      (e) => {
        const { x, y } = videoToRemote(e.clientX, e.clientY);
        sendInput({ type: 'mousemove', x, y });
      },
      { signal },
    );
    video.addEventListener(
      'mousedown',
      (e) => {
        e.preventDefault();
        video.focus();
        const { x, y } = videoToRemote(e.clientX, e.clientY);
        sendInput({ type: 'mousedown', x, y, button: e.button });
      },
      { signal },
    );
    video.addEventListener(
      'mouseup',
      (e) => {
        e.preventDefault();
        const { x, y } = videoToRemote(e.clientX, e.clientY);
        sendInput({ type: 'mouseup', x, y, button: e.button });
      },
      { signal },
    );
    video.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        const { x, y } = videoToRemote(e.clientX, e.clientY);
        sendInput({
          type: 'scroll',
          x,
          y,
          deltaX: e.deltaX,
          deltaY: e.deltaY,
        });
      },
      { passive: false, signal },
    );
    video.addEventListener('contextmenu', (e) => e.preventDefault(), { signal });
    video.addEventListener(
      'keydown',
      (e) => {
        e.preventDefault();
        sendInput({ type: 'keydown', code: e.code, key: e.key });
      },
      { signal },
    );
    video.addEventListener(
      'keyup',
      (e) => {
        e.preventDefault();
        sendInput({ type: 'keyup', code: e.code, key: e.key });
      },
      { signal },
    );

    log('Input capture attached to video element', 'ok');
  }, [log, sendInput, videoToRemote]);

  // --- TURN credentials ---
  const fetchIceServers = useCallback(async () => {
    try {
      const m = relayUrl.match(/^wss?:\/\/([^/]+)/);
      const host = m ? m[1].replace(/^relay\./, 'api.') : 'api.callmor.ai';
      const proto =
        relayUrl.startsWith('ws://') || window.location.protocol === 'http:'
          ? 'http'
          : 'https';
      const resp = await fetch(`${proto}://${host}/turn`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data = await resp.json();
      if (Array.isArray(data.ice_servers) && data.ice_servers.length > 0) {
        iceServersRef.current = data.ice_servers.map((s: IceServer) => ({
          urls: s.urls,
          ...(s.username ? { username: s.username, credential: s.credential } : {}),
        }));
        log(`Loaded ${iceServersRef.current.length} ICE servers (including TURN)`, 'ok');
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      log(`TURN fetch failed (${msg}); using public STUN only`, 'err');
    }
  }, [log, relayUrl]);

  // --- Cleanup ---
  const cleanup = useCallback(() => {
    if (readyIntervalRef.current) {
      window.clearInterval(readyIntervalRef.current);
      readyIntervalRef.current = null;
    }
    if (statsIntervalRef.current) {
      window.clearInterval(statsIntervalRef.current);
      statsIntervalRef.current = null;
    }
    lastStatsRef.current = { bytes: 0, frames: 0 };
    if (pcRef.current) {
      try { pcRef.current.close(); } catch { /* noop */ }
      pcRef.current = null;
    }
    const ws = wsRef.current;
    if (ws) {
      try {
        ws.onopen = null;
        ws.onmessage = null;
        ws.onclose = null;
        ws.onerror = null;
      } catch { /* noop */ }
      try { ws.close(); } catch { /* noop */ }
      wsRef.current = null;
    }
    if (inputAbortRef.current) {
      inputAbortRef.current.abort();
      inputAbortRef.current = null;
    }
    inputChannelRef.current = null;
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
    setNeedsClickToPlay(false);
  }, []);

  const disconnect = useCallback(() => {
    const ws = wsRef.current;
    if (ws) {
      try { ws.close(); } catch { /* noop */ }
    }
    cleanup();
    setConnState('ended');
    log('Disconnected', 'sys');
  }, [cleanup, log]);

  // --- PeerConnection setup ---
  const createPeerConnection = useCallback(() => {
    const pc = new RTCPeerConnection({ iceServers: iceServersRef.current });
    pcRef.current = pc;

    pc.ondatachannel = (e) => {
      const channel = e.channel;
      log(`Received data channel: "${channel.label}"`, 'ok');
      if (channel.label === 'input') {
        inputChannelRef.current = channel;
        channel.onopen = () => {
          log('Input data channel open', 'ok');
          channel.send(JSON.stringify({ type: 'permission', value: permission }));
          log(`Permission: ${permission}`, 'ok');
          if (permission === 'full_control') {
            setupInputCapture();
          }
        };
        channel.onmessage = (ev) => {
          try {
            const msg = JSON.parse(ev.data);
            if (msg.type === 'screen-size') {
              remoteSizeRef.current = { width: msg.width, height: msg.height };
              log(`Remote screen: ${msg.width}x${msg.height}`, 'ok');
            }
          } catch { /* noop */ }
        };
        channel.onclose = () => log('Input data channel closed');
      }
    };

    pc.onicecandidate = (e) => {
      if (e.candidate) {
        sendSignal({
          signal: 'ice-candidate',
          candidate: {
            candidate: e.candidate.candidate,
            sdpMLineIndex: e.candidate.sdpMLineIndex,
            sdpMid: e.candidate.sdpMid,
          },
        });
      }
    };

    pc.oniceconnectionstatechange = () => {
      log(`ICE: ${pc.iceConnectionState}`, 'ice');
      if (
        pc.iceConnectionState === 'connected' ||
        pc.iceConnectionState === 'completed'
      ) {
        setConnState('streaming');
      } else if (pc.iceConnectionState === 'failed') {
        log('ICE connection failed', 'err');
        setConnState('failed');
      }
    };

    pc.ontrack = (e) => {
      log(`Received ${e.track.kind} track`, 'ok');
      const video = videoRef.current;
      if (!video) return;
      video.srcObject = e.streams[0] || new MediaStream([e.track]);

      video.addEventListener(
        'loadedmetadata',
        () => log(`Video metadata loaded: ${video.videoWidth}x${video.videoHeight}`, 'ok'),
        { once: true },
      );
      video.addEventListener('playing', () => log('Video element playing', 'ok'), { once: true });
      video.addEventListener('error', () =>
        log(`Video error: ${video.error?.message || 'unknown'}`, 'err'),
      );

      video
        .play()
        .then(() => {
          log('video.play() resolved', 'ok');
          setNeedsClickToPlay(false);
        })
        .catch((err: Error) => {
          log(`video.play() rejected: ${err.name}: ${err.message}. Click to unblock.`, 'err');
          setNeedsClickToPlay(true);
        });

      statsIntervalRef.current = window.setInterval(async () => {
        if (!pcRef.current) return;
        try {
          const stats = await pcRef.current.getStats(e.track);
          stats.forEach((report: any) => {
            if (report.type === 'inbound-rtp' && report.kind === 'video') {
              const dBytes = report.bytesReceived - lastStatsRef.current.bytes;
              const dFrames =
                (report.framesDecoded || 0) - lastStatsRef.current.frames;
              lastStatsRef.current = {
                bytes: report.bytesReceived,
                frames: report.framesDecoded || 0,
              };
              log(
                `RX: +${dBytes} bytes, +${dFrames} frames (total ${report.framesDecoded || 0} frames, ${report.bytesReceived} bytes, ${report.packetsLost || 0} lost)`,
              );
            }
          });
        } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          log(`stats err: ${msg}`, 'err');
        }
      }, 3000);

      video.focus();
    };
  }, [log, permission, sendSignal, setupInputCapture]);

  const handleSignal = useCallback(
    async (payload: any) => {
      if (!payload?.signal) return;

      if (payload.signal === 'offer') {
        log(`Received SDP offer from agent (${payload.sdp.length} bytes)`, 'ok');
        setConnState('negotiating');
        if (readyIntervalRef.current) {
          window.clearInterval(readyIntervalRef.current);
          readyIntervalRef.current = null;
        }
        createPeerConnection();
        const pc = pcRef.current;
        if (!pc) return;
        await pc.setRemoteDescription({ type: 'offer', sdp: payload.sdp });
        const answer = await pc.createAnswer();
        await pc.setLocalDescription(answer);
        log(`Sending SDP answer (${answer.sdp?.length || 0} bytes)`);
        sendSignal({ signal: 'answer', sdp: answer.sdp });
      } else if (payload.signal === 'ice-candidate') {
        const pc = pcRef.current;
        if (pc && payload.candidate) {
          try {
            await pc.addIceCandidate({
              candidate: payload.candidate.candidate,
              sdpMLineIndex: payload.candidate.sdpMLineIndex,
              sdpMid: payload.candidate.sdpMid,
            });
          } catch (err: unknown) {
            const msg = err instanceof Error ? err.message : String(err);
            log(`addIceCandidate failed: ${msg}`, 'err');
          }
        }
      }
    },
    [createPeerConnection, log, sendSignal],
  );

  const connect = useCallback(async () => {
    if (!relayUrl || !machineId) {
      log('Missing relay URL or machine id in URL', 'err');
      setConnState('failed');
      return;
    }
    setConnState('connecting');
    log('Fetching TURN credentials...');
    await fetchIceServers();
    log(`Connecting to relay ${relayUrl}...`);

    const ws = new WebSocket(relayUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      const hello: Record<string, unknown> = {
        type: 'hello',
        role: 'browser',
        machine_id: machineId,
      };
      if (sessionToken) hello.token = sessionToken;
      ws.send(JSON.stringify(hello));
      log(`Connected to relay as browser for "${machineId}"`, 'ok');

      const sendReady = () => sendSignal({ signal: 'ready', session_id: sessionId });
      window.setTimeout(() => {
        sendReady();
        log('Sent "ready" to agent, waiting for offer...', 'ok');
      }, 300);

      readyIntervalRef.current = window.setInterval(() => {
        const pc = pcRef.current;
        if (pc && pc.signalingState === 'stable' && pc.remoteDescription) return;
        log('Still waiting for agent — resending ready', 'sys');
        sendReady();
      }, 3000);
    };

    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data);
        if (msg.type === 'relay' && msg.payload) {
          void handleSignal(msg.payload);
        } else if (msg.type === 'error') {
          log(`Relay error: ${msg.message}`, 'err');
        }
      } catch (err: unknown) {
        const m = err instanceof Error ? err.message : String(err);
        log(`bad relay frame: ${m}`, 'err');
      }
    };

    ws.onclose = () => {
      log('Relay disconnected');
      cleanup();
      setConnState((prev) => (prev === 'ended' ? prev : 'ended'));
    };
    ws.onerror = () => {
      log('Relay error', 'err');
      cleanup();
      setConnState('failed');
    };
  }, [
    cleanup,
    fetchIceServers,
    handleSignal,
    log,
    machineId,
    relayUrl,
    sendSignal,
    sessionId,
    sessionToken,
  ]);

  // Auto-connect on mount; clean up on unmount.
  useEffect(() => {
    void connect();
    return () => cleanup();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const requestFullscreen = () => {
    const el = videoRef.current;
    if (!el) return;
    if (!document.fullscreenElement) {
      el.requestFullscreen?.().catch(() => { /* noop */ });
    } else {
      document.exitFullscreen?.().catch(() => { /* noop */ });
    }
  };

  const unblockAutoplay = () => {
    videoRef.current?.play().then(() => setNeedsClickToPlay(false)).catch(() => { /* noop */ });
  };

  const stateLabel: Record<ConnState, string> = {
    idle: 'Idle',
    connecting: 'Connecting',
    negotiating: 'Negotiating',
    streaming: 'Connected',
    failed: 'Failed',
    ended: 'Disconnected',
  };

  const stateColor: Record<ConnState, string> = {
    idle: 'bg-gray-700 text-gray-300',
    connecting: 'bg-amber-900/60 text-amber-300',
    negotiating: 'bg-amber-900/60 text-amber-300',
    streaming: 'bg-green-900/60 text-green-300',
    failed: 'bg-red-900/60 text-red-300',
    ended: 'bg-gray-800 text-gray-400',
  };

  const sessionEnded = connState === 'ended' || connState === 'failed';

  return (
    <div className="fixed inset-0 flex flex-col bg-black text-gray-100 overflow-hidden">
      {/* Toolbar */}
      <header className="shrink-0 h-11 bg-gray-950 border-b border-gray-800 px-3 flex items-center gap-3">
        <button
          onClick={() => navigate('/connect')}
          className="text-gray-400 hover:text-white p-1"
          title="Back"
        >
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="flex items-center gap-2 min-w-0">
          {connState === 'streaming' ? (
            <Wifi className="w-4 h-4 text-green-400 shrink-0" />
          ) : sessionEnded ? (
            <WifiOff className="w-4 h-4 text-gray-500 shrink-0" />
          ) : (
            <Loader2 className="w-4 h-4 text-amber-400 shrink-0 animate-spin" />
          )}
          <span className="text-sm font-medium text-white truncate">
            {hostname || machineId || 'Remote'}
          </span>
          {permission === 'view_only' && (
            <span className="text-[10px] uppercase tracking-wider bg-amber-500/20 text-amber-300 px-1.5 py-0.5 rounded shrink-0">
              View only
            </span>
          )}
        </div>
        <span className={`ml-1 text-xs px-2 py-0.5 rounded ${stateColor[connState]}`}>
          {stateLabel[connState]}
        </span>
        <div className="flex-1" />
        <button
          onClick={() => setShowLogs((v) => !v)}
          className={`inline-flex items-center gap-1 px-2 py-1 text-xs rounded border ${
            showLogs
              ? 'bg-gray-800 border-gray-700 text-white'
              : 'bg-transparent border-gray-800 text-gray-400 hover:text-white'
          }`}
          title="Toggle debug logs"
        >
          <Terminal className="w-3.5 h-3.5" /> Logs
        </button>
        <button
          onClick={requestFullscreen}
          className="inline-flex items-center gap-1 px-2 py-1 text-xs rounded border border-gray-800 text-gray-300 hover:text-white"
          title="Fullscreen"
        >
          <Maximize className="w-3.5 h-3.5" /> Fullscreen
        </button>
        <button
          onClick={disconnect}
          disabled={sessionEnded}
          className="inline-flex items-center gap-1 px-2 py-1 text-xs rounded bg-red-600 hover:bg-red-700 disabled:opacity-40 text-white"
          title="End session"
        >
          <LogOut className="w-3.5 h-3.5" /> Disconnect
        </button>
      </header>

      {/* Video + logs */}
      <div className="flex-1 flex min-h-0">
        <div className="flex-1 relative flex items-center justify-center bg-black min-h-0">
          <video
            ref={videoRef}
            autoPlay
            playsInline
            tabIndex={0}
            className="max-w-full max-h-full bg-black outline-none"
          />

          {!sessionEnded && connState !== 'streaming' && !needsClickToPlay && (
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="flex items-center gap-3 px-4 py-2 bg-gray-900/80 border border-gray-800 rounded">
                <Loader2 className="w-4 h-4 animate-spin text-blue-400" />
                <span className="text-sm text-gray-200">
                  {connState === 'connecting'
                    ? 'Connecting to relay…'
                    : connState === 'negotiating'
                    ? 'Negotiating session…'
                    : 'Waiting for agent…'}
                </span>
              </div>
            </div>
          )}

          {needsClickToPlay && (
            <button
              onClick={unblockAutoplay}
              className="absolute inset-0 flex items-center justify-center bg-black/70"
            >
              <div className="px-5 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded font-medium">
                Click to start video
              </div>
            </button>
          )}

          {sessionEnded && (
            <div className="absolute inset-0 flex items-center justify-center bg-black/85 p-4">
              <div className="bg-gray-900 border border-gray-800 rounded-lg p-6 max-w-sm w-full text-center">
                <div className="w-10 h-10 mx-auto mb-3 rounded-full bg-gray-800 flex items-center justify-center">
                  <WifiOff className="w-5 h-5 text-gray-400" />
                </div>
                <h3 className="text-lg font-semibold text-white mb-1">
                  {connState === 'failed' ? 'Session failed' : 'Session ended'}
                </h3>
                <p className="text-sm text-gray-400 mb-4">
                  {connState === 'failed'
                    ? 'The connection could not be established. Check the code and try again.'
                    : 'The remote session is closed.'}
                </p>
                <button
                  onClick={() => navigate('/connect')}
                  className="w-full py-2 bg-blue-600 hover:bg-blue-700 text-white rounded font-medium"
                >
                  Connect again
                </button>
              </div>
            </div>
          )}
        </div>

        {showLogs && (
          <aside className="w-96 max-w-[40vw] shrink-0 border-l border-gray-800 bg-gray-950/90 flex flex-col">
            <div className="px-3 py-2 border-b border-gray-800 flex items-center justify-between">
              <span className="text-xs uppercase tracking-wider text-gray-400">Logs</span>
              <button
                onClick={() => setLogs([])}
                className="text-xs text-gray-500 hover:text-white"
              >
                Clear
              </button>
            </div>
            <div className="flex-1 overflow-y-auto font-mono text-[11px] leading-relaxed p-2">
              {logs.map((l, i) => (
                <div key={i} className={classForCls(l.cls)}>
                  <span className="text-gray-600">[{l.ts}]</span> {l.text}
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          </aside>
        )}
      </div>
    </div>
  );
}

function classForCls(c: LogCls): string {
  switch (c) {
    case 'err':
      return 'text-red-400';
    case 'ok':
      return 'text-green-400';
    case 'ice':
      return 'text-purple-300';
    default:
      return 'text-yellow-300';
  }
}
