# Hermes HTTP API

Hermes is an optional loopback API for local agent clients. It listens only on
`127.0.0.1:17321`; it is not a network or browser API.

## Enable and get a token

Start Hermes in **Settings → Hermes**. On the first start, Maktabati generates a
cryptographically random 256-bit bearer token and stores it in the application settings database.
The token is shown in that panel and can be copied from there. Use **Regenerate token** to revoke
the current token immediately and issue a replacement. The generic settings read/write commands
do not expose this secret.

## Authenticate requests

Send the token in the `Authorization` header on every request except `GET /health`:

```http
Authorization: Bearer YOUR_TOKEN
```

For example, from a local agent or shell client:

```sh
curl --fail-with-body \
  -H "Authorization: Bearer $MAKTABATI_HERMES_TOKEN" \
  "http://127.0.0.1:17321/books?query=arabic"
```

`GET /health` is unauthenticated for liveness checks. Every request, including health checks,
must use a `Host` of `127.0.0.1:17321` or `localhost:17321`. Requests carrying any `Origin`
header are rejected; browser access and CORS are intentionally unsupported. A token rotation
invalidates existing client credentials, so update the agent's local secret before reconnecting.
