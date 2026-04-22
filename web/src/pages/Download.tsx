import { Link } from 'react-router-dom';
import {
  Download as DownloadIcon,
  Monitor,
  Apple,
  Terminal,
  Shield,
  Zap,
  Server,
  ArrowRight,
  Check,
} from 'lucide-react';

const API_BASE = import.meta.env.VITE_API_URL || '';

type OsOption = {
  os: string;
  icon: typeof Monitor;
  file: string;
  href: string;
  note: string;
  disabled?: boolean;
};

const quickConnectOptions: OsOption[] = [
  {
    os: 'Windows',
    icon: Monitor,
    file: 'callmor.exe',
    href: `${API_BASE}/downloads/agent/public/windows`,
    note: 'Windows 10 or later · 64-bit · runs instantly, no install',
  },
  {
    os: 'Linux',
    icon: Terminal,
    file: 'callmor-agent.deb',
    href: `${API_BASE}/downloads/agent/public/linux`,
    note: 'Debian / Ubuntu · 64-bit',
  },
  {
    os: 'macOS',
    icon: Apple,
    file: 'callmor-agent.pkg',
    href: `${API_BASE}/downloads/agent/public/macos`,
    note: 'Coming soon — use Windows or Linux for now',
    disabled: true,
  },
];

const rustdeskOptions: OsOption[] = [
  {
    os: 'Windows',
    icon: Monitor,
    file: 'callmor-rd.exe',
    href: `${API_BASE}/downloads/rustdesk/windows`,
    note: 'Windows 10 or later · 64-bit · pre-configured installer',
  },
  {
    os: 'Linux',
    icon: Terminal,
    file: 'callmor-rd.deb',
    href: `${API_BASE}/downloads/rustdesk/linux`,
    note: 'Coming soon — Debian / Ubuntu build in progress',
    disabled: true,
  },
  {
    os: 'macOS',
    icon: Apple,
    file: 'callmor-rd.dmg',
    href: `${API_BASE}/downloads/rustdesk/macos`,
    note: 'Coming soon — macOS build in progress',
    disabled: true,
  },
];

export default function Download() {
  return (
    <div className="px-4 py-12 min-h-[calc(100vh-56px)]">
      <div className="max-w-6xl mx-auto">
        <div className="text-center mb-10">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-gray-800 bg-gray-900/60 text-xs text-gray-400 mb-4">
            <DownloadIcon className="w-3.5 h-3.5" />
            Downloads
          </div>
          <h1 className="text-3xl md:text-4xl font-bold text-white">
            Choose how you want to connect
          </h1>
          <p className="mt-3 text-gray-400 max-w-2xl mx-auto">
            Callmor Remote Desktop offers two paths. Quick Connect for ad-hoc
            support with zero setup. RustDesk Mode for persistent access to
            machines you manage long-term.
          </p>
        </div>

        <div className="mb-8 p-4 bg-amber-950/40 border border-amber-900 rounded flex gap-3">
          <Shield className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" />
          <div className="text-sm text-amber-100">
            <div className="font-medium mb-1">Windows may warn you the first time</div>
            <div className="text-amber-200/80">
              Our installers aren't code-signed yet, so SmartScreen may say "Windows protected your PC."
              Click <span className="font-semibold">More info</span> → <span className="font-semibold">Run anyway</span>.
              Defender may also hold the file — if so, open Protection history and choose <span className="font-semibold">Allow on device</span>.
            </div>
          </div>
        </div>

        {/* Two mode cards */}
        <div className="grid md:grid-cols-2 gap-6">
          {/* Quick Connect */}
          <div className="bg-gray-900 border border-gray-800 rounded-xl p-6 flex flex-col">
            <div className="flex items-center gap-3 mb-2">
              <div className="w-10 h-10 rounded-md bg-blue-500/10 border border-blue-500/20 flex items-center justify-center">
                <Zap className="w-5 h-5 text-blue-400" />
              </div>
              <div>
                <h2 className="text-xl font-semibold text-white">Quick Connect</h2>
                <div className="text-xs text-gray-500">Works immediately. No setup.</div>
              </div>
            </div>
            <p className="text-sm text-gray-400 mt-3">
              The person running this computer clicks a single file, sees a
              code and PIN, and shares them. Anyone at{' '}
              <Link to="/connect" className="text-blue-400 hover:text-blue-300">
                remote.callmor.ai/connect
              </Link>{' '}
              enters them and connects in their browser.
            </p>

            <ul className="mt-4 space-y-2 text-sm text-gray-300">
              <BulletPoint>One-click <span className="font-mono text-xs">.exe</span> — nothing to install</BulletPoint>
              <BulletPoint>Access code + PIN shown on screen</BulletPoint>
              <BulletPoint>Browser-based viewer — no install needed on the controller</BulletPoint>
              <BulletPoint>Works on any network, through any firewall</BulletPoint>
            </ul>

            <div className="mt-6 pt-5 border-t border-gray-800 space-y-2.5">
              {quickConnectOptions.map((o) => (
                <OsDownloadRow key={o.os} {...o} />
              ))}
            </div>
          </div>

          {/* RustDesk Mode */}
          <div className="bg-gray-900 border border-gray-800 rounded-xl p-6 flex flex-col">
            <div className="flex items-center gap-3 mb-2">
              <div className="w-10 h-10 rounded-md bg-purple-500/10 border border-purple-500/20 flex items-center justify-center">
                <Server className="w-5 h-5 text-purple-400" />
              </div>
              <div>
                <h2 className="text-xl font-semibold text-white">RustDesk Mode</h2>
                <div className="text-xs text-gray-500">Full-featured. Persistent machines.</div>
              </div>
            </div>
            <p className="text-sm text-gray-400 mt-3">
              A production-grade remote access client running on our self-hosted
              Callmor rendezvous server. Install once on each computer you want
              to manage and keep the connection forever.
            </p>

            <ul className="mt-4 space-y-2 text-sm text-gray-300">
              <BulletPoint>Full desktop streaming (H.264 / VP9)</BulletPoint>
              <BulletPoint>File transfer, two-factor auth, session recording</BulletPoint>
              <BulletPoint>Persistent across reboots</BulletPoint>
              <BulletPoint>
                <span className="font-semibold text-purple-300">Requires router port forwarding</span>{' '}
                — see setup guide
              </BulletPoint>
            </ul>

            <div className="mt-6 pt-5 border-t border-gray-800 space-y-2.5">
              {rustdeskOptions.map((o) => (
                <OsDownloadRow key={o.os} {...o} />
              ))}
            </div>

            <Link
              to="/rustdesk-setup"
              className="mt-5 inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-purple-600 hover:bg-purple-700 text-white rounded-md text-sm font-medium"
            >
              Setup guide <ArrowRight className="w-4 h-4" />
            </Link>
          </div>
        </div>

        {/* Comparison table */}
        <div className="mt-12 bg-gray-900/50 border border-gray-800 rounded-xl p-6">
          <h3 className="text-lg font-semibold text-white mb-4">Which should I use?</h3>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-gray-800 text-gray-400">
                  <th className="text-left py-3 pr-4 font-medium"></th>
                  <th className="text-left py-3 px-4 font-medium">
                    <span className="inline-flex items-center gap-1.5 text-blue-300">
                      <Zap className="w-4 h-4" /> Quick Connect
                    </span>
                  </th>
                  <th className="text-left py-3 px-4 font-medium">
                    <span className="inline-flex items-center gap-1.5 text-purple-300">
                      <Server className="w-4 h-4" /> RustDesk Mode
                    </span>
                  </th>
                </tr>
              </thead>
              <tbody className="text-gray-300">
                <ComparisonRow label="Setup time" a="Seconds" b="5 min + router config" />
                <ComparisonRow label="Viewer needs" a="Any browser" b="Callmor RustDesk client" />
                <ComparisonRow label="Persistence" a="Session-based" b="Permanent" />
                <ComparisonRow label="Network config" a="None" b="Port forwarding" />
                <ComparisonRow
                  label="Best for"
                  a="Ad-hoc support, drive-by help"
                  b="Your own fleet of machines"
                  last
                />
              </tbody>
            </table>
          </div>
        </div>

        <div className="mt-10 text-center text-sm text-gray-500">
          Got an access code from a colleague?{' '}
          <Link to="/connect" className="text-blue-400 hover:text-blue-300">
            Connect to a computer →
          </Link>
          <span className="mx-3 text-gray-700">·</span>
          <Link to="/login" className="text-blue-400 hover:text-blue-300">
            Sign in to manage machines
          </Link>
        </div>
      </div>
    </div>
  );
}

function BulletPoint({ children }: { children: React.ReactNode }) {
  return (
    <li className="flex items-start gap-2">
      <Check className="w-4 h-4 text-green-400 flex-shrink-0 mt-0.5" />
      <span>{children}</span>
    </li>
  );
}

function OsDownloadRow({ os, icon: Icon, file, href, note, disabled }: OsOption) {
  return (
    <a
      href={disabled ? undefined : href}
      className={`flex items-center gap-3 p-3 bg-gray-950/60 border border-gray-800 rounded hover:border-gray-700 transition ${
        disabled ? 'opacity-50 cursor-not-allowed' : ''
      }`}
      onClick={(e) => {
        if (disabled) e.preventDefault();
      }}
    >
      <Icon className="w-6 h-6 text-gray-400 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-white">
          {os}
          {disabled && <span className="ml-2 text-xs text-gray-500">(coming soon)</span>}
        </div>
        <div className="text-xs text-gray-500 truncate">{note}</div>
        <div className="text-[11px] text-gray-600 font-mono truncate">{file}</div>
      </div>
      {!disabled && <DownloadIcon className="w-4 h-4 text-gray-500 flex-shrink-0" />}
    </a>
  );
}

function ComparisonRow({
  label,
  a,
  b,
  last,
}: {
  label: string;
  a: string;
  b: string;
  last?: boolean;
}) {
  return (
    <tr className={last ? '' : 'border-b border-gray-800/60'}>
      <td className="py-3 pr-4 font-medium text-gray-400">{label}</td>
      <td className="py-3 px-4">{a}</td>
      <td className="py-3 px-4">{b}</td>
    </tr>
  );
}
