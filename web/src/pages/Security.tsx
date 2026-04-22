export default function Security() {
  return (
    <div className="max-w-3xl mx-auto px-4 sm:px-6 py-16">
      <div className="bg-gray-900 border border-gray-800 rounded-lg p-8 md:p-10">
        <h1 className="text-3xl font-bold text-white mb-4">Security</h1>
        <div className="space-y-4 text-gray-300 leading-relaxed">
          <p>
            Every Callmor session uses WebRTC with DTLS-SRTP encryption for
            video, audio, and input. Media streams run directly between the
            agent and the viewer whenever a peer-to-peer path is possible; if
            not, traffic is relayed through our TURN servers but remains
            encrypted end-to-end. Callmor operators cannot view session
            contents.
          </p>
          <p>
            Ad-hoc sessions are authenticated with a short access code and a
            single-use 4-digit PIN that expires when the session ends. For
            organizations, machines enroll with a rotatable tenant token and
            every session is authorized against a per-machine access list.
          </p>
          <p>
            All access events (logins, machine enrollments, session starts,
            permission changes) are written to an audit log that administrators
            can review at any time.
          </p>
        </div>
      </div>
    </div>
  );
}
