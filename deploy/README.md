# Deployment boundaries

The map-control plugin and the existing Minecraft server run as the same OS user as the website's private socket client. Socket directories are mode 0700 and sockets are mode 0600. Squaremap is a separately installed dependency. The plugin jar is built against the target server's Paper and squaremap APIs without bundling those dependencies.

For a deferred plugin update, Paper's configured update directory can hold a verified jar for the next operator-initiated restart. The correct update directory and plugin filename are deployment configuration; do not assume defaults or install a jar while changing a public repository. New profile capabilities become available after that restart, followed by the compatible website/backend deployment.

The original deployment uses OpenRC. Its parameterized service source is retained at `openrc/mcserver`; the private conf.d file is excluded and replaced by `openrc/mcserver.conf.example`. The service source defines supervised startup and SIGTERM shutdown with an eventual SIGKILL fallback when explicitly invoked by OpenRC. Working directory, OS identity, Java version, startup arguments, logs and supervisor policy remain deployment configuration. No repository build or test installs or invokes the service.

The website adapter recognizes the environment names in [the example](website.env.example). Paths and secrets must be supplied privately. Existing world seeds, management secrets, player lists and server.properties never belong in this repository.
