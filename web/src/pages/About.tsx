export default function About() {
  return (
    <div className="max-w-3xl mx-auto px-4 sm:px-6 py-16">
      <div className="bg-gray-900 border border-gray-800 rounded-lg p-8 md:p-10">
        <h1 className="text-3xl font-bold text-white mb-4">About Callmor</h1>
        <div className="space-y-4 text-gray-300 leading-relaxed">
          <p>
            Callmor Remote Desktop is a simple, secure way to see and control a
            computer somewhere else. We believe remote access should feel like
            a phone call — fast to start, safe by default, and disposable when
            you're done.
          </p>
          <p>
            Built on WebRTC with end-to-end encryption, Callmor works out of
            the box on desktop and mobile browsers. There's no client to
            install for the person connecting, and the computer being shared
            only runs a small background agent.
          </p>
        </div>
      </div>
    </div>
  );
}
