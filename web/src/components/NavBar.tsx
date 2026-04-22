import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Monitor, Menu, X, LogOut, LayoutDashboard } from 'lucide-react';
import { useAuth } from '../lib/auth';

/**
 * Public-site navigation bar. Renders on marketing / unauthenticated
 * pages (Landing, Connect, Download, Login, Register, legal stubs).
 * App-shell pages (Dashboard/Team/etc.) have their own header.
 */
export default function NavBar() {
  const { isAuthenticated, user, logout } = useAuth();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);

  const handleLogout = () => {
    logout();
    setOpen(false);
    navigate('/');
  };

  const links = [
    { to: '/', label: 'Home' },
    { to: '/connect', label: 'Connect' },
    { to: '/download', label: 'Download' },
    { to: '/rustdesk-setup', label: 'RustDesk' },
  ];

  return (
    <header className="sticky top-0 z-40 bg-gray-950/85 backdrop-blur border-b border-gray-800">
      <div className="max-w-6xl mx-auto px-4 sm:px-6 h-14 flex items-center justify-between">
        <Link to="/" className="flex items-center gap-2 text-white font-semibold">
          <Monitor className="w-5 h-5 text-blue-400" />
          <span>Callmor</span>
        </Link>

        {/* Desktop nav */}
        <nav className="hidden md:flex items-center gap-6">
          {links.map((l) => (
            <Link
              key={l.to}
              to={l.to}
              className="text-sm text-gray-300 hover:text-white transition"
            >
              {l.label}
            </Link>
          ))}
          {isAuthenticated ? (
            <div className="flex items-center gap-3">
              <Link
                to="/app"
                className="inline-flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
              >
                <LayoutDashboard className="w-4 h-4" /> Dashboard
              </Link>
              <span className="text-xs text-gray-500 hidden lg:inline">{user?.email}</span>
              <button
                onClick={handleLogout}
                className="text-gray-400 hover:text-white"
                title="Sign out"
              >
                <LogOut className="w-4 h-4" />
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-3">
              <Link to="/login" className="text-sm text-gray-300 hover:text-white">
                Sign in
              </Link>
              <Link
                to="/register"
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm font-medium"
              >
                Sign up
              </Link>
            </div>
          )}
        </nav>

        {/* Mobile toggle */}
        <button
          className="md:hidden text-gray-300 hover:text-white"
          onClick={() => setOpen((v) => !v)}
          aria-label="Toggle menu"
        >
          {open ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
        </button>
      </div>

      {/* Mobile drawer */}
      {open && (
        <div className="md:hidden border-t border-gray-800 bg-gray-950">
          <nav className="max-w-6xl mx-auto px-4 py-3 flex flex-col gap-1">
            {links.map((l) => (
              <Link
                key={l.to}
                to={l.to}
                onClick={() => setOpen(false)}
                className="px-2 py-2 text-sm text-gray-300 hover:text-white hover:bg-gray-900 rounded"
              >
                {l.label}
              </Link>
            ))}
            <div className="h-px bg-gray-800 my-1" />
            {isAuthenticated ? (
              <>
                <Link
                  to="/app"
                  onClick={() => setOpen(false)}
                  className="px-2 py-2 text-sm text-blue-400 hover:text-blue-300"
                >
                  Dashboard
                </Link>
                <button
                  onClick={handleLogout}
                  className="text-left px-2 py-2 text-sm text-gray-400 hover:text-white"
                >
                  Sign out
                </button>
              </>
            ) : (
              <>
                <Link
                  to="/login"
                  onClick={() => setOpen(false)}
                  className="px-2 py-2 text-sm text-gray-300 hover:text-white"
                >
                  Sign in
                </Link>
                <Link
                  to="/register"
                  onClick={() => setOpen(false)}
                  className="px-2 py-2 text-sm text-blue-400 hover:text-blue-300"
                >
                  Sign up
                </Link>
              </>
            )}
          </nav>
        </div>
      )}
    </header>
  );
}
