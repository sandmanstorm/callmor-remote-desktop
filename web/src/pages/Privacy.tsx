export default function Privacy() {
  return (
    <div className="max-w-3xl mx-auto px-4 sm:px-6 py-16">
      <div className="bg-gray-900 border border-gray-800 rounded-lg p-8 md:p-10">
        <h1 className="text-3xl font-bold text-white mb-4">Privacy</h1>
        <div className="space-y-4 text-gray-300 leading-relaxed">
          <p>
            Callmor collects the minimum information needed to operate the
            service: account email and display name, machine names and
            metadata (hostname, OS, last-seen time), and session signaling
            events. We do not record screen contents or inputs unless
            recording is explicitly enabled by an organization administrator
            — in which case the recordings are stored in that organization's
            own storage bucket.
          </p>
          <p>
            TURN relays forward encrypted packets when a direct peer-to-peer
            path isn't available. The relay sees the source and destination
            endpoints but never the decrypted media.
          </p>
          <p>
            You can delete your account and any associated machines at any
            time from the dashboard. Deletions propagate within minutes.
          </p>
        </div>
      </div>
    </div>
  );
}
