# Security Policy

## Supported scope

Security fixes apply to the current development branch until versioned releases exist. Release support policy will be recorded before M3.

## Reporting

Do not open a public issue containing source lessons, voice data, private paths, credentials, model artifacts, generated private audio, or an exploitable security detail. Use a private repository security advisory when available. If no private channel is configured, stop publication and contact the repository owner through a previously established private channel.

## Security boundaries

Treat lessons, JSON, Markdown, paths, worker frames, audio, model artifacts, media metadata, and external-process output as untrusted. Reports should include the affected revision, impact, reproduction using nonsensitive fixtures, and any known containment action.

Do not delete evidence or affected artifacts during initial containment. Disable the unsafe operation, preserve checksums and manifests, and follow the incident and rollback procedure.

