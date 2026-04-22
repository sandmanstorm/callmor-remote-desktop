import { Link } from 'react-router-dom';
import {
  Download as DownloadIcon,
  Server,
  Router,
  KeyRound,
  PlayCircle,
  Info,
  Apple,
  Terminal,
  ArrowRight,
} from 'lucide-react';

const API_BASE = import.meta.env.VITE_API_URL || '';

export default function RustdeskSetup() {
  return (
    <div className="px-4 py-12">
      <div className="max-w-3xl mx-auto">
        {/* Header */}
        <div className="mb-10">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-purple-900/50 bg-purple-950/40 text-xs text-purple-300 mb-4">
            <Server className="w-3.5 h-3.5" />
            RustDesk Mode Setup
          </div>
          <h1 className="text-3xl md:text-4xl font-bold text-white tracking-tight">
            Set up persistent remote access
          </h1>
          <p className="mt-4 text-gray-400 text-lg leading-relaxed">
            Install the Callmor client on every computer you want to reach,
            forward a handful of ports once, and every machine stays available
            on your private 9-digit ID forever.
          </p>
        </div>

        {/* Step 1 */}
        <Step
          n={1}
          icon={<DownloadIcon className="w-5 h-5 text-purple-400" />}
          title="Download the installer"
          body={
            <>
              <p className="text-gray-400 mb-4">
                The Windows build is pre-configured to use the Callmor
                rendezvous server — no manual server entry required.
              </p>
              <a
                href={`${API_BASE}/downloads/rustdesk/windows/branded`}
                className="inline-flex items-center gap-2 px-5 py-3 bg-purple-600 hover:bg-purple-700 text-white rounded-md text-sm font-medium"
              >
                <DownloadIcon className="w-4 h-4" />
                Download Callmor-RustDesk (Windows)
              </a>
              <div className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-gray-500">
                <a
                  href={`${API_BASE}/downloads/rustdesk/macos/official`}
                  className="inline-flex items-center gap-1.5 hover:text-gray-300"
                >
                  <Apple className="w-3.5 h-3.5" /> macOS (official build)
                </a>
                <a
                  href={`${API_BASE}/downloads/rustdesk/linux/official`}
                  className="inline-flex items-center gap-1.5 hover:text-gray-300"
                >
                  <Terminal className="w-3.5 h-3.5" /> Linux (official build)
                </a>
              </div>
              <p className="mt-3 text-xs text-gray-600">
                Official macOS and Linux builds require you to point them at
                our server on first launch. The Windows branded build does this
                for you.
              </p>
            </>
          }
        />

        {/* Step 2 */}
        <Step
          n={2}
          icon={<PlayCircle className="w-5 h-5 text-purple-400" />}
          title="Run the installer"
          body={
            <p className="text-gray-400">
              Double-click the file and accept the defaults. The Callmor client
              installs silently, auto-configures itself against our rendezvous
              server, and starts in the system tray. Nothing else to configure.
            </p>
          }
        />

        {/* Step 3 */}
        <Step
          n={3}
          icon={<KeyRound className="w-5 h-5 text-purple-400" />}
          title="Note your ID"
          body={
            <p className="text-gray-400">
              The main window shows your 9-digit Callmor ID (for example{' '}
              <span className="font-mono text-gray-200">123 456 789</span>) and a
              one-time password. The ID is permanent — write it down or save it
              in a password manager. The password rotates on each session unless
              you set a permanent one in settings.
            </p>
          }
        />

        {/* Step 4 */}
        <Step
          n={4}
          icon={<Router className="w-5 h-5 text-purple-400" />}
          title="Router setup (one time, for the host network)"
          body={
            <>
              <p className="text-gray-400 mb-4">
                For your network to accept incoming Callmor connections, your
                router needs to forward the ports below to the Callmor server
                at <span className="font-mono text-gray-200">10.10.100.34</span>.
                This is a one-time configuration done by whoever manages the
                host network.
              </p>

              <div className="overflow-x-auto">
                <table className="w-full text-sm bg-gray-950/60 border border-gray-800 rounded">
                  <thead>
                    <tr className="text-left text-gray-400 border-b border-gray-800">
                      <th className="py-2.5 px-4 font-medium">Port</th>
                      <th className="py-2.5 px-4 font-medium">Protocol</th>
                      <th className="py-2.5 px-4 font-medium">Purpose</th>
                    </tr>
                  </thead>
                  <tbody className="text-gray-300 font-mono text-sm">
                    <PortRow port="21115" proto="TCP" purpose="NAT test" />
                    <PortRow port="21116" proto="TCP" purpose="Signal" />
                    <PortRow port="21116" proto="UDP" purpose="Signal" />
                    <PortRow port="21117" proto="TCP" purpose="Relay" />
                    <PortRow port="21118" proto="TCP" purpose="WebSocket signal" />
                    <PortRow port="21119" proto="TCP" purpose="WebSocket relay" last />
                  </tbody>
                </table>
              </div>

              <div className="mt-5 p-4 bg-blue-950/40 border border-blue-900 rounded flex gap-3">
                <Info className="w-5 h-5 text-blue-400 flex-shrink-0 mt-0.5" />
                <div className="text-sm text-blue-100">
                  <div className="font-medium mb-1">
                    Not on the host network?
                  </div>
                  <div className="text-blue-200/80">
                    If you aren't on the network hosting Callmor, your network
                    admin must forward these ports or we cannot reach your
                    machine. Share this page with them — everything they need
                    is in the table above.
                  </div>
                </div>
              </div>
            </>
          }
        />

        {/* Step 5 */}
        <Step
          n={5}
          icon={<ArrowRight className="w-5 h-5 text-purple-400" />}
          title="Add to portal"
          body={
            <>
              <p className="text-gray-400 mb-3">
                This is the primary flow for tenant-managed machines. Go to your{' '}
                <Link to="/app" className="text-blue-400 hover:text-blue-300">
                  Dashboard at /app
                </Link>
                , click <span className="text-white font-medium">Add Machine</span>,
                paste the 9-digit ID and the permanent password you set on the
                machine. From then on, click <span className="text-white font-medium">Connect</span>{' '}
                any time to launch a remote session — no codes, no PINs, no
                re-typing credentials.
              </p>
              <p className="text-gray-500 text-sm">
                Tip: set a permanent password in RustDesk under{' '}
                <span className="text-gray-300">
                  Settings → Security → Unlock Security Settings → Permanent Password
                </span>
                . The portal stores it so launching takes one click.
              </p>
            </>
          }
          last
        />

        {/* CTA back to download */}
        <div className="mt-12 p-6 bg-gray-900 border border-gray-800 rounded-xl flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div>
            <h3 className="text-white font-semibold">Ready to install?</h3>
            <p className="text-sm text-gray-400 mt-1">
              Head back to the downloads page for all builds.
            </p>
          </div>
          <Link
            to="/download"
            className="inline-flex items-center gap-2 px-4 py-2.5 bg-blue-600 hover:bg-blue-700 text-white rounded-md text-sm font-medium"
          >
            Go to downloads <ArrowRight className="w-4 h-4" />
          </Link>
        </div>

        <p className="mt-10 text-center text-xs text-gray-600">
          Powered by RustDesk open-source components. Self-hosted on Callmor
          infrastructure.
        </p>
      </div>
    </div>
  );
}

function Step({
  n,
  icon,
  title,
  body,
  last,
}: {
  n: number;
  icon: React.ReactNode;
  title: string;
  body: React.ReactNode;
  last?: boolean;
}) {
  return (
    <section className={last ? 'pb-2' : 'pb-10 mb-10 border-b border-gray-800/60'}>
      <div className="flex items-start gap-4">
        <div className="flex flex-col items-center flex-shrink-0">
          <div className="w-10 h-10 rounded-full bg-purple-600 text-white font-semibold flex items-center justify-center text-sm">
            {n}
          </div>
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-3">
            {icon}
            <h2 className="text-xl font-semibold text-white">{title}</h2>
          </div>
          <div className="text-base leading-relaxed">{body}</div>
        </div>
      </div>
    </section>
  );
}

function PortRow({
  port,
  proto,
  purpose,
  last,
}: {
  port: string;
  proto: string;
  purpose: string;
  last?: boolean;
}) {
  return (
    <tr className={last ? '' : 'border-b border-gray-800/60'}>
      <td className="py-2.5 px-4">{port}</td>
      <td className="py-2.5 px-4">{proto}</td>
      <td className="py-2.5 px-4 font-sans text-gray-400">{purpose}</td>
    </tr>
  );
}
