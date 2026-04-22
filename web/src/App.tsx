import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { AuthProvider, useAuth } from './lib/auth';
import Login from './pages/Login';
import Register from './pages/Register';
import Dashboard from './pages/Dashboard';
import Team from './pages/Team';
import Admin from './pages/Admin';
import Activity from './pages/Activity';
import Recordings from './pages/Recordings';
import AcceptInvite from './pages/AcceptInvite';
import Connect from './pages/Connect';
import Download from './pages/Download';
import RustdeskSetup from './pages/RustdeskSetup';
import Landing from './pages/Landing';
import About from './pages/About';
import Security from './pages/Security';
import Terms from './pages/Terms';
import Privacy from './pages/Privacy';
import Viewer from './pages/Viewer';
import PublicLayout from './components/PublicLayout';

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  return isAuthenticated ? <>{children}</> : <Navigate to="/login" />;
}

function PublicOnlyRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated } = useAuth();
  return isAuthenticated ? <Navigate to="/app" /> : <>{children}</>;
}

function App() {
  return (
    <AuthProvider>
      <BrowserRouter>
        <Routes>
          {/* Public marketing + adhoc flow (wrapped with NavBar + footer) */}
          <Route path="/" element={<PublicLayout><Landing /></PublicLayout>} />
          <Route path="/connect" element={<PublicLayout><Connect /></PublicLayout>} />
          <Route path="/download" element={<PublicLayout><Download /></PublicLayout>} />
          <Route path="/rustdesk-setup" element={<PublicLayout><RustdeskSetup /></PublicLayout>} />
          <Route path="/about" element={<PublicLayout><About /></PublicLayout>} />
          <Route path="/security" element={<PublicLayout><Security /></PublicLayout>} />
          <Route path="/terms" element={<PublicLayout><Terms /></PublicLayout>} />
          <Route path="/privacy" element={<PublicLayout><Privacy /></PublicLayout>} />
          <Route path="/invite/:token" element={<PublicLayout><AcceptInvite /></PublicLayout>} />

          {/* Auth forms — redirect to /app if already signed in */}
          <Route
            path="/login"
            element={
              <PublicOnlyRoute>
                <PublicLayout><Login /></PublicLayout>
              </PublicOnlyRoute>
            }
          />
          <Route
            path="/register"
            element={
              <PublicOnlyRoute>
                <PublicLayout><Register /></PublicLayout>
              </PublicOnlyRoute>
            }
          />

          {/* Full-screen viewer — accessible without login (adhoc tokens) */}
          <Route path="/viewer/:machineId" element={<Viewer />} />

          {/* Authenticated app */}
          <Route path="/app" element={<ProtectedRoute><Dashboard /></ProtectedRoute>} />
          <Route path="/team" element={<ProtectedRoute><Team /></ProtectedRoute>} />
          <Route path="/activity" element={<ProtectedRoute><Activity /></ProtectedRoute>} />
          <Route path="/recordings" element={<ProtectedRoute><Recordings /></ProtectedRoute>} />
          <Route path="/admin" element={<ProtectedRoute><Admin /></ProtectedRoute>} />

          {/* Fallback */}
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </BrowserRouter>
    </AuthProvider>
  );
}

export default App;
