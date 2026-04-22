import { Download as DownloadIcon, Monitor, Apple, Terminal } from 'lucide-react';

const API_BASE = import.meta.env.VITE_API_URL || '';

/**
 * Public /download — no auth required. These installers trigger the
 * code+PIN ad-hoc flow: install, share the on-screen code, and anyone can
 * connect via /connect.
 */
export default function Download() {
  const options = [
    {
      os: 'Windows',
      icon: Monitor,
      file: 'callmor-agent-public.exe',
      href: `${API_BASE}/downloads/agent/public/windows`,
      note: 'Windows 10 or later · 64-bit',
    },
    {
      os: 'macOS',
      icon: Apple,
      file: 'callmor-agent-public.pkg',
      href: `${API_BASE}/downloads/agent/public/macos`,
      note: 'Coming soon — use Windows or Linux for now',
      disabled: true,
    },
    {
      os: 'Linux',
      icon: Terminal,
      file: 'callmor-agent-public.deb',
      href: `${API_BASE}/downloads/agent/public/linux`,
      note: 'Debian / Ubuntu · 64-bit',
    },
  ];

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex items-center justify-center p-4">
      <div className="w-full max-w-2xl">
        <div className="flex items-center gap-3 mb-6">
          <DownloadIcon className="w-8 h-8 text-blue-400" />
          <h1 className="text-2xl font-semibold">Download the Agent</h1>
        </div>
        <p className="text-sm text-gray-400 mb-8">
          Install once on the computer you want to share. When the agent starts
          it will show an access code and PIN — share both with anyone who
          needs to connect from <a href="/connect" className="text-blue-400 hover:underline">remote.callmor.ai/connect</a>.
          No account needed.
        </p>

        <div className="grid gap-3">
          {options.map(({ os, icon: Icon, file, href, note, disabled }) => (
            <a
              key={os}
              href={disabled ? undefined : href}
              className={`flex items-center gap-4 p-4 bg-gray-900 border border-gray-800 rounded hover:border-gray-700 transition ${
                disabled ? 'opacity-50 cursor-not-allowed' : ''
              }`}
              onClick={(e) => {
                if (disabled) e.preventDefault();
              }}
            >
              <Icon className="w-8 h-8 text-gray-400" />
              <div className="flex-1">
                <div className="text-lg font-medium text-white">
                  {os}
                  {disabled && <span className="ml-2 text-xs text-gray-500">(coming soon)</span>}
                </div>
                <div className="text-sm text-gray-500">{note}</div>
                <div className="text-xs text-gray-600 mt-1 font-mono">{file}</div>
              </div>
              {!disabled && <DownloadIcon className="w-5 h-5 text-gray-500" />}
            </a>
          ))}
        </div>

        <div className="mt-10 pt-6 border-t border-gray-800 text-sm text-gray-500">
          <p className="mb-2">Got an access code from a colleague?</p>
          <a href="/connect" className="text-blue-400 hover:text-blue-300">
            → Connect to a computer
          </a>
          <span className="mx-2 text-gray-700">·</span>
          <a href="/login" className="text-blue-400 hover:text-blue-300">
            Sign in to manage machines
          </a>
        </div>
      </div>
    </div>
  );
}
