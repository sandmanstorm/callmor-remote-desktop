import { Link } from 'react-router-dom';
import {
  ArrowRight,
  Download as DownloadIcon,
  Monitor,
  Lock,
  Shield,
  Zap,
  LayoutDashboard,
  Server,
  Check,
} from 'lucide-react';
import { useAuth } from '../lib/auth';

export default function Landing() {
  const { isAuthenticated } = useAuth();

  return (
    <div>
      {/* Hero */}
      <section className="relative overflow-hidden">
        <div
          className="absolute inset-0 -z-10 opacity-70"
          style={{
            background:
              'radial-gradient(60% 60% at 50% 0%, rgba(37,99,235,0.22) 0%, rgba(10,10,10,0) 60%)',
          }}
        />
        <div className="max-w-5xl mx-auto px-4 sm:px-6 pt-20 pb-16 text-center">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-gray-800 bg-gray-900/60 text-xs text-gray-400 mb-6">
            <span className="w-1.5 h-1.5 rounded-full bg-green-400" />
            Built for teams and one-off sessions alike
          </div>
          <h1 className="text-4xl md:text-5xl font-bold text-white tracking-tight">
            Remote access that just works
          </h1>
          <p className="mt-5 text-xl text-gray-400 max-w-2xl mx-auto">
            Share an access code and a 4-digit PIN. The other side types them in
            a browser. End-to-end encrypted, no account required.
          </p>
          <div className="mt-8 flex flex-col sm:flex-row items-center justify-center gap-3">
            <Link
              to="/connect"
              className="inline-flex items-center gap-2 px-5 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-md text-sm font-medium"
            >
              Connect to a computer <ArrowRight className="w-4 h-4" />
            </Link>
            <Link
              to="/download"
              className="inline-flex items-center gap-2 px-5 py-3 bg-gray-900 hover:bg-gray-800 border border-gray-700 text-gray-100 rounded-md text-sm font-medium"
            >
              <DownloadIcon className="w-4 h-4" /> Share your computer
            </Link>
            {isAuthenticated && (
              <Link
                to="/app"
                className="inline-flex items-center gap-2 px-5 py-3 bg-transparent hover:bg-gray-900 border border-gray-800 text-gray-200 rounded-md text-sm font-medium"
              >
                <LayoutDashboard className="w-4 h-4" /> Go to Dashboard
              </Link>
            )}
          </div>
        </div>
      </section>

      {/* Feature row */}
      <section className="max-w-5xl mx-auto px-4 sm:px-6 pb-20">
        <div className="grid md:grid-cols-3 gap-4">
          <Feature
            icon={<Zap className="w-5 h-5 text-blue-400" />}
            title="No account needed"
            body="Ad-hoc sessions work like a phone call. Install the agent on one side, type the code on the other. That's it."
          />
          <Feature
            icon={<Lock className="w-5 h-5 text-blue-400" />}
            title="End-to-end encrypted WebRTC"
            body="Video and input travel over DTLS-SRTP directly between the two browsers and the machine. We never see pixels."
          />
          <Feature
            icon={<Shield className="w-5 h-5 text-blue-400" />}
            title="Works through any firewall"
            body="If a direct peer-to-peer path fails, traffic relays through our TURN servers automatically. No router config, no VPN."
          />
        </div>
      </section>

      {/* Two ways to connect */}
      <section className="border-t border-gray-800 bg-gray-950">
        <div className="max-w-5xl mx-auto px-4 sm:px-6 py-20">
          <div className="text-center mb-10">
            <h2 className="text-3xl md:text-4xl font-bold text-white">Two ways to connect</h2>
            <p className="mt-3 text-gray-400 text-lg">
              Pick the mode that fits the job. You can use both.
            </p>
          </div>
          <div className="grid md:grid-cols-2 gap-5">
            <ModeCard
              accent="blue"
              icon={<Zap className="w-5 h-5 text-blue-400" />}
              title="Quick Connect"
              tagline="Works immediately. No setup."
              body="One click on the host, a code and PIN appear, share them, the other side connects in any browser. Perfect for ad-hoc support."
              bullets={[
                'One-click portable agent',
                'Browser-based viewer',
                'No router config',
              ]}
              ctaLabel="Connect with a code"
              ctaTo="/connect"
            />
            <ModeCard
              accent="purple"
              icon={<Server className="w-5 h-5 text-purple-400" />}
              title="RustDesk Mode"
              tagline="Full-featured. Persistent machines."
              body="Install the Callmor client once per computer and reach it forever by its 9-digit ID. Full streaming, file transfer, 2FA. Requires port forwarding on the host network."
              bullets={[
                'Permanent 9-digit ID per machine',
                'File transfer, 2FA, recording',
                'Self-hosted on Callmor infra',
              ]}
              ctaLabel="See setup guide"
              ctaTo="/rustdesk-setup"
            />
          </div>
        </div>
      </section>

      {/* How it works */}
      <section className="border-t border-gray-800 bg-gray-950">
        <div className="max-w-5xl mx-auto px-4 sm:px-6 py-20">
          <div className="text-center mb-12">
            <h2 className="text-3xl md:text-4xl font-bold text-white">How it works</h2>
            <p className="mt-3 text-gray-400 text-lg">Three steps. That's the whole product.</p>
          </div>
          <div className="grid md:grid-cols-3 gap-4">
            <Step
              n={1}
              title="Download and run the agent"
              body={
                <>
                  Install on the computer you want to share from our <Link
                    to="/download"
                    className="text-blue-400 hover:text-blue-300"
                  >Download page</Link>. Takes about 30 seconds.
                </>
              }
            />
            <Step
              n={2}
              title="Share the code and PIN"
              body="The agent displays an access code and a 4-digit PIN on screen. Read them to whoever is connecting, or paste them into a chat."
            />
            <Step
              n={3}
              title="Type them at remote.callmor.ai/connect"
              body={
                <>
                  The other person opens <Link
                    to="/connect"
                    className="text-blue-400 hover:text-blue-300"
                  >remote.callmor.ai/connect</Link>, enters the code and PIN, and the session starts.
                </>
              }
            />
          </div>
        </div>
      </section>

      {/* Teams CTA */}
      <section className="max-w-5xl mx-auto px-4 sm:px-6 py-20">
        <div className="bg-gradient-to-b from-gray-900 to-gray-950 border border-gray-800 rounded-xl p-8 md:p-12 flex flex-col md:flex-row items-start md:items-center justify-between gap-6">
          <div>
            <div className="flex items-center gap-2 text-blue-400 mb-2">
              <Monitor className="w-4 h-4" />
              <span className="text-xs uppercase tracking-wider font-medium">For teams</span>
            </div>
            <h3 className="text-2xl md:text-3xl font-bold text-white">Manage a fleet of machines</h3>
            <p className="mt-2 text-gray-400 max-w-xl">
              Create an organization to enroll machines permanently, control
              access per user, record sessions, and audit activity.
            </p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {isAuthenticated ? (
              <Link
                to="/app"
                className="inline-flex items-center gap-2 px-5 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-md text-sm font-medium"
              >
                Open Dashboard <ArrowRight className="w-4 h-4" />
              </Link>
            ) : (
              <>
                <Link
                  to="/login"
                  className="px-5 py-3 text-sm text-gray-300 hover:text-white"
                >
                  Sign in
                </Link>
                <Link
                  to="/register"
                  className="inline-flex items-center gap-2 px-5 py-3 bg-blue-600 hover:bg-blue-700 text-white rounded-md text-sm font-medium"
                >
                  Create organization <ArrowRight className="w-4 h-4" />
                </Link>
              </>
            )}
          </div>
        </div>
      </section>
    </div>
  );
}

function Feature({ icon, title, body }: { icon: React.ReactNode; title: string; body: string }) {
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-6 hover:border-gray-700 transition">
      <div className="w-9 h-9 rounded-md bg-blue-500/10 border border-blue-500/20 flex items-center justify-center mb-4">
        {icon}
      </div>
      <h3 className="text-white font-semibold mb-2">{title}</h3>
      <p className="text-sm text-gray-400 leading-relaxed">{body}</p>
    </div>
  );
}

function ModeCard({
  accent,
  icon,
  title,
  tagline,
  body,
  bullets,
  ctaLabel,
  ctaTo,
}: {
  accent: 'blue' | 'purple';
  icon: React.ReactNode;
  title: string;
  tagline: string;
  body: string;
  bullets: string[];
  ctaLabel: string;
  ctaTo: string;
}) {
  const iconWrap =
    accent === 'blue'
      ? 'bg-blue-500/10 border border-blue-500/20'
      : 'bg-purple-500/10 border border-purple-500/20';
  const cta =
    accent === 'blue'
      ? 'bg-blue-600 hover:bg-blue-700'
      : 'bg-purple-600 hover:bg-purple-700';
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-xl p-6 flex flex-col">
      <div className="flex items-center gap-3 mb-2">
        <div className={`w-10 h-10 rounded-md flex items-center justify-center ${iconWrap}`}>
          {icon}
        </div>
        <div>
          <h3 className="text-xl font-semibold text-white">{title}</h3>
          <div className="text-xs text-gray-500">{tagline}</div>
        </div>
      </div>
      <p className="text-sm text-gray-400 mt-3">{body}</p>
      <ul className="mt-4 space-y-2 text-sm text-gray-300 flex-1">
        {bullets.map((b) => (
          <li key={b} className="flex items-start gap-2">
            <Check className="w-4 h-4 text-green-400 flex-shrink-0 mt-0.5" />
            <span>{b}</span>
          </li>
        ))}
      </ul>
      <Link
        to={ctaTo}
        className={`mt-6 inline-flex items-center justify-center gap-2 px-4 py-2.5 ${cta} text-white rounded-md text-sm font-medium`}
      >
        {ctaLabel} <ArrowRight className="w-4 h-4" />
      </Link>
    </div>
  );
}

function Step({ n, title, body }: { n: number; title: string; body: React.ReactNode }) {
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-6">
      <div className="w-8 h-8 rounded-full bg-blue-600 text-white font-semibold flex items-center justify-center mb-4 text-sm">
        {n}
      </div>
      <h3 className="text-white font-semibold mb-2">{title}</h3>
      <p className="text-sm text-gray-400 leading-relaxed">{body}</p>
    </div>
  );
}
