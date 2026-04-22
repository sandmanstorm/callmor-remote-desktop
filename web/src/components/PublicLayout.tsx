import { Link } from 'react-router-dom';
import NavBar from './NavBar';

/**
 * Wrapper for all public / marketing pages: landing, connect, download,
 * login, register, legal stubs, accept-invite. Renders a shared top NavBar
 * and a minimal footer.
 */
export default function PublicLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen flex flex-col bg-gray-950 text-gray-100">
      <NavBar />
      <main className="flex-1">{children}</main>
      <footer className="border-t border-gray-800 bg-gray-950">
        <div className="max-w-6xl mx-auto px-4 sm:px-6 py-6 flex flex-col sm:flex-row items-center justify-between gap-3">
          <div className="text-xs text-gray-500">
            &copy; {new Date().getFullYear()} Callmor &middot; Remote access that just works
          </div>
          <nav className="flex items-center gap-5 text-xs text-gray-400">
            <Link to="/about" className="hover:text-white">About</Link>
            <Link to="/security" className="hover:text-white">Security</Link>
            <Link to="/terms" className="hover:text-white">Terms</Link>
            <Link to="/privacy" className="hover:text-white">Privacy</Link>
          </nav>
        </div>
      </footer>
    </div>
  );
}
