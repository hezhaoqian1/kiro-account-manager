# Railway Headless Deployment

The Railway image runs `src-tauri/src/server.rs` as an Axum service. It does
not start Tauri, a WebView, a tray process, or a local desktop OAuth listener.
The React management console and all gateway protocols share the public
`$PORT` listener:

- `GET /healthz` is the unauthenticated Railway health check.
- `/api/*` is the administrator API and web console backend.
- `/v1/messages`, `/v1/chat/completions`, `/v1/responses`, and `/v1/models`
  are the client gateway APIs.

## Required Railway variables

Create a Railway Postgres service and make sure its `DATABASE_URL` is visible
to the application service. Set these variables in the application service:

```text
DATABASE_URL=${{Postgres.DATABASE_URL}}
ADMIN_TOKEN=<long random value used only for the web console>
PUBLIC_BASE_URL=https://<your-public-domain>
```

`PORT` is supplied by Railway. `KIRO_DATA_DIR` may be left at the image default;
it is only an ephemeral mirror of the database state and is recreated on every
deploy. `PUBLIC_BASE_URL` should be the public HTTPS URL for OAuth callbacks;
when omitted, the server also understands Railway's `RAILWAY_PUBLIC_DOMAIN`.
No Railway Volume is required. The gateway API key is generated and persisted
in the database-backed gateway configuration on first boot.

The server refuses to start when `ADMIN_TOKEN` or `DATABASE_URL` is missing.
Open the web console, go to Gateway -> API Key management, and generate or
rotate client keys there. Keys are never written to source, Docker layers,
request logs, or startup logs.

## First login

Open the service URL and enter `ADMIN_TOKEN`. In the Login page, Google/Github
starts the headless OAuth flow and returns through
`/api/auth/callback`. BuilderId/Enterprise accounts can be imported as the
JSON export produced by the desktop app. Imported accounts, refresh tokens,
gateway config, request logs, and response-cache files are mirrored to
Postgres by the server.

## Client endpoints

Use a key generated in the web console as `Authorization: Bearer ...` or
`x-api-key`:

```text
Anthropic: https://<domain>/v1/messages
OpenAI Chat: https://<domain>/v1/chat/completions
OpenAI Responses: https://<domain>/v1/responses
Models: https://<domain>/v1/models
```

The existing account pool, automatic token refresh, health/failure switching,
thinking/tool-call streaming, Responses session recovery, Kiro cache token
parsing, local token estimation, and full response-cache behavior remain in the
shared gateway modules.
